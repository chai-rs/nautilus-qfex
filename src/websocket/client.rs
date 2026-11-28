//! One WebSocket client for both QFEX sockets.
//!
//! Market data (`mds`): no credentials; subscriptions are recorded and sent again after a
//! reconnect. Trade: sends the HMAC `auth` frame after every connect and reconnect and, once the
//! venue answers `success`, the session frames (channel subscriptions, cancel on disconnect).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    sync::Arc,
    time::{Duration, Instant},
};

use nautilus_common::live::get_runtime;
use nautilus_network::{
    RECONNECTED, SocketStateSink,
    error::SendError,
    mode::ReconnectRequestOutcome,
    websocket::{
        WebSocketClient, WebSocketConfig, WebSocketReconnectHandle, channel_epoch_message_handler,
    },
};
use serde_json::Value;
use strum::IntoEnumIterator;
use tokio::{
    sync::{Mutex, Notify, mpsc},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::Message;
use ustr::Ustr;

use super::{
    error::QfexWsClientError,
    messages::{QfexMdsRequest, QfexTradeRequest, QfexWsMessage},
};
use crate::{
    common::{
        enums::{QfexMdsChannel, QfexTradeChannel},
        recording::QfexRecorder,
    },
    signing::QfexSigner,
};

/// Client ping frames. After `auth` the venue also pings; the transport answers those.
const HEARTBEAT_INTERVAL_SECS: u64 = 20;
const RECONNECT_DELAY_INITIAL_MS: u64 = 250;
const RECONNECT_DELAY_MAX_MS: u64 = 30_000;
const RECONNECT_BACKOFF_FACTOR: f64 = 2.0;
const RECONNECT_JITTER_MS: u64 = 200;

/// How many market data subscribe frames may await a venue acknowledgement at once. QFEX
/// publishes no quota, so the gate bounds the burst in frames instead of guessing a rate.
const SUBSCRIBE_INFLIGHT_MAX: usize = 4;
/// Registrations that arrive inside this window leave as one frame a channel.
const SUBSCRIBE_DEBOUNCE_MS: u64 = 50;
/// A subscribe frame the venue never answered is sent again after this long.
const SUBSCRIBE_ACK_TIMEOUT_MS: u64 = 10_000;
/// First wait before a refused subscribe is sent again, and the ceiling it doubles up to.
const SUBSCRIBE_RETRY_INITIAL_MS: u64 = 250;
const SUBSCRIBE_RETRY_MAX_MS: u64 = 30_000;

type Subscriptions = Arc<Mutex<BTreeSet<(QfexMdsChannel, Ustr)>>>;

/// What is sent again on every (re)connection.
#[derive(Clone)]
struct Session {
    /// Trade socket only.
    signer: Option<QfexSigner>,
    /// Trade socket: frames sent after each successful `auth`.
    frames: Arc<Vec<String>>,
    /// Market data socket.
    subscriptions: Subscriptions,
    /// Market data socket: paces the live subscribe path.
    gate: Arc<SubscribeGate>,
    trade_recorder: Option<QfexRecorder>,
}

impl Session {
    fn record_sent(&self, text: &str) {
        if let Some(recorder) = &self.trade_recorder {
            recorder.sent(text);
        }
    }
}

/// One outgoing market data frame: one channel and every symbol of it.
struct Frame {
    subscribe: bool,
    channel: QfexMdsChannel,
    symbols: Vec<Ustr>,
}

impl Frame {
    fn request(&self) -> QfexMdsRequest<'_> {
        let channels = std::slice::from_ref(&self.channel);
        let symbols = self.symbols.as_slice();
        if self.subscribe {
            QfexMdsRequest::Subscribe { channels, symbols }
        } else {
            QfexMdsRequest::Unsubscribe { channels, symbols }
        }
    }
}

/// Registrations waiting to go out, and the frames the venue has not answered yet.
struct GateState {
    subscribe: BTreeSet<(QfexMdsChannel, Ustr)>,
    unsubscribe: BTreeSet<(QfexMdsChannel, Ustr)>,
    /// One entry a channel: when the frame went out and what it held, so a refused or
    /// unanswered frame can be queued again.
    inflight: BTreeMap<QfexMdsChannel, (Instant, Vec<Ustr>)>,
    /// Earliest the next subscribe may go out; set after the venue refuses one.
    hold_until: Option<Instant>,
    retry_ms: u64,
}

impl Default for GateState {
    fn default() -> Self {
        Self {
            subscribe: BTreeSet::new(),
            unsubscribe: BTreeSet::new(),
            inflight: BTreeMap::new(),
            hold_until: None,
            retry_ms: SUBSCRIBE_RETRY_INITIAL_MS,
        }
    }
}

impl GateState {
    /// Whether a subscribe frame may go out: budget left, and a channel with none in flight.
    fn sendable(&self) -> bool {
        self.inflight.len() < SUBSCRIBE_INFLIGHT_MAX
            && self
                .subscribe
                .iter()
                .any(|(channel, _)| !self.inflight.contains_key(channel))
    }

    /// Queues the subscriptions of frames the venue never answered.
    fn expire(&mut self, now: Instant) {
        let timeout = Duration::from_millis(SUBSCRIBE_ACK_TIMEOUT_MS);
        let stale: Vec<QfexMdsChannel> = self
            .inflight
            .iter()
            .filter(|(_, (sent, _))| now.duration_since(*sent) >= timeout)
            .map(|(channel, _)| *channel)
            .collect();
        for channel in stale {
            if let Some((_, symbols)) = self.inflight.remove(&channel) {
                self.subscribe
                    .extend(symbols.into_iter().map(|symbol| (channel, symbol)));
            }
        }
    }

    /// How long the pump waits before it flushes again. `None` means nothing is owed.
    fn next_wait(&self, now: Instant) -> Option<Duration> {
        if !self.unsubscribe.is_empty() {
            return Some(Duration::ZERO);
        }
        if self.sendable() {
            return match self.hold_until {
                Some(hold) if hold > now => Some(hold - now),
                _ => Some(Duration::ZERO),
            };
        }
        // Capped: wake when the oldest unanswered frame times out.
        self.inflight
            .values()
            .map(|(sent, _)| *sent)
            .min()
            .map(|sent| {
                (sent + Duration::from_millis(SUBSCRIBE_ACK_TIMEOUT_MS))
                    .saturating_duration_since(now)
            })
    }

    /// The frames to send now: the drops, then one subscribe frame a channel up to the budget.
    fn take_frames(&mut self, now: Instant) -> Vec<Frame> {
        let mut frames: Vec<Frame> = group(std::mem::take(&mut self.unsubscribe))
            .into_iter()
            .map(|(channel, symbols)| Frame {
                subscribe: false,
                channel,
                symbols,
            })
            .collect();
        if self.hold_until.is_some_and(|hold| hold > now) {
            return frames;
        }
        self.hold_until = None;
        while self.sendable() {
            let Some(channel) = self
                .subscribe
                .iter()
                .map(|(channel, _)| *channel)
                .find(|channel| !self.inflight.contains_key(channel))
            else {
                break;
            };
            let symbols: Vec<Ustr> = self
                .subscribe
                .iter()
                .filter(|(c, _)| *c == channel)
                .map(|(_, symbol)| *symbol)
                .collect();
            self.subscribe.retain(|(c, _)| *c != channel);
            self.inflight.insert(channel, (now, symbols.clone()));
            frames.push(Frame {
                subscribe: true,
                channel,
                symbols,
            });
        }
        frames
    }

    fn hold_off(&mut self) {
        self.hold_until = Some(Instant::now() + Duration::from_millis(self.retry_ms));
        self.retry_ms = self.retry_ms.saturating_mul(2).min(SUBSCRIBE_RETRY_MAX_MS);
    }
}

/// `(channel, symbol)` pairs as one group a channel. The set is ordered by channel first, so
/// one pass is enough.
fn group(pairs: BTreeSet<(QfexMdsChannel, Ustr)>) -> Vec<(QfexMdsChannel, Vec<Ustr>)> {
    let mut groups: Vec<(QfexMdsChannel, Vec<Ustr>)> = Vec::new();
    for (channel, symbol) in pairs {
        match groups.last_mut() {
            Some((last, symbols)) if *last == channel => symbols.push(symbol),
            _ => groups.push((channel, vec![symbol])),
        }
    }
    groups
}

/// Paces the live market data subscribe path: registrations are collected and leave as one
/// frame a channel, the shape `start_session` already sends on every (re)connection.
#[derive(Default)]
struct SubscribeGate {
    state: Mutex<GateState>,
    wake: Notify,
}

impl SubscribeGate {
    /// Records one registration. `subscribe` false queues a drop.
    async fn queue(&self, subscribe: bool, channel: QfexMdsChannel, symbol: Ustr) {
        let pair = (channel, symbol);
        let mut state = self.state.lock().await;
        if subscribe {
            // A drop not sent yet cancels out: the venue subscription stays as it is.
            if !state.unsubscribe.remove(&pair) {
                state.subscribe.insert(pair);
            }
        } else {
            let sent = state
                .inflight
                .get(&channel)
                .is_some_and(|(_, symbols)| symbols.contains(&symbol));
            // Nothing to drop at the venue if the subscribe never went out.
            if !state.subscribe.remove(&pair) || sent {
                state.unsubscribe.insert(pair);
            }
        }
        drop(state);
        self.wake.notify_one();
    }

    /// The venue answered one frame.
    async fn acknowledged(&self) {
        let mut state = self.state.lock().await;
        // The acknowledgement carries no identity the adapter kept, so the oldest unanswered
        // frame is the one released. An `unsubscribed` ack releases one too, which only
        // loosens the cap by the drops in flight.
        let oldest = state
            .inflight
            .iter()
            .min_by_key(|(_, (sent, _))| *sent)
            .map(|(channel, _)| *channel);
        if let Some(channel) = oldest {
            state.inflight.remove(&channel);
            state.retry_ms = SUBSCRIBE_RETRY_INITIAL_MS;
        }
        drop(state);
        self.wake.notify_one();
    }

    /// The venue refused a frame: queue what it held again, after a backoff.
    async fn refused(&self, incoming: &Value) {
        if incoming.get("type").and_then(Value::as_str) != Some("subscribe") {
            return;
        }
        let Some(channels) = incoming.get("channels").and_then(Value::as_array) else {
            return;
        };
        let mut state = self.state.lock().await;
        let mut queued = false;
        for channel in channels {
            let Ok(channel) = serde_json::from_value::<QfexMdsChannel>(channel.clone()) else {
                continue;
            };
            if let Some((_, symbols)) = state.inflight.remove(&channel) {
                state
                    .subscribe
                    .extend(symbols.into_iter().map(|symbol| (channel, symbol)));
                queued = true;
            }
        }
        if queued {
            state.hold_off();
        }
        drop(state);
        self.wake.notify_one();
    }

    /// The connection went: `start_session` sends every recorded subscription again, and no
    /// acknowledgement from the old connection is coming. What is queued stays queued, because
    /// a repeated subscribe is harmless and a registration may have arrived mid-reconnect.
    async fn connection_lost(&self) {
        let mut state = self.state.lock().await;
        state.inflight.clear();
        state.hold_until = None;
        state.retry_ms = SUBSCRIBE_RETRY_INITIAL_MS;
        drop(state);
        self.wake.notify_one();
    }
}

/// Sends the market data frames of a live connection: one frame a channel, and at most
/// `SUBSCRIBE_INFLIGHT_MAX` subscribes awaiting an acknowledgement.
async fn subscribe_pump(client: Arc<WebSocketClient>, gate: Arc<SubscribeGate>) {
    loop {
        let wait = {
            let mut state = gate.state.lock().await;
            let now = Instant::now();
            state.expire(now);
            state.next_wait(now)
        };
        match wait {
            None => gate.wake.notified().await,
            // An acknowledgement or a new registration cuts the wait short.
            Some(wait) if !wait.is_zero() => {
                let _ = tokio::time::timeout(wait, gate.wake.notified()).await;
            }
            Some(_) => {}
        }
        // Collect the registrations of one breath, so many symbols leave as one frame.
        tokio::time::sleep(Duration::from_millis(SUBSCRIBE_DEBOUNCE_MS)).await;
        let frames = gate.state.lock().await.take_frames(Instant::now());
        for frame in frames {
            if let Err(e) = send_json(&client, &frame.request()).await {
                log::warn!("QFEX WebSocket: subscription frame not sent: {e}");
            }
        }
    }
}

/// A QFEX WebSocket connection that reconnects on its own.
pub struct QfexWebSocketClient {
    url: String,
    session: Session,
    client: Option<Arc<WebSocketClient>>,
    task: Option<JoinHandle<()>>,
    pump: Option<JoinHandle<()>>,
    state_sink: Option<SocketStateSink>,
    reconnect: Arc<parking_lot::Mutex<Option<WebSocketReconnectHandle>>>,
}

impl Debug for QfexWebSocketClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The trade URL carries the public key: keep it out of logs.
        f.debug_struct(stringify!(QfexWebSocketClient))
            .field("trade", &self.session.signer.is_some())
            .field("connected", &self.is_active())
            .finish_non_exhaustive()
    }
}

impl QfexWebSocketClient {
    /// A market data client. Needs no credentials.
    #[must_use]
    pub fn new_mds(url: String) -> Self {
        Self {
            url,
            session: Session {
                signer: None,
                frames: Arc::default(),
                subscriptions: Arc::default(),
                gate: Arc::default(),
                trade_recorder: None,
            },
            client: None,
            task: None,
            pump: None,
            state_sink: None,
            reconnect: Arc::default(),
        }
    }

    /// Reports `Connected` and `Disconnected` of every connection this client makes to `sink`.
    #[must_use]
    pub fn with_state_sink(mut self, sink: SocketStateSink) -> Self {
        self.state_sink = Some(sink);
        self
    }

    /// Hands every frame of the trade socket, sent or received, to `recorder`. The `auth`
    /// frame is recorded redacted and the URL, which carries the public key, never.
    #[must_use]
    pub fn with_recorder(mut self, recorder: QfexRecorder) -> Self {
        self.session.trade_recorder = Some(recorder);
        self
    }

    /// Asks the live transport to reconnect; `Closed` while there is none.
    pub fn reconnect_request(
        &self,
    ) -> impl Fn() -> ReconnectRequestOutcome + Send + Sync + 'static {
        let reconnect = self.reconnect.clone();
        move || {
            reconnect.lock().as_ref().map_or(
                ReconnectRequestOutcome::Closed,
                WebSocketReconnectHandle::request_reconnect,
            )
        }
    }

    /// A trade client: authenticates, subscribes to the four private channels and, when asked,
    /// turns on cancel on disconnect. All of it is repeated after a reconnect.
    ///
    /// # Errors
    ///
    /// Returns an error if `url` is not a URL.
    pub fn new_trade(
        url: &str,
        signer: QfexSigner,
        cancel_on_disconnect: bool,
    ) -> Result<Self, QfexWsClientError> {
        let url = url::Url::parse_with_params(url, [("api_key", signer.api_key())])
            .map_err(|e| QfexWsClientError::Client(format!("invalid trade socket URL: {e}")))?
            .to_string();

        let mut frames = vec![
            QfexTradeRequest::Subscribe {
                channels: QfexTradeChannel::iter().collect(),
            }
            .to_frame(),
        ];
        if cancel_on_disconnect {
            frames.push(
                QfexTradeRequest::CancelOnDisconnect {
                    cancel_on_disconnect: true,
                }
                .to_frame(),
            );
        }

        Ok(Self {
            url,
            session: Session {
                signer: Some(signer),
                frames: Arc::new(frames),
                subscriptions: Arc::default(),
                gate: Arc::default(),
                trade_recorder: None,
            },
            client: None,
            task: None,
            pump: None,
            state_sink: None,
            reconnect: Arc::default(),
        })
    }

    /// Connects and returns the stream of frames.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or the first frame (auth, subscriptions) fails.
    pub async fn connect(
        &mut self,
    ) -> Result<mpsc::UnboundedReceiver<QfexWsMessage>, QfexWsClientError> {
        self.disconnect().await;

        let config = WebSocketConfig::builder()
            .url(self.url.clone())
            .heartbeat_interval_secs(HEARTBEAT_INTERVAL_SECS)
            .reconnect_delay_initial_ms(RECONNECT_DELAY_INITIAL_MS)
            .reconnect_delay_max_ms(RECONNECT_DELAY_MAX_MS)
            .reconnect_backoff_factor(RECONNECT_BACKOFF_FACTOR)
            .reconnect_jitter_ms(RECONNECT_JITTER_MS)
            .build()
            .map_err(|e| QfexWsClientError::Client(e.to_string()))?;
        let (handler, raw_rx) = channel_epoch_message_handler();
        let client = WebSocketClient::epoch_builder()
            .config(config)
            .epoch_handler(handler)
            .maybe_state_sink(self.state_sink.clone())
            .connect()
            .await
            .map_err(|e| QfexWsClientError::Network(e.to_string()))?;
        let client = Arc::new(client);

        // The read task starts after this so a reconnect cannot interleave with the first send.
        if let Err(e) = start_session(&client, &self.session, 0).await {
            client.disconnect().await;
            return Err(e);
        }
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        *self.reconnect.lock() = Some(client.reconnect_handle());
        self.client = Some(client.clone());
        if self.session.signer.is_none() {
            self.pump = Some(
                get_runtime().spawn(subscribe_pump(client.clone(), self.session.gate.clone())),
            );
        }
        self.task =
            Some(get_runtime().spawn(read_loop(raw_rx, out_tx, client, self.session.clone())));

        Ok(out_rx)
    }

    /// Closes the connection and stops the read and subscribe tasks.
    pub async fn disconnect(&mut self) {
        self.reconnect.lock().take();
        if let Some(client) = self.client.take() {
            client.disconnect().await;
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
        self.session.gate.connection_lost().await;
    }

    /// Whether the connection is up.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.is_active())
    }

    /// Subscribes one market data channel for one symbol. Before `connect` it is only recorded.
    ///
    /// While connected the registration is handed to the subscribe gate, which sends one frame
    /// a channel shortly after. A send failure is logged there, not returned here.
    ///
    /// # Errors
    ///
    /// Never fails today; the result is kept for callers.
    pub async fn subscribe(
        &self,
        channel: QfexMdsChannel,
        symbol: Ustr,
    ) -> Result<(), QfexWsClientError> {
        let mut subscriptions = self.session.subscriptions.lock().await;
        if !subscriptions.insert((channel, symbol)) {
            return Ok(());
        }
        drop(subscriptions);
        if self.client.is_some() {
            self.session.gate.queue(true, channel, symbol).await;
        }
        Ok(())
    }

    /// Unsubscribes one market data channel for one symbol. Dropping some symbols of a channel
    /// leaves the rest subscribed: the frame carries the dropped symbols only.
    ///
    /// # Errors
    ///
    /// Never fails today; the result is kept for callers.
    pub async fn unsubscribe(
        &self,
        channel: QfexMdsChannel,
        symbol: Ustr,
    ) -> Result<(), QfexWsClientError> {
        let mut subscriptions = self.session.subscriptions.lock().await;
        if !subscriptions.remove(&(channel, symbol)) {
            return Ok(());
        }
        drop(subscriptions);
        if self.client.is_some() {
            self.session.gate.queue(false, channel, symbol).await;
        }
        Ok(())
    }

    /// Sends one request on the trade socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket is not connected or the frame cannot be sent.
    pub async fn send(&self, request: &QfexTradeRequest) -> Result<(), QfexWsClientError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| QfexWsClientError::Client("not connected".to_string()))?;
        let frame = request.to_frame();
        client.send_text(frame.clone(), None).await?;
        self.session.record_sent(&frame);
        Ok(())
    }
}

async fn send_json(
    client: &WebSocketClient,
    request: &QfexMdsRequest<'_>,
) -> Result<(), QfexWsClientError> {
    let text =
        serde_json::to_string(request).map_err(|e| QfexWsClientError::Client(e.to_string()))?;
    Ok(client.send_text(text, None).await?)
}

/// First frames of a connection: `auth` on the trade socket, the recorded subscriptions on the
/// market data socket (one frame a channel).
async fn start_session(
    client: &WebSocketClient,
    session: &Session,
    epoch: u64,
) -> Result<(), QfexWsClientError> {
    if let Some(signer) = &session.signer {
        if client.connection_epoch() != epoch {
            log::warn!("QFEX WebSocket: reconnect notice of a replaced connection, no auth sent");
            return Ok(());
        }
        // A fresh nonce and timestamp every time: the venue refuses a reused nonce.
        let auth = signer
            .auth()
            .map_err(|e| QfexWsClientError::Client(e.to_string()))?;
        let frame = auth.ws_message();
        return match client
            .send_text_on_connection(frame.clone(), None, epoch)
            .await
        {
            Err(SendError::ConnectionChanged) => {
                log::warn!("QFEX WebSocket: connection replaced before auth, the next one logs in");
                Ok(())
            }
            sent => {
                sent?;
                session.record_sent(&frame);
                Ok(())
            }
        };
    }

    let subscriptions = session.subscriptions.lock().await;
    for channel in subscriptions
        .iter()
        .map(|(c, _)| *c)
        .collect::<BTreeSet<_>>()
    {
        let symbols: Vec<Ustr> = subscriptions
            .iter()
            .filter(|(c, _)| *c == channel)
            .map(|(_, symbol)| *symbol)
            .collect();
        let request = QfexMdsRequest::Subscribe {
            channels: &[channel],
            symbols: &symbols,
        };
        send_json(client, &request).await?;
    }
    Ok(())
}

/// The private channel frames, on the connection whose `auth` succeeded and no other: a frame
/// sent with `send_text` during a reconnect is replayed on the next socket before its `auth`.
async fn send_session_frames(client: &WebSocketClient, session: &Session, epoch: u64) {
    if client.connection_epoch() != epoch {
        log::warn!(
            "QFEX WebSocket: login answered on a closed connection, its session frames dropped"
        );
        return;
    }
    for frame in session.frames.iter() {
        match client
            .send_text_on_connection(frame.clone(), None, epoch)
            .await
        {
            Ok(()) => session.record_sent(frame),
            Err(e) => log::warn!("QFEX WebSocket: session frame not sent on its connection: {e}"),
        }
    }
}

async fn read_loop(
    mut raw_rx: mpsc::UnboundedReceiver<(u64, Message)>,
    out_tx: mpsc::UnboundedSender<QfexWsMessage>,
    client: Arc<WebSocketClient>,
    session: Session,
) {
    while let Some((epoch, message)) = raw_rx.recv().await {
        // Ping, pong and close frames are the transport's business.
        let Message::Text(text) = message else {
            continue;
        };

        if text.as_str() == RECONNECTED {
            session.gate.connection_lost().await;
            if out_tx.send(QfexWsMessage::Reconnected).is_err() {
                return;
            }
            if let Err(e) = start_session(&client, &session, epoch).await {
                log::warn!("QFEX WebSocket: session start after reconnect failed: {e}");
            }
            continue;
        }
        if let Some(recorder) = &session.trade_recorder {
            recorder.received(&text);
        }

        let message = match QfexWsMessage::parse(&text) {
            Ok(message) => message,
            Err(e) => {
                log::warn!("QFEX WebSocket: unreadable frame: {e}");
                continue;
            }
        };

        match &message {
            QfexWsMessage::Authenticated => send_session_frames(&client, &session, epoch).await,
            QfexWsMessage::Subscribed => session.gate.acknowledged().await,
            QfexWsMessage::Error(error) => session.gate.refused(&error.incoming_message).await,
            _ => {}
        }
        if out_tx.send(message).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use axum::{
        Router,
        extract::{
            State,
            ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        },
        response::Response,
        routing::get,
    };
    use nautilus_network::{SocketState, SocketStateSink, mode::ReconnectRequestOutcome};
    use serde_json::{Value, json};
    use tokio::sync::Notify;

    use super::*;
    use crate::{
        signing::tests::{API_KEY, NONCE, SIGNATURE, UNIX_TS, fixed_signer},
        websocket::messages::tests::fixture_lines,
    };

    /// A venue stand-in for both sockets: pings each new connection, acks `subscribe` (and
    /// sends one captured `bbo`), answers `auth` with success, drops the connection on demand.
    #[derive(Clone, Default)]
    struct MockVenue {
        connections: Arc<AtomicUsize>,
        /// (connection number, frame) for every text frame received.
        received: Arc<parking_lot::Mutex<Vec<(usize, Value)>>>,
        /// Query string of every upgrade request.
        queries: Arc<parking_lot::Mutex<Vec<String>>>,
        pongs: Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
        drop_connection: Arc<Notify>,
        /// While set, subscribe replies are kept back until `release_acks`.
        hold_acks: Arc<AtomicBool>,
        release_acks: Arc<Notify>,
        /// Channel name to refuse once, with the frame echoed in `incoming_message`.
        refuse: Arc<parking_lot::Mutex<Option<String>>>,
        defer_first_auth_then_drop: Arc<AtomicBool>,
        next_upgrade_gate: Arc<Mutex<()>>,
    }

    impl MockVenue {
        async fn start() -> (Self, String) {
            let venue = Self::default();
            let app =
                Router::new()
                    .route(
                        "/ws",
                        get(
                            |ws: WebSocketUpgrade,
                             uri: axum::http::Uri,
                             State(venue): State<Self>| async move {
                                drop(venue.next_upgrade_gate.lock().await);
                                venue
                                    .queries
                                    .lock()
                                    .push(uri.query().unwrap_or_default().to_string());
                                let response: Response =
                                    ws.on_upgrade(|socket| venue.serve(socket));
                                response
                            },
                        ),
                    )
                    .with_state(venue.clone());
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("ws://{}/ws", listener.local_addr().unwrap());
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            (venue, url)
        }

        async fn serve(self, mut socket: WebSocket) {
            let connection = self.connections.fetch_add(1, Ordering::SeqCst) + 1;
            socket
                .send(WsMessage::Ping(b"venue".as_slice().into()))
                .await
                .unwrap();
            let bbo = fixture_lines("mds")
                .into_iter()
                .find(|line| line.contains(r#""type":"bbo""#))
                .unwrap();
            let mut deferred: Vec<String> = Vec::new();
            loop {
                let message = tokio::select! {
                    () = self.drop_connection.notified() => return,
                    () = self.release_acks.notified() => {
                        for reply in deferred.drain(..) {
                            socket.send(WsMessage::Text(reply.into())).await.unwrap();
                        }
                        if self.defer_first_auth_then_drop.swap(false, Ordering::SeqCst) {
                            return;
                        }
                        continue;
                    }
                    message = socket.recv() => message,
                };
                match message {
                    Some(Ok(WsMessage::Text(text))) => {
                        let frame: Value = serde_json::from_str(&text).unwrap();
                        self.received.lock().push((connection, frame.clone()));
                        let replies = if frame["type"] == "auth"
                            && self.defer_first_auth_then_drop.load(Ordering::SeqCst)
                        {
                            deferred.push(json!({"subscribed": "fills"}).to_string());
                            deferred.push(json!({"type": "auth", "result": "success"}).to_string());
                            vec![]
                        } else if frame["type"] == "auth" {
                            vec![json!({"type": "auth", "result": "success"}).to_string()]
                        } else if frame["type"] == "subscribe" && frame["params"].is_null() {
                            if self.refuse_now(&frame) {
                                vec![
                                    json!({"type": "error", "message": "refused", "incoming_message": frame})
                                        .to_string(),
                                ]
                            } else {
                                let ack = json!({"type": "subscribed", "channels": frame["channels"], "symbols": frame["symbols"]});
                                let acks = vec![ack.to_string(), bbo.clone()];
                                if self.hold_acks.load(Ordering::SeqCst) {
                                    deferred.extend(acks);
                                    vec![]
                                } else {
                                    acks
                                }
                            }
                        } else {
                            vec![]
                        };
                        for reply in replies {
                            socket.send(WsMessage::Text(reply.into())).await.unwrap();
                        }
                    }
                    Some(Ok(WsMessage::Pong(payload))) => self.pongs.lock().push(payload.to_vec()),
                    Some(Ok(_)) => {}
                    _ => return,
                }
            }
        }

        /// Whether this subscribe frame is the one to refuse. The refusal is spent on reading.
        fn refuse_now(&self, frame: &Value) -> bool {
            let mut refuse = self.refuse.lock();
            let Some(name) = refuse.clone() else {
                return false;
            };
            let hit = frame["channels"]
                .as_array()
                .is_some_and(|channels| channels.iter().any(|c| c.as_str() == Some(&name)));
            if hit {
                *refuse = None;
            }
            hit
        }

        fn requests(&self, connection: usize) -> Vec<Value> {
            self.received
                .lock()
                .iter()
                .filter(|(c, _)| *c == connection)
                .map(|(_, frame)| frame.clone())
                .collect()
        }
    }

    async fn next(rx: &mut mpsc::UnboundedReceiver<QfexWsMessage>) -> QfexWsMessage {
        tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("timed out waiting for a frame")
            .expect("stream closed")
    }

    async fn wait_until(what: &str, condition: impl Fn() -> bool) {
        for _ in 0..1000 {
            if condition() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {what}");
    }

    fn aapl() -> Ustr {
        Ustr::from("AAPL-USD")
    }

    /// Every market data channel the adapter subscribes.
    const MDS_CHANNELS: [QfexMdsChannel; 6] = [
        QfexMdsChannel::Level2,
        QfexMdsChannel::Bbo,
        QfexMdsChannel::Trade,
        QfexMdsChannel::MarkPrice,
        QfexMdsChannel::Underlier,
        QfexMdsChannel::Funding,
    ];

    #[tokio::test]
    async fn mds_state_sink_sees_connect_loss_and_reconnect() {
        let (venue, url) = MockVenue::start().await;
        let states = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let seen = states.clone();
        let mut client = QfexWebSocketClient::new_mds(url)
            .with_state_sink(SocketStateSink::new(move |state| seen.lock().push(state)));
        let _rx = client.connect().await.unwrap();
        wait_until("connected", || states.lock().len() == 1).await;

        venue.drop_connection.notify_one();
        wait_until("reconnected", || states.lock().len() == 3).await;
        client.disconnect().await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert_eq!(
            *states.lock(),
            vec![
                SocketState::Connected,
                SocketState::Disconnected,
                SocketState::Connected,
            ],
            "a deliberate disconnect reports nothing"
        );
    }

    #[tokio::test]
    async fn mds_subscribe_batches_every_symbol_of_a_channel_into_one_frame() {
        let (venue, url) = MockVenue::start().await;
        let mut client = QfexWebSocketClient::new_mds(url);
        let _rx = client.connect().await.unwrap();

        let symbols: Vec<Ustr> = (0..32)
            .map(|i| Ustr::from(&format!("SYM{i:02}-USD")))
            .collect();
        for channel in MDS_CHANNELS {
            for symbol in &symbols {
                client.subscribe(channel, *symbol).await.unwrap();
            }
        }

        wait_until("one frame a channel", || {
            venue.requests(1).len() >= MDS_CHANNELS.len()
        })
        .await;
        let requests = venue.requests(1);
        assert_eq!(
            requests.len(),
            MDS_CHANNELS.len(),
            "192 registrations leave as one frame a channel"
        );
        for request in &requests {
            assert_eq!(request["type"], "subscribe");
            assert_eq!(request["channels"].as_array().unwrap().len(), 1);
            assert_eq!(
                request["symbols"].as_array().unwrap().len(),
                symbols.len(),
                "every symbol of the channel in one frame"
            );
        }
        client.disconnect().await;
    }

    #[tokio::test]
    async fn mds_subscribe_gate_holds_frames_past_the_inflight_cap() {
        let (venue, url) = MockVenue::start().await;
        venue.hold_acks.store(true, Ordering::SeqCst);
        let mut client = QfexWebSocketClient::new_mds(url);
        let _rx = client.connect().await.unwrap();

        assert!(MDS_CHANNELS.len() > SUBSCRIBE_INFLIGHT_MAX);
        for channel in MDS_CHANNELS {
            client.subscribe(channel, aapl()).await.unwrap();
        }
        wait_until("the first frames", || {
            venue.requests(1).len() >= SUBSCRIBE_INFLIGHT_MAX
        })
        .await;
        tokio::time::sleep(Duration::from_millis(5 * SUBSCRIBE_DEBOUNCE_MS)).await;
        assert_eq!(
            venue.requests(1).len(),
            SUBSCRIBE_INFLIGHT_MAX,
            "the rest wait for an acknowledgement"
        );

        venue.hold_acks.store(false, Ordering::SeqCst);
        venue.release_acks.notify_one();
        wait_until("the rest", || venue.requests(1).len() == MDS_CHANNELS.len()).await;
        client.disconnect().await;
    }

    #[tokio::test]
    async fn mds_subscribe_refused_by_the_venue_is_sent_again() {
        let (venue, url) = MockVenue::start().await;
        *venue.refuse.lock() = Some("level2".to_string());
        let mut client = QfexWebSocketClient::new_mds(url);
        let mut rx = client.connect().await.unwrap();

        client
            .subscribe(QfexMdsChannel::Level2, aapl())
            .await
            .unwrap();
        wait_until("the retry", || venue.requests(1).len() == 2).await;
        let frame = json!({"type": "subscribe", "channels": ["level2"], "symbols": ["AAPL-USD"]});
        assert_eq!(venue.requests(1), [frame.clone(), frame]);
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Error(_)));
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Subscribed));
        client.disconnect().await;
    }

    #[tokio::test]
    async fn mds_subscribe_delivers_ack_then_data_and_answers_ping() {
        let (venue, url) = MockVenue::start().await;
        let mut client = QfexWebSocketClient::new_mds(url);
        let mut rx = client.connect().await.unwrap();

        client.subscribe(QfexMdsChannel::Bbo, aapl()).await.unwrap();
        // A repeat is not sent again.
        client.subscribe(QfexMdsChannel::Bbo, aapl()).await.unwrap();

        assert!(matches!(next(&mut rx).await, QfexWsMessage::Subscribed));
        let QfexWsMessage::Bbo(book) = next(&mut rx).await else {
            panic!("expected a bbo frame");
        };
        assert_eq!(book.symbol, "AAPL-USD");
        assert_eq!(
            venue.requests(1),
            [json!({"type": "subscribe", "channels": ["bbo"], "symbols": ["AAPL-USD"]})]
        );
        assert_eq!(*venue.queries.lock(), [""], "no key on the public socket");

        wait_until("the pong", || !venue.pongs.lock().is_empty()).await;
        assert_eq!(venue.pongs.lock()[0], b"venue");

        client
            .unsubscribe(QfexMdsChannel::Bbo, aapl())
            .await
            .unwrap();
        wait_until("the unsubscribe", || venue.requests(1).len() == 2).await;
        assert_eq!(
            venue.requests(1)[1],
            json!({"type": "unsubscribe", "channels": ["bbo"], "symbols": ["AAPL-USD"]})
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn mds_reconnect_request_follows_the_connection() {
        let (venue, url) = MockVenue::start().await;
        let mut client = QfexWebSocketClient::new_mds(url);
        let request = client.reconnect_request();
        assert_eq!(request(), ReconnectRequestOutcome::Closed, "before connect");

        client
            .subscribe(QfexMdsChannel::Level2, aapl())
            .await
            .unwrap();
        let mut rx = client.connect().await.unwrap();
        wait_until("the subscribe", || venue.requests(1).len() == 1).await;

        assert_eq!(request(), ReconnectRequestOutcome::Accepted);
        loop {
            if matches!(next(&mut rx).await, QfexWsMessage::Reconnected) {
                break;
            }
        }
        wait_until("the resubscribe", || venue.requests(2).len() == 1).await;
        assert_eq!(venue.requests(2), venue.requests(1));

        client.disconnect().await;
        assert_eq!(
            request(),
            ReconnectRequestOutcome::Closed,
            "after disconnect"
        );
    }

    #[tokio::test]
    async fn mds_reconnect_resends_every_subscription_grouped_by_channel() {
        let (venue, url) = MockVenue::start().await;
        let mut client = QfexWebSocketClient::new_mds(url);
        // Recorded before connecting, sent by `connect`.
        client
            .subscribe(QfexMdsChannel::Level2, aapl())
            .await
            .unwrap();
        let mut rx = client.connect().await.unwrap();
        client
            .subscribe(QfexMdsChannel::Level2, Ustr::from("US500-USD"))
            .await
            .unwrap();
        client
            .subscribe(QfexMdsChannel::Trade, aapl())
            .await
            .unwrap();
        wait_until("three subscribes", || venue.requests(1).len() == 3).await;

        venue.drop_connection.notify_one();
        loop {
            if matches!(next(&mut rx).await, QfexWsMessage::Reconnected) {
                break;
            }
        }
        wait_until("the resubscribe", || venue.requests(2).len() == 2).await;
        assert_eq!(
            venue.requests(2),
            [
                json!({"type": "subscribe", "channels": ["level2"], "symbols": ["AAPL-USD", "US500-USD"]}),
                json!({"type": "subscribe", "channels": ["trade"], "symbols": ["AAPL-USD"]}),
            ]
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn trade_socket_authenticates_then_starts_the_session_again_after_reconnect() {
        let (venue, url) = MockVenue::start().await;
        let mut client = QfexWebSocketClient::new_trade(&url, fixed_signer(), true).unwrap();
        assert!(!format!("{client:?}").contains(API_KEY));
        let mut rx = client.connect().await.unwrap();

        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        let session = [
            json!({"type": "auth", "params": {"hmac": {"public_key": API_KEY, "nonce": NONCE, "unix_ts": UNIX_TS, "signature": SIGNATURE}}}),
            json!({"type": "subscribe", "params": {"channels": ["order_responses", "fills", "positions", "balances"]}}),
            json!({"type": "cancel_on_disconnect", "params": {"cancel_on_disconnect": true}}),
        ];
        wait_until("the session frames", || venue.requests(1).len() == 3).await;
        assert_eq!(venue.requests(1), session);
        assert_eq!(*venue.queries.lock(), [format!("api_key={API_KEY}")]);

        client
            .send(&QfexTradeRequest::CancelAllOrders { symbol: None })
            .await
            .unwrap();
        wait_until("the request", || venue.requests(1).len() == 4).await;
        assert_eq!(
            venue.requests(1)[3],
            json!({"type": "cancel_all_orders", "params": {}})
        );

        venue.drop_connection.notify_one();
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Reconnected));
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        wait_until("the second session", || venue.requests(2).len() == 3).await;
        assert_eq!(venue.requests(2), session);
        client.disconnect().await;
    }

    #[tokio::test]
    async fn trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket() {
        let (venue, url) = MockVenue::start().await;
        let states = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let seen = states.clone();
        let mut client = QfexWebSocketClient::new_trade(&url, fixed_signer(), false)
            .unwrap()
            .with_state_sink(SocketStateSink::new(move |state| seen.lock().push(state)));
        let mut rx = client.connect().await.unwrap();
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        wait_until("the first session", || venue.requests(1).len() == 2).await;

        let read_loop_held_at_reconnect = client.session.gate.state.lock().await;
        venue.drop_connection.notify_one();
        wait_until("the second socket", || {
            venue.connections.load(Ordering::SeqCst) == 2 && states.lock().len() == 3
        })
        .await;
        venue.drop_connection.notify_one();
        wait_until("the third socket", || {
            venue.connections.load(Ordering::SeqCst) == 3 && states.lock().len() == 5
        })
        .await;
        assert!(
            rx.try_recv().is_err(),
            "the read loop was not held at the reconnect notice"
        );
        drop(read_loop_held_at_reconnect);

        wait_until("the third session", || venue.requests(3).len() >= 2).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let kinds: Vec<Value> = venue
            .requests(3)
            .iter()
            .map(|frame| frame["type"].clone())
            .collect();
        assert_eq!(
            kinds,
            [json!("auth"), json!("subscribe")],
            "the live socket gets one auth and the session frames once"
        );
        assert!(
            venue.requests(2).is_empty(),
            "nothing went out on the second socket"
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in() {
        let (venue, url) = MockVenue::start().await;
        let states = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let seen = states.clone();
        let mut client = QfexWebSocketClient::new_trade(&url, fixed_signer(), false)
            .unwrap()
            .with_state_sink(SocketStateSink::new(move |state| seen.lock().push(state)));
        let mut rx = client.connect().await.unwrap();
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        wait_until("the first session", || venue.requests(1).len() == 2).await;
        let socket = client.client.clone().unwrap();

        let next_upgrade_held = venue.next_upgrade_gate.lock().await;
        venue.drop_connection.notify_one();
        wait_until("the reconnect", || !socket.is_active()).await;
        assert_eq!(socket.connection_epoch(), 0);
        let session = client.session.clone();
        let waiting = socket.clone();
        let auth = tokio::spawn(async move { start_session(&waiting, &session, 0).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !auth.is_finished(),
            "the auth did not wait for the next connection"
        );
        drop(next_upgrade_held);

        assert!(
            auth.await.unwrap().is_ok(),
            "a replaced connection is handed to the next one"
        );
        loop {
            if matches!(next(&mut rx).await, QfexWsMessage::Reconnected) {
                break;
            }
        }
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        wait_until("the second session", || venue.requests(2).len() >= 2).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let kinds: Vec<Value> = venue
            .requests(2)
            .iter()
            .map(|frame| frame["type"].clone())
            .collect();
        assert_eq!(
            kinds,
            [json!("auth"), json!("subscribe")],
            "the next socket gets its own auth only, and the session frames once"
        );
        assert_eq!(socket.connection_epoch(), 1);
        client.disconnect().await;
    }

    #[tokio::test]
    async fn trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth() {
        let (venue, url) = MockVenue::start().await;
        venue
            .defer_first_auth_then_drop
            .store(true, Ordering::SeqCst);
        let states = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let seen = states.clone();
        let mut client = QfexWebSocketClient::new_trade(&url, fixed_signer(), false)
            .unwrap()
            .with_state_sink(SocketStateSink::new(move |state| seen.lock().push(state)));
        let mut rx = client.connect().await.unwrap();
        wait_until("the first auth", || venue.requests(1).len() == 1).await;

        let read_loop_held_at_subscribe_gate = client.session.gate.state.lock().await;
        venue.release_acks.notify_one();
        wait_until("the next socket", || {
            venue.connections.load(Ordering::SeqCst) == 2 && states.lock().len() == 3
        })
        .await;
        assert!(
            rx.try_recv().is_err(),
            "the read loop was not held at the subscribed frame"
        );
        drop(read_loop_held_at_subscribe_gate);

        loop {
            if matches!(next(&mut rx).await, QfexWsMessage::Reconnected) {
                break;
            }
        }
        assert!(matches!(next(&mut rx).await, QfexWsMessage::Authenticated));
        wait_until("the second session", || venue.requests(2).len() >= 2).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let kinds: Vec<Value> = venue
            .requests(2)
            .iter()
            .map(|frame| frame["type"].clone())
            .collect();
        assert_eq!(
            kinds,
            [json!("auth"), json!("subscribe")],
            "the next socket gets its own auth first, and the session frames once"
        );
        assert_eq!(
            venue.requests(1).len(),
            1,
            "only the auth went out on the first socket"
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn send_without_a_connection_is_an_error() {
        let client =
            QfexWebSocketClient::new_trade("ws://127.0.0.1:1/ws", fixed_signer(), false).unwrap();
        let error = client
            .send(&QfexTradeRequest::CancelAllOrders { symbol: None })
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "client error: not connected");
    }
}
