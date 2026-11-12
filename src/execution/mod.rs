//! Live execution client for QFEX.
//!
//! Orders go over the trade socket. Reports come from the socket (open orders) and from REST
//! (fills, closed orders, positions, balance).

pub mod dispatch;
pub mod order;

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use nautilus_common::{
    clients::ExecutionClient,
    live::{get_runtime, runner::get_exec_event_sender},
    messages::execution::{
        BatchCancelOrders, CancelAllOrders, CancelOrder, GenerateFillReports,
        GenerateOrderStatusReport, GenerateOrderStatusReports, GeneratePositionStatusReports,
        ModifyOrder, QueryAccount, QueryOrder, SubmitOrder, SubmitOrderList,
    },
};
use nautilus_core::{UnixNanos, params::Params, time::get_atomic_clock_realtime};
use nautilus_live::{ExecutionClientCore, ExecutionEventEmitter};
use nautilus_model::{
    accounts::AccountAny,
    enums::OmsType,
    identifiers::{
        AccountId, ClientId, ClientOrderId, InstrumentId, StrategyId, Venue, VenueOrderId,
    },
    instruments::Instrument,
    orders::{Order, OrderAny},
    reports::{FillReport, OrderStatusReport, PositionStatusReport},
    types::{AccountBalance, MarginBalance},
};
use parking_lot::Mutex;
use tokio::{
    sync::{RwLock, mpsc::UnboundedReceiver, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use ustr::Ustr;

use self::{dispatch::OrderDispatch, order::is_stop};
use crate::{
    common::{credential::QfexCredential, parse::venue_symbol_of, urls::qfex_trade_ws_url},
    config::exec::QfexExecClientConfig,
    http::{
        client::{MAX_PAGE_LIMIT, QfexHttpClient, QfexUserQuery},
        models::QfexBalance,
        parse::{
            parse_balance, parse_fill_report, parse_historic_order_status_report,
            parse_position_status_report,
        },
    },
    signing::QfexSigner,
    websocket::{
        client::QfexWebSocketClient,
        messages::{QfexTradeRequest, QfexWsMessage, QfexWsOrder, QfexWsOrders},
    },
};

const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Longest wait for the answer to the login and to `get_user_orders`.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// The default page of `get_user_orders`, which is also the largest the docs mention.
const OPEN_ORDERS_LIMIT: u32 = 1_000;
/// Venue times are compared with the local clock when catching up: allow this much skew.
const CATCH_UP_MARGIN_NS: u64 = 60_000_000_000;

fn now() -> UnixNanos {
    get_atomic_clock_realtime().get_time_ns()
}

fn spawn(future: impl Future<Output = ()> + Send + 'static) -> JoinHandle<()> {
    get_runtime().spawn(future)
}

fn page_of(
    symbol: Option<String>,
    start: Option<UnixNanos>,
    end: Option<UnixNanos>,
) -> QfexUserQuery {
    // shortcut: one page of `MAX_PAGE_LIMIT` rows per REST report request; page with `offset`
    // when an account closes more orders or trades more than that between two reconciliations.
    QfexUserQuery {
        symbol,
        start,
        end,
        limit: Some(MAX_PAGE_LIMIT),
        ..Default::default()
    }
}

fn symbol_of(instrument_id: Option<&InstrumentId>) -> anyhow::Result<Option<String>> {
    Ok(instrument_id
        .map(venue_symbol_of)
        .transpose()?
        .map(str::to_string))
}

#[derive(Debug)]
struct Cancel {
    strategy_id: StrategyId,
    instrument_id: InstrumentId,
    client_order_id: ClientOrderId,
    venue_order_id: Option<VenueOrderId>,
}

impl From<&CancelOrder> for Cancel {
    fn from(cmd: &CancelOrder) -> Self {
        Self {
            strategy_id: cmd.strategy_id,
            instrument_id: cmd.instrument_id,
            client_order_id: cmd.client_order_id,
            venue_order_id: cmd.venue_order_id,
        }
    }
}

/// What the client and its tasks share while connected.
#[derive(Debug)]
struct Shared {
    /// Sending needs a read lock only; the write lock is for the disconnect.
    ws: RwLock<QfexWebSocketClient>,
    dispatch: OrderDispatch,
    http: QfexHttpClient,
    emitter: ExecutionEventEmitter,
    account_id: AccountId,
    /// Requests for the open orders that wait for an `all_orders_response`. The answer has no
    /// request id, but every request asks the same, so any answer serves the oldest waiter.
    waiting_for_orders: Mutex<VecDeque<oneshot::Sender<QfexWsOrders>>>,
    /// Local time from which a catch-up after a reconnect has to look.
    caught_up_ns: AtomicU64,
}

impl Shared {
    async fn send(&self, request: &QfexTradeRequest) -> anyhow::Result<()> {
        Ok(self.ws.read().await.send(request).await?)
    }

    fn emit_balance(&self, balance: &QfexBalance) -> anyhow::Result<()> {
        let balance = parse_balance(balance)?;
        self.emitter
            .emit_account_state(vec![balance], vec![], true, now(), None);
        Ok(())
    }

    async fn emit_rest_balance(&self) -> anyhow::Result<()> {
        self.emit_balance(&self.http.get_user_positions().await?.balance)
    }

    fn answer_orders(&self, mut orders: QfexWsOrders) {
        let mut waiting = self.waiting_for_orders.lock();
        // A waiter that gave up is skipped.
        while let Some(waiter) = waiting.pop_front() {
            match waiter.send(orders) {
                Ok(()) => return,
                Err(back) => orders = back,
            }
        }
    }

    /// Open orders and open stop orders, over the trade socket.
    async fn open_orders(&self) -> anyhow::Result<Vec<QfexWsOrder>> {
        let (tx, rx) = oneshot::channel();
        self.waiting_for_orders.lock().push_back(tx);
        // shortcut: one page; an account with more than 1,000 open orders needs `offset`.
        self.send(&QfexTradeRequest::GetUserOrders {
            limit: OPEN_ORDERS_LIMIT,
            offset: 0,
        })
        .await?;
        let orders = tokio::time::timeout(ANSWER_TIMEOUT, rx)
            .await
            .context("QFEX did not answer get_user_orders")?
            .context("QFEX refused get_user_orders or the trade socket reconnected")?;
        Ok(orders
            .orders
            .into_iter()
            .chain(orders.stop_orders)
            .collect())
    }

    async fn open_order_reports(&self) -> anyhow::Result<Vec<OrderStatusReport>> {
        let ts_init = now();
        let rows = self.open_orders().await?;
        Ok(rows
            .iter()
            .filter_map(|row| match self.dispatch.order_report(row, ts_init) {
                Ok(report) => report,
                Err(e) => {
                    log::warn!("QFEX open order '{}' unreadable: {e}", row.order_id);
                    None
                }
            })
            .collect())
    }

    /// Orders in a final state, from REST.
    async fn closed_order_reports(
        &self,
        query: &QfexUserQuery,
    ) -> anyhow::Result<Vec<OrderStatusReport>> {
        let ts_init = now();
        let orders = self.http.get_historic_orders(query).await?.data;
        Ok(orders
            .iter()
            .filter_map(|order| {
                let instrument = self.dispatch.instrument(&order.symbol)?;
                parse_historic_order_status_report(order, self.account_id, instrument, ts_init)
                    .map_err(|e| log::warn!("QFEX order '{}' unreadable: {e}", order.order_id))
                    .ok()
            })
            .collect())
    }

    async fn order_report(
        &self,
        client_order_id: Option<ClientOrderId>,
        venue_order_id: Option<VenueOrderId>,
    ) -> anyhow::Result<Option<OrderStatusReport>> {
        let venue_order_id =
            venue_order_id.or_else(|| self.dispatch.venue_order_id(&client_order_id?));
        let is_it = |report: &OrderStatusReport| {
            venue_order_id == Some(report.venue_order_id)
                || (client_order_id.is_some() && report.client_order_id == client_order_id)
        };
        if let Some(report) = self.open_order_reports().await?.into_iter().find(&is_it) {
            return Ok(Some(report));
        }
        let mut query = page_of(None, None, None);
        query.order_id = venue_order_id.map(|id| id.to_string());
        let closed = self.closed_order_reports(&query).await?;
        Ok(closed.into_iter().find(is_it))
    }

    async fn fill_reports(&self, query: &QfexUserQuery) -> anyhow::Result<Vec<FillReport>> {
        let ts_init = now();
        let trades = self.http.get_user_trades(query).await?.data;
        Ok(trades
            .iter()
            .filter_map(|trade| {
                let instrument = self.dispatch.instrument(&trade.symbol)?;
                let mut report = parse_fill_report(trade, self.account_id, instrument, ts_init)
                    .map_err(|e| log::warn!("QFEX trade '{}' unreadable: {e}", trade.id))
                    .ok()?;
                // The REST trade has no client order id.
                report.client_order_id = self.dispatch.client_order_id(&report.venue_order_id);
                Some(report)
            })
            .collect())
    }

    async fn position_reports(&self) -> anyhow::Result<Vec<PositionStatusReport>> {
        let ts_init = now();
        let positions = self.http.get_user_positions().await?.positions;
        Ok(positions
            .iter()
            .filter_map(|position| {
                let instrument = self.dispatch.instrument(&position.symbol)?;
                parse_position_status_report(position, self.account_id, instrument, ts_init)
                    .map_err(|e| log::warn!("QFEX position {} unreadable: {e}", position.symbol))
                    .ok()
            })
            .collect())
    }

    /// Brings the engine up to date after a reconnect: what the venue sent while the socket
    /// was down is lost, so fills, orders and the balance are read again.
    async fn catch_up(&self) {
        let started = now().as_u64();
        let since = self.caught_up_ns.load(Ordering::Relaxed);
        let since = UnixNanos::from(since.saturating_sub(CATCH_UP_MARGIN_NS));
        let query = page_of(None, Some(since), None);
        let mut complete = true;
        let mut failed = |what: &str, e: anyhow::Error| {
            log::error!("QFEX catch-up after reconnect: {what} failed: {e}");
            complete = false;
        };

        // Fills before orders, so a closed order is not reported before its last fill.
        match self.fill_reports(&query).await {
            Ok(reports) => {
                for report in reports {
                    if self.dispatch.is_new_trade(report.trade_id) {
                        self.emitter.send_fill_report(report);
                    }
                }
            }
            Err(e) => failed("fills", e),
        }
        match self.open_orders().await {
            Ok(rows) => {
                for row in &rows {
                    self.dispatch.handle_order(row, now());
                }
            }
            Err(e) => failed("open orders", e),
        }
        match self.closed_order_reports(&query).await {
            Ok(reports) => {
                for report in reports {
                    self.dispatch.close(&report.venue_order_id);
                    self.emitter.send_order_status_report(report);
                }
            }
            Err(e) => failed("closed orders", e),
        }
        if let Err(e) = self.emit_rest_balance().await {
            failed("balance", e);
        }
        if complete {
            self.caught_up_ns.store(started, Ordering::Relaxed);
        }
    }
}

/// Waits for the answer to the `auth` frame that `connect` sent.
async fn logged_in(rx: &mut UnboundedReceiver<QfexWsMessage>) -> anyhow::Result<()> {
    let answer = async {
        loop {
            match rx.recv().await {
                Some(QfexWsMessage::Authenticated) => return Ok(()),
                Some(QfexWsMessage::Error(error)) => {
                    anyhow::bail!("QFEX refused the login: {}", error.message)
                }
                Some(_) => {}
                None => anyhow::bail!("QFEX trade socket closed during the login"),
            }
        }
    };
    tokio::time::timeout(ANSWER_TIMEOUT, answer)
        .await
        .context("QFEX did not answer the login")?
}

async fn consume(
    shared: Arc<Shared>,
    mut rx: UnboundedReceiver<QfexWsMessage>,
    cancel: CancellationToken,
) {
    let mut authenticated = true;
    loop {
        let message = tokio::select! {
            () = cancel.cancelled() => break,
            message = rx.recv() => match message {
                Some(message) => message,
                None => break,
            },
        };
        match message {
            QfexWsMessage::Order(row) => shared.dispatch.handle_order(&row, now()),
            QfexWsMessage::Fill(fill) => shared.dispatch.handle_fill(&fill, now()),
            QfexWsMessage::Balance(balance) => {
                let balance = QfexBalance {
                    available_balance: balance.available_balance,
                    order_margin: balance.order_margin,
                    position_margin: balance.position_margin,
                };
                if let Err(e) = shared.emit_balance(&balance) {
                    log::warn!("QFEX balance unreadable: {e}");
                }
            }
            QfexWsMessage::Orders(orders) => shared.answer_orders(orders),
            QfexWsMessage::Reconnected => {
                log::warn!("QFEX trade socket reconnected");
                authenticated = false;
                // Their answers will not come.
                shared.waiting_for_orders.lock().clear();
                shared.dispatch.clear_modifies();
            }
            QfexWsMessage::Authenticated if !authenticated => {
                authenticated = true;
                // In its own task: it waits for frames that this loop has to read.
                let shared = shared.clone();
                spawn(async move { shared.catch_up().await });
            }
            // A refused request sends itself back; a refused login has no request to send.
            QfexWsMessage::Error(error)
                if !authenticated && matches!(error.request_type(), None | Some("auth")) =>
            {
                log::error!(
                    "QFEX refused the login after a reconnect: {}; execution client stops",
                    error.message
                );
                cancel.cancel();
                break;
            }
            QfexWsMessage::Error(error) => {
                if error.request_type() == Some("get_user_orders") {
                    shared.waiting_for_orders.lock().pop_front();
                }
                if !shared.dispatch.handle_error(&error, now()) {
                    log::error!(
                        "QFEX error {}: {} (request: {})",
                        error.error_code.as_ref().map_or("?", AsRef::as_ref),
                        error.message,
                        error.request_type().unwrap_or("?")
                    );
                }
            }
            // Positions are read over REST when the engine asks.
            _ => {}
        }
    }
    shared.ws.write().await.disconnect().await;
}

/// Live execution client for QFEX.
#[derive(Debug)]
pub struct QfexExecutionClient {
    core: ExecutionClientCore,
    config: QfexExecClientConfig,
    emitter: ExecutionEventEmitter,
    shared: Option<Arc<Shared>>,
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
}

impl QfexExecutionClient {
    /// Creates a new [`QfexExecutionClient`]. Credentials are read on `connect`.
    ///
    /// # Errors
    ///
    /// Never fails today; the signature is the one the factory expects.
    pub fn new(core: ExecutionClientCore, config: QfexExecClientConfig) -> anyhow::Result<Self> {
        let emitter = ExecutionEventEmitter::new(
            get_atomic_clock_realtime(),
            core.trader_id,
            core.account_id,
            core.account_type,
            None,
        );
        Ok(Self {
            core,
            config,
            emitter,
            shared: None,
            cancel: CancellationToken::new(),
            tasks: Vec::new(),
        })
    }

    fn shared(&self) -> anyhow::Result<Arc<Shared>> {
        self.shared
            .clone()
            .filter(|_| self.is_connected())
            .context("QFEX execution client is not connected")
    }

    fn submit(&self, orders: Vec<OrderAny>) -> anyhow::Result<()> {
        let shared = self.shared()?;
        let mut requests = Vec::new();
        for order in orders {
            if order.is_closed() {
                log::warn!("Cannot submit closed order {}", order.client_order_id());
                continue;
            }
            let request = venue_symbol_of(&order.instrument_id()).and_then(|symbol| {
                let symbol = Ustr::from(symbol);
                anyhow::ensure!(
                    shared.dispatch.instrument(&symbol).is_some(),
                    "QFEX has no symbol {symbol}"
                );
                order::add_order(&order, symbol)
            });
            match request {
                Ok(request) => {
                    self.emitter.emit_order_submitted(&order);
                    shared.dispatch.track(&order);
                    requests.push((order.client_order_id(), QfexTradeRequest::AddOrder(request)));
                }
                Err(e) => self.emitter.emit_order_denied(&order, &e.to_string()),
            }
        }
        spawn(async move {
            for (client_order_id, request) in requests {
                if let Err(e) = shared.send(&request).await {
                    // Not rejected here: the frame may have left. The order stays submitted
                    // and the engine's in-flight check asks for its state.
                    log::error!("QFEX add_order for {client_order_id} got no answer: {e}");
                }
            }
        });
        Ok(())
    }

    /// The cancel request of one order, or a cancel rejection if there is none to send.
    fn cancel_request(&self, shared: &Shared, cancel: &Cancel) -> Option<QfexTradeRequest> {
        let order = self.core.get_order(&cancel.client_order_id).ok();
        if let Some(order) = &order {
            // So that the answer comes back as an order event.
            shared.dispatch.track(order);
        }
        let venue_order_id = shared
            .dispatch
            .venue_order_id(&cancel.client_order_id)
            .or(cancel.venue_order_id);
        let stop = order.is_some_and(|order| is_stop(order.order_type()));
        let request = venue_symbol_of(&cancel.instrument_id).and_then(|symbol| {
            order::cancel_order(
                stop,
                Ustr::from(symbol),
                cancel.client_order_id.as_str(),
                venue_order_id.as_ref(),
            )
        });
        request
            .map_err(|e| {
                self.emitter.emit_order_cancel_rejected_event(
                    cancel.strategy_id,
                    cancel.instrument_id,
                    cancel.client_order_id,
                    cancel.venue_order_id,
                    &e.to_string(),
                    now(),
                );
            })
            .ok()
    }

    /// Sends the cancels one after the other, in order.
    fn cancel(&self, cancels: Vec<Cancel>) -> anyhow::Result<()> {
        let shared = self.shared()?;
        let requests: Vec<_> = cancels
            .into_iter()
            .filter_map(|cancel| Some((self.cancel_request(&shared, &cancel)?, cancel)))
            .collect();
        spawn(async move {
            for (request, cancel) in requests {
                if let Err(e) = shared.send(&request).await {
                    shared.emitter.emit_order_cancel_rejected_event(
                        cancel.strategy_id,
                        cancel.instrument_id,
                        cancel.client_order_id,
                        cancel.venue_order_id,
                        &format!("not sent: {e}"),
                        now(),
                    );
                }
            }
        });
        Ok(())
    }

    fn modify_request(
        &self,
        shared: &Shared,
        cmd: &ModifyOrder,
    ) -> anyhow::Result<QfexTradeRequest> {
        let order = self.core.get_order(&cmd.client_order_id)?;
        shared.dispatch.track(&order);
        let venue_order_id = shared
            .dispatch
            .venue_order_id(&cmd.client_order_id)
            .or(cmd.venue_order_id)
            .context("order has no venue order id yet")?;
        let price = if is_stop(order.order_type()) {
            cmd.trigger_price.or(order.trigger_price())
        } else {
            cmd.price.or(order.price())
        };
        order::modify_order(
            &order,
            &venue_order_id,
            Ustr::from(venue_symbol_of(&cmd.instrument_id)?),
            // Assumed: the venue takes the new total quantity, not the open rest.
            cmd.quantity.unwrap_or(order.quantity()),
            price.context("order has no price to modify")?,
        )
    }
}

#[async_trait(?Send)]
impl ExecutionClient for QfexExecutionClient {
    fn is_connected(&self) -> bool {
        self.core.is_connected() && !self.cancel.is_cancelled()
    }

    fn client_id(&self) -> ClientId {
        self.core.client_id
    }

    fn account_id(&self) -> AccountId {
        self.core.account_id
    }

    fn venue(&self) -> Venue {
        self.core.venue
    }

    fn oms_type(&self) -> OmsType {
        self.core.oms_type
    }

    fn get_account(&self) -> Option<AccountAny> {
        self.core.cache().account_owned(&self.core.account_id)
    }

    fn generate_account_state(
        &self,
        balances: Vec<AccountBalance>,
        margins: Vec<MarginBalance>,
        reported: bool,
        ts_event: UnixNanos,
        info: Option<Params>,
    ) -> anyhow::Result<()> {
        self.emitter
            .emit_account_state(balances, margins, reported, ts_event, info);
        Ok(())
    }

    fn start(&mut self) -> anyhow::Result<()> {
        if !self.core.is_started() {
            self.emitter.set_sender(get_exec_event_sender());
            self.core.set_started();
        }
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        self.cancel.cancel();
        self.core.set_stopped();
        Ok(())
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        if self.is_connected() {
            return Ok(());
        }
        let config = &self.config;
        let credential = QfexCredential::resolve(
            config.api_key.clone(),
            config.api_secret.clone(),
            config.environment,
        )?
        .context("QFEX execution needs an API key and secret")?;
        let signer = QfexSigner::new(credential);
        let http = QfexHttpClient::new(
            config.environment,
            config.base_url_http.clone(),
            config.api_version.clone(),
            config.http_timeout_secs,
            config.proxy_url.clone(),
            Some(signer.clone()),
        )?;

        let mut instruments = HashMap::new();
        for instrument in http.request_instruments().await? {
            let symbol = Ustr::from(venue_symbol_of(&instrument.id())?);
            instruments.insert(symbol, instrument);
        }
        let balance = parse_balance(&http.get_user_positions().await?.balance)?;
        self.emitter
            .emit_account_state(vec![balance], vec![], true, now(), None);

        let url = config
            .base_url_ws
            .clone()
            .unwrap_or_else(|| qfex_trade_ws_url(config.environment).to_string());
        let mut ws = QfexWebSocketClient::new_trade(&url, signer, config.cancel_on_disconnect)?;
        let caught_up_ns = AtomicU64::new(now().as_u64());
        let mut rx = ws.connect().await?;
        if let Err(e) = logged_in(&mut rx).await {
            ws.disconnect().await;
            return Err(e);
        }

        let shared = Arc::new(Shared {
            ws: RwLock::new(ws),
            dispatch: OrderDispatch::new(self.core.account_id, self.emitter.clone(), instruments),
            http,
            emitter: self.emitter.clone(),
            account_id: self.core.account_id,
            waiting_for_orders: Mutex::default(),
            caught_up_ns,
        });
        self.cancel = CancellationToken::new();
        self.tasks = vec![spawn(consume(shared.clone(), rx, self.cancel.clone()))];
        self.shared = Some(shared);
        self.core.set_connected();
        log::info!("Connected QFEX execution client {}", self.core.client_id);
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.cancel.cancel();
        for mut task in self.tasks.drain(..) {
            if tokio::time::timeout(DISCONNECT_TIMEOUT, &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
        self.shared = None;
        self.core.set_disconnected();
        Ok(())
    }

    fn submit_order(&self, cmd: SubmitOrder) -> anyhow::Result<()> {
        let order = self.core.get_order(&cmd.client_order_id)?;
        self.submit(vec![order])
    }

    fn submit_order_list(&self, cmd: SubmitOrderList) -> anyhow::Result<()> {
        // Sent one by one: the venue has no order lists, so contingencies are not kept there.
        let orders = self.core.get_orders_for_list(&cmd.order_list)?;
        self.submit(orders)
    }

    fn modify_order(&self, cmd: ModifyOrder) -> anyhow::Result<()> {
        let shared = self.shared()?;
        let rejected = |emitter: &ExecutionEventEmitter, cmd: &ModifyOrder, reason: String| {
            emitter.emit_order_modify_rejected_event(
                cmd.strategy_id,
                cmd.instrument_id,
                cmd.client_order_id,
                cmd.venue_order_id,
                &reason,
                now(),
            );
        };
        match self.modify_request(&shared, &cmd) {
            Ok(request) => {
                spawn(async move {
                    shared.dispatch.begin_modify(cmd.client_order_id);
                    if let Err(e) = shared.send(&request).await {
                        shared.dispatch.end_modify(&cmd.client_order_id);
                        rejected(&shared.emitter, &cmd, format!("not sent: {e}"));
                    }
                });
            }
            Err(e) => rejected(&self.emitter, &cmd, e.to_string()),
        }
        Ok(())
    }

    fn cancel_order(&self, cmd: CancelOrder) -> anyhow::Result<()> {
        self.cancel(vec![Cancel::from(&cmd)])
    }

    fn cancel_all_orders(&self, cmd: CancelAllOrders) -> anyhow::Result<()> {
        // One cancel per order of the strategy: the venue's `cancel_all_orders` would also
        // cancel the orders of other strategies on the account.
        let cancels = {
            let cache = self.core.cache();
            let open = cache.orders_open(
                None,
                Some(&cmd.instrument_id),
                Some(&cmd.strategy_id),
                None,
                cmd.order_side,
            );
            open.iter()
                .map(|order| Cancel {
                    strategy_id: order.strategy_id(),
                    instrument_id: order.instrument_id(),
                    client_order_id: order.client_order_id(),
                    venue_order_id: order.venue_order_id(),
                })
                .collect()
        };
        self.cancel(cancels)
    }

    fn batch_cancel_orders(&self, cmd: BatchCancelOrders) -> anyhow::Result<()> {
        self.cancel(cmd.cancels.iter().map(Cancel::from).collect())
    }

    fn query_account(&self, _cmd: QueryAccount) -> anyhow::Result<()> {
        let shared = self.shared()?;
        spawn(async move {
            if let Err(e) = shared.emit_rest_balance().await {
                log::warn!("QFEX account query failed: {e}");
            }
        });
        Ok(())
    }

    fn query_order(&self, cmd: QueryOrder) -> anyhow::Result<()> {
        let shared = self.shared()?;
        spawn(async move {
            let id = cmd.client_order_id;
            match shared.order_report(Some(id), cmd.venue_order_id).await {
                Ok(Some(report)) => shared.emitter.send_order_status_report(report),
                Ok(None) => log::warn!("QFEX order {id} not found"),
                Err(e) => log::warn!("QFEX order query for {id} failed: {e}"),
            }
        });
        Ok(())
    }

    async fn generate_order_status_report(
        &self,
        cmd: &GenerateOrderStatusReport,
    ) -> anyhow::Result<Option<OrderStatusReport>> {
        self.shared()?
            .order_report(cmd.client_order_id, cmd.venue_order_id)
            .await
    }

    async fn generate_order_status_reports(
        &self,
        cmd: &GenerateOrderStatusReports,
    ) -> anyhow::Result<Vec<OrderStatusReport>> {
        let shared = self.shared()?;
        let mut reports = shared.open_order_reports().await?;
        if !cmd.open_only {
            let symbol = symbol_of(cmd.instrument_id.as_ref())?;
            let query = page_of(symbol, cmd.start, cmd.end);
            reports.extend(shared.closed_order_reports(&query).await?);
        }
        if let Some(instrument_id) = cmd.instrument_id {
            reports.retain(|report| report.instrument_id == instrument_id);
        }
        Ok(reports)
    }

    async fn generate_fill_reports(
        &self,
        cmd: GenerateFillReports,
    ) -> anyhow::Result<Vec<FillReport>> {
        let shared = self.shared()?;
        let mut query = page_of(symbol_of(cmd.instrument_id.as_ref())?, cmd.start, cmd.end);
        query.order_id = cmd.venue_order_id.map(|id| id.to_string());
        let reports = shared.fill_reports(&query).await?;
        // The engine has these fills now: the `fills` channel must not send them again.
        for report in &reports {
            shared.dispatch.is_new_trade(report.trade_id);
        }
        Ok(reports)
    }

    async fn generate_position_status_reports(
        &self,
        cmd: &GeneratePositionStatusReports,
    ) -> anyhow::Result<Vec<PositionStatusReport>> {
        let mut reports = self.shared()?.position_reports().await?;
        if let Some(instrument_id) = cmd.instrument_id {
            reports.retain(|report| report.instrument_id == instrument_id);
        }
        Ok(reports)
    }
}
