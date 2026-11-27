//! Live market data client.
//!
//! One public WebSocket (`mds`) feeds books, quotes, trades, mark and index prices and funding
//! rates. Bars poll REST once per bar close, and the instrument status polls `/refdata`: the
//! socket has no channel for the status. Nothing here needs credentials.

use std::{future::Future, sync::Arc, time::Duration};

use ahash::AHashMap;
use anyhow::Context;
use dashmap::{DashMap, DashSet};
use nautilus_common::{
    cache::InstrumentLookupError,
    clients::{DataClient, execute_bar_subscriptions, execute_instrument_subscriptions},
    live::{get_runtime, runner::get_data_event_sender, sender::EventSender},
    messages::{
        DataEvent,
        data::{
            BarsResponse, BookResponse, DataResponse, FundingRatesResponse, InstrumentResponse,
            InstrumentsResponse, RequestBars, RequestBookSnapshot, RequestFundingRates,
            RequestInstrument, RequestInstruments, RequestTrades, SubscribeBars,
            SubscribeBookDeltas, SubscribeBookDepth, SubscribeFundingRates, SubscribeIndexPrices,
            SubscribeInstrument, SubscribeInstrumentStatus, SubscribeInstruments,
            SubscribeMarkPrices, SubscribeQuotes, SubscribeTrades, UnsubscribeBars,
            UnsubscribeBookDeltas, UnsubscribeBookDepth, UnsubscribeFundingRates,
            UnsubscribeIndexPrices, UnsubscribeInstrument, UnsubscribeInstrumentStatus,
            UnsubscribeInstruments, UnsubscribeMarkPrices, UnsubscribeQuotes, UnsubscribeTrades,
        },
    },
};
use nautilus_core::{
    UnixNanos,
    datetime::datetime_to_unix_nanos,
    time::{AtomicTime, get_atomic_clock_realtime},
};
use nautilus_live::{SocketControl, SocketControlFactory};
use nautilus_model::{
    data::{BarType, Data, InstrumentStatus},
    enums::{BookType, MarketStatusAction},
    identifiers::{ClientId, InstrumentId, Venue},
    instruments::{Instrument, InstrumentAny},
    orderbook::OrderBook,
};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use ustr::Ustr;

use crate::{
    common::{
        consts::QFEX_VENUE,
        enums::{QfexBarInterval, QfexMdsChannel, QfexSymbolStatus},
        parse::{instrument_id_of, venue_symbol_of},
        urls::qfex_mds_ws_url,
    },
    config::data::QfexDataClientConfig,
    http::{client::QfexHttpClient, models::QfexSymbol},
    websocket::{
        QfexWebSocketClient, QfexWsClientError, QfexWsMessage, handler::parse_mds_message,
    },
};

/// Endpoint label of the market data socket in `SocketStateChanged`.
const MARKET_DATA_ENDPOINT: &str = "market_data";

/// The venue closes a bar some time after the interval ends.
const BAR_POLL_DELAY: Duration = Duration::from_secs(2);
// shortcut: the status is read from the whole `/refdata` once a minute, so a suspension is
// seen up to 60 s late; use a status channel if the venue adds one (`refdata` is not a
// channel of the socket, 12 Nov 2026).
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(60);

/// What a strategy subscribed to. Deltas and depth share the `level2` channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Deltas,
    Depth,
    Quotes,
    Trades,
    MarkPrices,
    IndexPrices,
    FundingRates,
}

impl Kind {
    const ALL: [Self; 7] = [
        Self::Deltas,
        Self::Depth,
        Self::Quotes,
        Self::Trades,
        Self::MarkPrices,
        Self::IndexPrices,
        Self::FundingRates,
    ];

    const fn channel(self) -> QfexMdsChannel {
        match self {
            Self::Deltas | Self::Depth => QfexMdsChannel::Level2,
            Self::Quotes => QfexMdsChannel::Bbo,
            Self::Trades => QfexMdsChannel::Trade,
            Self::MarkPrices => QfexMdsChannel::MarkPrice,
            Self::IndexPrices => QfexMdsChannel::Underlier,
            Self::FundingRates => QfexMdsChannel::Funding,
        }
    }

    fn of(data: &Data) -> Option<(Self, InstrumentId)> {
        match data {
            Data::BookDeltas(deltas) => Some((Self::Deltas, deltas.instrument_id)),
            Data::BookDepth(depth) => Some((Self::Depth, depth.instrument_id)),
            Data::Quote(quote) => Some((Self::Quotes, quote.instrument_id)),
            Data::Trade(trade) => Some((Self::Trades, trade.instrument_id)),
            Data::MarkPrice(mark) => Some((Self::MarkPrices, mark.instrument_id)),
            Data::IndexPrice(index) => Some((Self::IndexPrices, index.instrument_id)),
            Data::FundingRate(rate) => Some((Self::FundingRates, rate.instrument_id)),
            _ => None,
        }
    }
}

/// Returns the first bar close after `now` for epoch-aligned bars of `interval` nanoseconds.
const fn next_bar_close(now: u64, interval: u64) -> u64 {
    (now / interval + 1) * interval
}

/// State shared by the client and its tasks.
#[derive(Debug, Clone)]
struct Feed {
    http: QfexHttpClient,
    // `connect` and `disconnect` of the WebSocket client need `&mut self`
    ws: Arc<tokio::sync::Mutex<QfexWebSocketClient>>,
    socket: Arc<SocketControl>,
    instruments: Arc<DashMap<InstrumentId, InstrumentAny>>,
    subscriptions: Arc<DashSet<(Kind, InstrumentId)>>,
    /// Instruments with a status subscription, and the status last sent for each.
    statuses: Arc<DashMap<InstrumentId, Option<QfexSymbolStatus>>>,
    sender: EventSender<DataEvent>,
    clock: &'static AtomicTime,
}

impl Feed {
    fn send(&self, data: Data) {
        if let Err(e) = self.sender.send(DataEvent::Data(data)) {
            log::error!("Failed to send QFEX data: {e}");
        }
    }

    fn wants_channel(&self, channel: QfexMdsChannel, instrument_id: InstrumentId) -> bool {
        Kind::ALL.iter().any(|kind| {
            kind.channel() == channel && self.subscriptions.contains(&(*kind, instrument_id))
        })
    }

    async fn set_channel(
        &self,
        channel: QfexMdsChannel,
        symbol: Ustr,
        subscribe: bool,
    ) -> Result<(), QfexWsClientError> {
        let ws = self.ws.lock().await;
        if subscribe {
            ws.subscribe(channel, symbol).await
        } else {
            ws.unsubscribe(channel, symbol).await
        }
    }

    fn instrument_of(&self, symbol: &Ustr) -> Option<InstrumentAny> {
        let instrument_id = instrument_id_of(symbol.as_str()).ok()?;
        self.instruments.get(&instrument_id).map(|i| i.clone())
    }

    /// Consumes the WebSocket stream until it ends.
    async fn run(self, mut rx: UnboundedReceiver<QfexWsMessage>) {
        while let Some(message) = rx.recv().await {
            match &message {
                QfexWsMessage::Reconnected => log::info!("QFEX market data socket reconnected"),
                QfexWsMessage::Error(error) => log::error!(
                    "QFEX market data socket error {:?}: {}",
                    error.error_code,
                    error.message
                ),
                _ => {}
            }

            let now = self.clock.get_time_ns();
            match parse_mds_message(&message, |symbol| self.instrument_of(symbol), now) {
                Ok(data) => {
                    for item in data {
                        if Kind::of(&item).is_some_and(|key| self.subscriptions.contains(&key)) {
                            self.send(item);
                        }
                    }
                }
                Err(e) => log::warn!("QFEX market data frame dropped: {e}"),
            }
        }
        log::debug!("QFEX market data stream ended");
    }

    /// Emits each closed bar of `bar_type`: one REST request per bar close.
    async fn poll_bars(self, instrument: InstrumentAny, bar_type: BarType, interval: u64) {
        loop {
            let now = self.clock.get_time_ns().as_u64();
            let close = next_bar_close(now, interval);
            tokio::time::sleep(Duration::from_nanos(close - now) + BAR_POLL_DELAY).await;

            // The venue stamps a candle with its open time (`startedAt`)
            let open = UnixNanos::from(close - interval);
            match self
                .http
                .request_bars(
                    bar_type,
                    &instrument,
                    Some(open),
                    Some(UnixNanos::from(close)),
                    None,
                )
                .await
            {
                Ok(bars) => match bars.into_iter().find(|bar| bar.ts_event == open) {
                    Some(bar) => self.send(Data::Bar(bar)),
                    // No trade in the interval, or the candle is late
                    None => log::warn!("QFEX returned no {bar_type} bar opened at {open}"),
                },
                Err(e) => log::error!("QFEX bar poll failed for {bar_type}: {e}"),
            }
        }
    }

    /// Emits the status of each subscribed instrument when it is first read and when it
    /// changes. The time of the event is the time of the poll: `/refdata` has none.
    async fn poll_statuses(self) {
        loop {
            match self.http.get_refdata().await {
                Ok(refdata) => {
                    let now = self.clock.get_time_ns();
                    for item in refdata.data {
                        let Ok(symbol) = serde_json::from_value::<QfexSymbol>(item) else {
                            continue;
                        };
                        let Ok(instrument_id) = instrument_id_of(symbol.symbol.as_str()) else {
                            continue;
                        };
                        let Some(mut last) = self.statuses.get_mut(&instrument_id) else {
                            continue;
                        };
                        let Some(action) = symbol.status.as_market_status_action() else {
                            continue;
                        };
                        if last.replace(symbol.status) != Some(symbol.status) {
                            self.send(Data::InstrumentStatus(InstrumentStatus::new(
                                instrument_id,
                                action,
                                now,
                                now,
                                None,
                                None,
                                Some(action == MarketStatusAction::Trading),
                                None,
                                None,
                            )));
                        }
                    }
                }
                Err(e) => log::warn!("QFEX instrument status poll failed: {e}"),
            }
            tokio::time::sleep(STATUS_POLL_INTERVAL).await;
        }
    }

    /// Disconnects the socket and forgets every subscription.
    async fn close(self, ws_url: String) {
        let mut ws = self.ws.lock().await;
        ws.disconnect().await;
        // The client keeps its recorded subscriptions for the next connect: start a new one
        *ws = QfexWebSocketClient::new_mds(ws_url).with_state_sink(self.socket.sink());
        self.subscriptions.clear();
        self.statuses.clear();
    }
}

/// Market data client for QFEX.
#[derive(Debug)]
pub struct QfexDataClient {
    client_id: ClientId,
    config: QfexDataClientConfig,
    feed: Feed,
    feed_task: Option<JoinHandle<()>>,
    bar_tasks: AHashMap<BarType, JoinHandle<()>>,
    status_task: Option<JoinHandle<()>>,
    close_task: Option<JoinHandle<()>>,
    is_connected: bool,
}

impl QfexDataClient {
    /// Creates a new [`QfexDataClient`]. Nothing is connected until [`DataClient::connect`].
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(client_id: ClientId, config: QfexDataClientConfig) -> anyhow::Result<Self> {
        let http = QfexHttpClient::new(
            config.environment,
            config.base_url_http.clone(),
            config.api_version.clone(),
            config.http_timeout_secs,
            config.proxy_url.clone(),
            None,
        )?;
        let socket =
            SocketControlFactory::new(client_id, Some(*QFEX_VENUE)).control(MARKET_DATA_ENDPOINT);
        let feed = Feed {
            http,
            ws: Arc::new(tokio::sync::Mutex::new(
                QfexWebSocketClient::new_mds(ws_url(&config)).with_state_sink(socket.sink()),
            )),
            socket: Arc::new(socket),
            instruments: Arc::new(DashMap::new()),
            subscriptions: Arc::new(DashSet::new()),
            statuses: Arc::new(DashMap::new()),
            sender: get_data_event_sender(),
            clock: get_atomic_clock_realtime(),
        };

        Ok(Self {
            client_id,
            config,
            feed,
            feed_task: None,
            bar_tasks: AHashMap::new(),
            status_task: None,
            close_task: None,
            is_connected: false,
        })
    }

    fn instrument(&self, instrument_id: &InstrumentId) -> anyhow::Result<InstrumentAny> {
        self.feed
            .instruments
            .get(instrument_id)
            .map(|instrument| instrument.clone())
            .ok_or_else(|| InstrumentLookupError::not_found(*instrument_id).into())
    }

    fn spawn_channel(&self, channel: QfexMdsChannel, symbol: Ustr, subscribe: bool) {
        let feed = self.feed.clone();
        get_runtime().spawn(async move {
            if let Err(e) = feed.set_channel(channel, symbol, subscribe).await {
                log::error!(
                    "Failed to set QFEX {channel:?} subscription of {symbol} to {subscribe}: {e}"
                );
            }
        });
    }

    fn subscribe_kind(&self, kind: Kind, instrument_ids: &[InstrumentId]) -> anyhow::Result<()> {
        execute_instrument_subscriptions(instrument_ids, |instrument_id| {
            self.instrument(&instrument_id)?;
            let symbol = Ustr::from(venue_symbol_of(&instrument_id)?);
            let channel = kind.channel();
            let first = !self.feed.wants_channel(channel, instrument_id);
            self.feed.subscriptions.insert((kind, instrument_id));

            if first {
                self.spawn_channel(channel, symbol, true);
            }
            Ok(())
        })
    }

    fn unsubscribe_kind(&self, kind: Kind, instrument_ids: &[InstrumentId]) -> anyhow::Result<()> {
        execute_instrument_subscriptions(instrument_ids, |instrument_id| {
            let channel = kind.channel();
            if self
                .feed
                .subscriptions
                .remove(&(kind, instrument_id))
                .is_none()
                || self.feed.wants_channel(channel, instrument_id)
            {
                return Ok(());
            }

            let symbol = Ustr::from(venue_symbol_of(&instrument_id)?);
            self.spawn_channel(channel, symbol, false);
            Ok(())
        })
    }

    fn send_cached_instruments(&self, instrument_id: Option<&InstrumentId>) {
        for entry in self.feed.instruments.iter() {
            if instrument_id.is_none_or(|id| id == entry.key()) {
                self.feed.send(entry.value().clone().into());
            }
        }
    }

    fn spawn_request(
        &self,
        what: &'static str,
        request: impl Future<Output = anyhow::Result<DataResponse>> + Send + 'static,
    ) {
        let sender = self.feed.sender.clone();
        get_runtime().spawn(async move {
            match request.await {
                Ok(response) => {
                    if let Err(e) = sender.send(DataEvent::Response(response)) {
                        log::error!("Failed to send QFEX {what} response: {e}");
                    }
                }
                Err(e) => log::error!("QFEX {what} request failed: {e:?}"),
            }
        });
    }

    fn abort_tasks(&mut self) {
        if let Some(task) = self.feed_task.take() {
            task.abort();
        }
        for (_, task) in self.bar_tasks.drain() {
            task.abort();
        }
        if let Some(task) = self.status_task.take() {
            task.abort();
        }
        self.is_connected = false;
    }

    fn stop_in_background(&mut self) {
        self.abort_tasks();
        self.close_task = Some(get_runtime().spawn(self.feed.clone().close(ws_url(&self.config))));
    }
}

fn ws_url(config: &QfexDataClientConfig) -> String {
    config
        .base_url_ws
        .clone()
        .unwrap_or_else(|| qfex_mds_ws_url(config.environment).to_string())
}

fn validate_book_type(book_type: BookType) -> anyhow::Result<()> {
    anyhow::ensure!(
        book_type == BookType::L2_MBP,
        "QFEX serves L2_MBP books only, was {book_type}"
    );
    Ok(())
}

#[async_trait::async_trait(?Send)]
impl DataClient for QfexDataClient {
    fn client_id(&self) -> ClientId {
        self.client_id
    }

    fn venue(&self) -> Option<Venue> {
        Some(*QFEX_VENUE)
    }

    fn start(&mut self) -> anyhow::Result<()> {
        log::info!(
            "Starting QFEX data client {} ({})",
            self.client_id,
            self.config.environment
        );
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        self.stop_in_background();
        Ok(())
    }

    fn reset(&mut self) -> anyhow::Result<()> {
        self.stop_in_background();
        Ok(())
    }

    fn dispose(&mut self) -> anyhow::Result<()> {
        self.stop_in_background();
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn is_disconnected(&self) -> bool {
        !self.is_connected
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        if self.is_connected {
            return Ok(());
        }
        // A stop still closing the socket must not close the new connection
        if let Some(task) = self.close_task.take() {
            let _ = task.await;
        }

        let instruments = self
            .feed
            .http
            .request_instruments()
            .await
            .context("failed to load QFEX instruments")?;
        let count = instruments.len();
        for instrument in instruments {
            self.feed
                .instruments
                .insert(instrument.id(), instrument.clone());
            self.feed.send(instrument.into());
        }

        let rx = self
            .feed
            .ws
            .lock()
            .await
            .connect()
            .await
            .context("failed to connect QFEX market data WebSocket")?;
        self.feed_task = Some(get_runtime().spawn(self.feed.clone().run(rx)));
        self.is_connected = true;
        log::info!(
            "Connected QFEX data client {}: {count} instruments",
            self.client_id
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.abort_tasks();
        if let Some(task) = self.close_task.take() {
            let _ = task.await;
        }
        self.feed.clone().close(ws_url(&self.config)).await;
        log::info!("Disconnected QFEX data client {}", self.client_id);
        Ok(())
    }

    fn subscribe_instruments(&mut self, _cmd: SubscribeInstruments) -> anyhow::Result<()> {
        // Instruments are loaded at connect: replay them
        self.send_cached_instruments(None);
        Ok(())
    }

    fn subscribe_instrument(&mut self, cmd: SubscribeInstrument) -> anyhow::Result<()> {
        execute_instrument_subscriptions(&cmd.instrument_ids, |instrument_id| {
            self.instrument(&instrument_id)?;
            self.send_cached_instruments(Some(&instrument_id));
            Ok(())
        })
    }

    fn subscribe_book_deltas(&mut self, cmd: SubscribeBookDeltas) -> anyhow::Result<()> {
        validate_book_type(cmd.book_type)?;
        self.subscribe_kind(Kind::Deltas, &cmd.instrument_ids)
    }

    fn subscribe_book_depth(&mut self, cmd: SubscribeBookDepth) -> anyhow::Result<()> {
        validate_book_type(cmd.book_type)?;
        self.subscribe_kind(Kind::Depth, &cmd.instrument_ids)
    }

    fn subscribe_quotes(&mut self, cmd: SubscribeQuotes) -> anyhow::Result<()> {
        self.subscribe_kind(Kind::Quotes, &cmd.instrument_ids)
    }

    fn subscribe_trades(&mut self, cmd: SubscribeTrades) -> anyhow::Result<()> {
        self.subscribe_kind(Kind::Trades, &cmd.instrument_ids)
    }

    fn subscribe_mark_prices(&mut self, cmd: SubscribeMarkPrices) -> anyhow::Result<()> {
        self.subscribe_kind(Kind::MarkPrices, &cmd.instrument_ids)
    }

    fn subscribe_index_prices(&mut self, cmd: SubscribeIndexPrices) -> anyhow::Result<()> {
        self.subscribe_kind(Kind::IndexPrices, &cmd.instrument_ids)
    }

    fn subscribe_funding_rates(&mut self, cmd: SubscribeFundingRates) -> anyhow::Result<()> {
        self.subscribe_kind(Kind::FundingRates, &cmd.instrument_ids)
    }

    fn subscribe_instrument_status(
        &mut self,
        cmd: SubscribeInstrumentStatus,
    ) -> anyhow::Result<()> {
        let result = execute_instrument_subscriptions(&cmd.instrument_ids, |instrument_id| {
            self.instrument(&instrument_id)?;
            self.feed.statuses.entry(instrument_id).or_insert(None);
            Ok(())
        });
        if !self.feed.statuses.is_empty() {
            // Started again, so that the new subscriber gets its status now and not at the
            // next poll. What was sent before is kept: nothing is sent twice.
            let task = get_runtime().spawn(self.feed.clone().poll_statuses());
            if let Some(old) = self.status_task.replace(task) {
                old.abort();
            }
        }
        result
    }

    fn subscribe_bars(&mut self, cmd: SubscribeBars) -> anyhow::Result<()> {
        execute_bar_subscriptions(&cmd.bar_types, |bar_type| {
            let interval = QfexBarInterval::try_from(&bar_type)?;
            let instrument = self.instrument(&bar_type.instrument_id())?;

            let task = get_runtime().spawn(self.feed.clone().poll_bars(
                instrument,
                bar_type,
                interval.as_nanos(),
            ));
            if let Some(old) = self.bar_tasks.insert(bar_type, task) {
                old.abort();
            }
            Ok(())
        })
    }

    fn unsubscribe_instruments(&mut self, _cmd: &UnsubscribeInstruments) -> anyhow::Result<()> {
        Ok(())
    }

    fn unsubscribe_instrument(&mut self, _cmd: &UnsubscribeInstrument) -> anyhow::Result<()> {
        Ok(())
    }

    fn unsubscribe_book_deltas(&mut self, cmd: &UnsubscribeBookDeltas) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::Deltas, &cmd.instrument_ids)
    }

    fn unsubscribe_book_depth(&mut self, cmd: &UnsubscribeBookDepth) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::Depth, &cmd.instrument_ids)
    }

    fn unsubscribe_quotes(&mut self, cmd: &UnsubscribeQuotes) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::Quotes, &cmd.instrument_ids)
    }

    fn unsubscribe_trades(&mut self, cmd: &UnsubscribeTrades) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::Trades, &cmd.instrument_ids)
    }

    fn unsubscribe_mark_prices(&mut self, cmd: &UnsubscribeMarkPrices) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::MarkPrices, &cmd.instrument_ids)
    }

    fn unsubscribe_index_prices(&mut self, cmd: &UnsubscribeIndexPrices) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::IndexPrices, &cmd.instrument_ids)
    }

    fn unsubscribe_funding_rates(&mut self, cmd: &UnsubscribeFundingRates) -> anyhow::Result<()> {
        self.unsubscribe_kind(Kind::FundingRates, &cmd.instrument_ids)
    }

    fn unsubscribe_instrument_status(
        &mut self,
        cmd: &UnsubscribeInstrumentStatus,
    ) -> anyhow::Result<()> {
        for instrument_id in &cmd.instrument_ids {
            self.feed.statuses.remove(instrument_id);
        }
        if self.feed.statuses.is_empty()
            && let Some(task) = self.status_task.take()
        {
            task.abort();
        }
        Ok(())
    }

    fn unsubscribe_bars(&mut self, cmd: &UnsubscribeBars) -> anyhow::Result<()> {
        execute_bar_subscriptions(&cmd.bar_types, |bar_type| {
            if let Some(task) = self.bar_tasks.remove(&bar_type) {
                task.abort();
            }
            Ok(())
        })
    }

    fn request_instruments(&self, request: RequestInstruments) -> anyhow::Result<()> {
        let feed = self.feed.clone();
        let client_id = request.client_id.unwrap_or(self.client_id);

        self.spawn_request("instruments", async move {
            let instruments = feed.http.request_instruments().await?;
            for instrument in &instruments {
                feed.instruments.insert(instrument.id(), instrument.clone());
            }
            Ok(DataResponse::Instruments(InstrumentsResponse::new(
                request.request_id,
                client_id,
                *QFEX_VENUE,
                instruments,
                datetime_to_unix_nanos(request.start),
                datetime_to_unix_nanos(request.end),
                feed.clock.get_time_ns(),
                request.params,
            )))
        });
        Ok(())
    }

    fn request_instrument(&self, request: RequestInstrument) -> anyhow::Result<()> {
        let instrument_id = request.single_instrument_id()?;
        let feed = self.feed.clone();
        let client_id = request.client_id.unwrap_or(self.client_id);

        self.spawn_request("instrument", async move {
            let instrument = feed
                .http
                .request_instruments()
                .await?
                .into_iter()
                .find(|instrument| instrument.id() == instrument_id)
                .ok_or_else(|| InstrumentLookupError::not_found(instrument_id))?;
            feed.instruments.insert(instrument_id, instrument.clone());
            Ok(DataResponse::Instrument(Box::new(InstrumentResponse::new(
                request.request_id,
                client_id,
                instrument_id,
                instrument,
                datetime_to_unix_nanos(request.start),
                datetime_to_unix_nanos(request.end),
                feed.clock.get_time_ns(),
                request.params,
            ))))
        });
        Ok(())
    }

    fn request_book_snapshot(&self, request: RequestBookSnapshot) -> anyhow::Result<()> {
        let instrument_id = request.single_instrument_id()?;
        let instrument = self.instrument(&instrument_id)?;
        let feed = self.feed.clone();
        let client_id = request.client_id.unwrap_or(self.client_id);

        self.spawn_request("book snapshot", async move {
            let deltas = feed.http.request_book_snapshot(&instrument).await?;
            let mut book = OrderBook::new(instrument_id, BookType::L2_MBP);
            book.apply_deltas(&deltas)?;
            Ok(DataResponse::Book(BookResponse::new(
                request.request_id,
                client_id,
                instrument_id,
                book,
                None,
                None,
                feed.clock.get_time_ns(),
                request.params,
            )))
        });
        Ok(())
    }

    fn request_trades(&self, _request: RequestTrades) -> anyhow::Result<()> {
        anyhow::bail!("QFEX has no public trade history endpoint: trades come from the stream")
    }

    fn request_funding_rates(&self, request: RequestFundingRates) -> anyhow::Result<()> {
        let instrument_id = request.single_instrument_id()?;
        let instrument = self.instrument(&instrument_id)?;
        let feed = self.feed.clone();
        let client_id = request.client_id.unwrap_or(self.client_id);
        let start = datetime_to_unix_nanos(request.start);
        let end = datetime_to_unix_nanos(request.end);
        let limit = request.limit.map(std::num::NonZeroUsize::get);

        self.spawn_request("funding rates", async move {
            let funding_rates = feed
                .http
                .request_funding_rates(&instrument, start, end, limit)
                .await?;
            Ok(DataResponse::FundingRates(FundingRatesResponse::new(
                request.request_id,
                client_id,
                instrument_id,
                funding_rates,
                start,
                end,
                feed.clock.get_time_ns(),
                request.params,
            )))
        });
        Ok(())
    }

    fn request_bars(&self, request: RequestBars) -> anyhow::Result<()> {
        let bar_type = request.single_bar_type()?;
        QfexBarInterval::try_from(&bar_type)?;
        let instrument = self.instrument(&bar_type.instrument_id())?;
        let feed = self.feed.clone();
        let client_id = request.client_id.unwrap_or(self.client_id);
        let start = datetime_to_unix_nanos(request.start);
        let end = datetime_to_unix_nanos(request.end);
        let limit = request.limit.map(std::num::NonZeroUsize::get);

        self.spawn_request("bars", async move {
            let bars = feed
                .http
                .request_bars(bar_type, &instrument, start, end, limit)
                .await?;
            Ok(DataResponse::Bars(BarsResponse::new(
                request.request_id,
                client_id,
                bar_type,
                bars,
                start,
                end,
                feed.clock.get_time_ns(),
                request.params,
            )))
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const MINUTE: u64 = 60_000_000_000;

    #[rstest]
    #[case(0, MINUTE, MINUTE)]
    #[case(MINUTE - 1, MINUTE, MINUTE)]
    // On the boundary the bar that just closed is not asked for again
    #[case(MINUTE, MINUTE, 2 * MINUTE)]
    #[case(7 * MINUTE + 1, 5 * MINUTE, 10 * MINUTE)]
    fn next_bar_close_is_the_next_interval_boundary(
        #[case] now: u64,
        #[case] interval: u64,
        #[case] expected: u64,
    ) {
        assert_eq!(next_bar_close(now, interval), expected);
    }

    #[rstest]
    fn deltas_and_depth_share_the_level2_channel() {
        let channels: Vec<_> = Kind::ALL.iter().map(|kind| kind.channel()).collect();
        assert_eq!(
            channels,
            [
                QfexMdsChannel::Level2,
                QfexMdsChannel::Level2,
                QfexMdsChannel::Bbo,
                QfexMdsChannel::Trade,
                QfexMdsChannel::MarkPrice,
                QfexMdsChannel::Underlier,
                QfexMdsChannel::Funding,
            ]
        );
    }
}
