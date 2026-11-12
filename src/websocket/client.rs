//! One WebSocket client for both QFEX sockets.
//!
//! Market data (`mds`): no credentials; subscriptions are recorded and sent again after a
//! reconnect. Trade: sends the HMAC `auth` frame after every connect and reconnect and, once the
//! venue answers `success`, the session frames (channel subscriptions, cancel on disconnect).

use std::{collections::BTreeSet, fmt::Debug, sync::Arc};

use nautilus_common::live::get_runtime;
use nautilus_network::{
    RECONNECTED,
    websocket::{WebSocketClient, WebSocketConfig, channel_message_handler},
};
use strum::IntoEnumIterator;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::Message;
use ustr::Ustr;

use super::{
    error::QfexWsClientError,
    messages::{QfexMdsRequest, QfexTradeRequest, QfexWsMessage},
};
use crate::{
    common::enums::{QfexMdsChannel, QfexTradeChannel},
    signing::QfexSigner,
};

/// Client ping frames. After `auth` the venue also pings; the transport answers those.
const HEARTBEAT_INTERVAL_SECS: u64 = 20;
const RECONNECT_DELAY_INITIAL_MS: u64 = 250;
const RECONNECT_DELAY_MAX_MS: u64 = 30_000;
const RECONNECT_BACKOFF_FACTOR: f64 = 2.0;
const RECONNECT_JITTER_MS: u64 = 200;

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
}

/// A QFEX WebSocket connection that reconnects on its own.
pub struct QfexWebSocketClient {
    url: String,
    session: Session,
    client: Option<Arc<WebSocketClient>>,
    task: Option<JoinHandle<()>>,
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
            },
            client: None,
            task: None,
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
            },
            client: None,
            task: None,
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
        let (handler, raw_rx) = channel_message_handler();
        let client = WebSocketClient::builder()
            .config(config)
            .message_handler(handler)
            .connect()
            .await
            .map_err(|e| QfexWsClientError::Network(e.to_string()))?;
        let client = Arc::new(client);

        // The read task starts after this so a reconnect cannot interleave with the first send.
        if let Err(e) = start_session(&client, &self.session).await {
            client.disconnect().await;
            return Err(e);
        }
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        self.client = Some(client.clone());
        self.task =
            Some(get_runtime().spawn(read_loop(raw_rx, out_tx, client, self.session.clone())));

        Ok(out_rx)
    }

    /// Closes the connection and stops the read task.
    pub async fn disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            client.disconnect().await;
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }

    /// Whether the connection is up.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.is_active())
    }

    /// Subscribes one market data channel for one symbol. Before `connect` it is only recorded.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame cannot be sent.
    pub async fn subscribe(
        &self,
        channel: QfexMdsChannel,
        symbol: Ustr,
    ) -> Result<(), QfexWsClientError> {
        let mut subscriptions = self.session.subscriptions.lock().await;
        if !subscriptions.insert((channel, symbol)) {
            return Ok(());
        }
        match &self.client {
            Some(client) => {
                let request = QfexMdsRequest::Subscribe {
                    channels: &[channel],
                    symbols: &[symbol],
                };
                send_json(client, &request).await
            }
            None => Ok(()),
        }
    }

    /// Unsubscribes one market data channel for one symbol.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame cannot be sent.
    pub async fn unsubscribe(
        &self,
        channel: QfexMdsChannel,
        symbol: Ustr,
    ) -> Result<(), QfexWsClientError> {
        let mut subscriptions = self.session.subscriptions.lock().await;
        if !subscriptions.remove(&(channel, symbol)) {
            return Ok(());
        }
        match &self.client {
            Some(client) => {
                let request = QfexMdsRequest::Unsubscribe {
                    channels: &[channel],
                    symbols: &[symbol],
                };
                send_json(client, &request).await
            }
            None => Ok(()),
        }
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
        Ok(client.send_text(request.to_frame(), None).await?)
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
) -> Result<(), QfexWsClientError> {
    if let Some(signer) = &session.signer {
        // A fresh nonce and timestamp every time: the venue refuses a reused nonce.
        let auth = signer
            .auth()
            .map_err(|e| QfexWsClientError::Client(e.to_string()))?;
        return Ok(client.send_text(auth.ws_message(), None).await?);
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

async fn read_loop(
    mut raw_rx: mpsc::UnboundedReceiver<Message>,
    out_tx: mpsc::UnboundedSender<QfexWsMessage>,
    client: Arc<WebSocketClient>,
    session: Session,
) {
    while let Some(message) = raw_rx.recv().await {
        // Ping, pong and close frames are the transport's business.
        let Message::Text(text) = message else {
            continue;
        };

        if text.as_str() == RECONNECTED {
            if out_tx.send(QfexWsMessage::Reconnected).is_err() {
                return;
            }
            if let Err(e) = start_session(&client, &session).await {
                log::warn!("QFEX WebSocket: session start after reconnect failed: {e}");
            }
            continue;
        }

        let message = match QfexWsMessage::parse(&text) {
            Ok(message) => message,
            Err(e) => {
                log::warn!("QFEX WebSocket: unreadable frame: {e}");
                continue;
            }
        };

        if matches!(message, QfexWsMessage::Authenticated) {
            for frame in session.frames.iter() {
                if let Err(e) = client.send_text(frame.clone(), None).await {
                    log::warn!("QFEX WebSocket: session frame not sent: {e}");
                }
            }
        }
        if out_tx.send(message).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use axum::{
        Router,
        extract::{
            State,
            ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        },
        response::Response,
        routing::get,
    };
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
            loop {
                let message = tokio::select! {
                    () = self.drop_connection.notified() => return,
                    message = socket.recv() => message,
                };
                match message {
                    Some(Ok(WsMessage::Text(text))) => {
                        let frame: Value = serde_json::from_str(&text).unwrap();
                        self.received.lock().push((connection, frame.clone()));
                        let replies = if frame["type"] == "auth" {
                            vec![json!({"type": "auth", "result": "success"}).to_string()]
                        } else if frame["type"] == "subscribe" && frame["params"].is_null() {
                            let ack = json!({"type": "subscribed", "channels": frame["channels"], "symbols": frame["symbols"]});
                            vec![ack.to_string(), bbo.clone()]
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
