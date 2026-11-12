//! The execution client against a local stand-in for the QFEX REST API and trade socket.
//!
//! Nothing here reaches the venue and the credentials are dummy strings. The frames the
//! stand-in sends are hand-made from the examples in the docs (11 Nov 2026): these tests show
//! that the client agrees with the docs, not that the venue behaves like the docs.

use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use nautilus_common::{
    cache::Cache,
    clients::ExecutionClient,
    live::runner::replace_exec_event_sender,
    messages::{
        ExecutionEvent, ExecutionReport,
        execution::{
            CancelAllOrders, CancelOrder, GenerateFillReports, GenerateOrderStatusReport,
            GenerateOrderStatusReports, GeneratePositionStatusReports, ModifyOrder, SubmitOrder,
        },
    },
};
use nautilus_core::{UUID4, UnixNanos};
use nautilus_cryptography::signing::hmac_signature;
use nautilus_live::ExecutionClientCore;
use nautilus_model::{
    enums::{AccountType, LiquiditySide, OmsType, OrderSide, OrderStatus, OrderType},
    events::{AccountState, OrderEventAny},
    identifiers::{ClientId, ClientOrderId, InstrumentId, StrategyId, TraderId, VenueOrderId},
    orders::{Order, OrderAny, OrderTestBuilder},
    types::{Money, Price, Quantity},
};
use nautilus_qfex::{
    common::consts::QFEX_VENUE, config::exec::QfexExecClientConfig, execution::QfexExecutionClient,
};
use parking_lot::Mutex;
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use tokio::sync::{Notify, broadcast, mpsc::UnboundedReceiver};

const API_KEY: &str = "test-public-key";
const API_SECRET: &str = "not-a-real-secret";
const ORDER_ID: &str = "5b309929-206f-40ec-804d-cbe46e81afc1";
const NEW_ORDER_ID: &str = "88888888-4444-4444-4444-1234567890ab";

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/http/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Default)]
struct VenueState {
    gets: Vec<String>,
    /// Every request frame of the trade socket, in order.
    frames: Vec<Value>,
    /// What the client did that the venue would refuse. Checked when the harness drops.
    failures: Vec<String>,
    nonces: HashSet<String>,
    open_orders: Vec<Value>,
    refuse_login: bool,
    /// Bodies of `POST /user/leverage`.
    posts: Vec<Value>,
    /// When set, `/user/trade` has this many trades, served in pages.
    trade_rows: usize,
}

#[derive(Clone)]
struct Venue {
    state: Arc<Mutex<VenueState>>,
    push: broadcast::Sender<String>,
    drop_connection: Arc<Notify>,
}

impl Venue {
    async fn start() -> (Self, String, String) {
        let venue = Self {
            state: Arc::default(),
            push: broadcast::channel(64).0,
            drop_connection: Arc::default(),
        };
        let app = Router::new()
            .route("/ws", get(upgrade))
            .fallback(http)
            .with_state(venue.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            venue,
            format!("http://{address}"),
            format!("ws://{address}/ws"),
        )
    }

    /// The rules of the Authentication page: known key, nonce of at most 100 characters not
    /// used before, Unix seconds within 5 minutes, lowercase hex HMAC-SHA256 of `nonce:ts`.
    fn check_auth(
        &self,
        key: &str,
        nonce: &str,
        unix_ts: &str,
        signature: &str,
    ) -> Result<(), String> {
        if key != API_KEY {
            return Err(format!("wrong public key '{key}'"));
        }
        let fresh = self.state.lock().nonces.insert(nonce.to_string());
        if nonce.is_empty() || nonce.len() > 100 || !fresh {
            return Err(format!("nonce '{nonce}' is empty, too long or used twice"));
        }
        let ts: u64 = unix_ts
            .parse()
            .map_err(|_| format!("timestamp '{unix_ts}' is not Unix seconds"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        if now.as_secs().abs_diff(ts) > 300 {
            return Err(format!("timestamp {ts} is not now"));
        }
        if signature != hmac_signature(API_SECRET, &format!("{nonce}:{unix_ts}")).unwrap() {
            return Err("wrong signature".to_string());
        }
        Ok(())
    }

    fn fail(&self, failure: String) {
        self.state.lock().failures.push(failure);
    }

    async fn serve(self, mut socket: WebSocket) {
        let mut push = self.push.subscribe();
        let mut authenticated = false;
        loop {
            tokio::select! {
                () = self.drop_connection.notified() => return,
                frame = push.recv() => {
                    let Ok(frame) = frame else { return };
                    if socket.send(Message::Text(frame.into())).await.is_err() {
                        return;
                    }
                }
                message = socket.recv() => {
                    let Some(Ok(message)) = message else { return };
                    let Message::Text(request) = message else { continue };
                    let request: Value = serde_json::from_str(&request).unwrap_or_default();
                    for answer in self.answer(&request, &mut authenticated) {
                        if socket.send(Message::Text(answer.to_string().into())).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    }

    fn answer(&self, request: &Value, authenticated: &mut bool) -> Vec<Value> {
        let kind = request["type"].as_str().unwrap_or_default();
        let params = &request["params"];
        if kind == "auth" {
            let hmac = &params["hmac"];
            let text = |name: &str| hmac[name].as_str().unwrap_or_default();
            // The docs send `unix_ts` as a number.
            let unix_ts = hmac["unix_ts"].as_u64().map(|ts| ts.to_string());
            let verdict = self.check_auth(
                text("public_key"),
                text("nonce"),
                &unix_ts.unwrap_or_default(),
                text("signature"),
            );
            let mut state = self.state.lock();
            state.frames.push(json!({"type": "auth"}));
            if let Err(reason) = verdict {
                state.failures.push(format!("auth: {reason}"));
            }
            if state.refuse_login {
                // unverified: the docs show no failed login; any `auth` frame without
                // `"result":"success"` counts as one.
                return vec![
                    json!({"type": "auth", "result": "error", "message": "invalid signature"}),
                ];
            }
            *authenticated = true;
            return vec![json!({"type": "auth", "result": "success"})];
        }

        let mut state = self.state.lock();
        state.frames.push(request.clone());
        if !*authenticated {
            state.failures.push(format!("{kind} before auth"));
            return vec![];
        }
        match kind {
            "subscribe" => {
                let channels = params["channels"].as_array().into_iter().flatten();
                channels
                    .map(|channel| json!({"subscribed": channel}))
                    .collect()
            }
            "get_user_orders" => {
                let number = |name: &str| params[name].as_u64().unwrap_or_default() as usize;
                let orders = state.open_orders.iter().skip(number("offset"));
                let page: Vec<_> = orders.take(number("limit")).collect();
                vec![json!({"all_orders_response": {"orders": page, "twaps": []}})]
            }
            _ => vec![],
        }
    }

    fn push(&self, frame: &Value) {
        self.push
            .send(frame.to_string())
            .expect("no WebSocket connection");
    }

    fn frames(&self, kind: &str) -> Vec<Value> {
        let state = self.state.lock();
        let frames = state.frames.iter().filter(|frame| frame["type"] == kind);
        frames.map(|frame| frame["params"].clone()).collect()
    }

    async fn wait_frames(&self, kind: &str, count: usize) -> Vec<Value> {
        wait_until(kind, || self.frames(kind).len() >= count).await;
        self.frames(kind)
    }

    fn gets(&self, path: &str) -> Vec<String> {
        let state = self.state.lock();
        let gets = state.gets.iter().filter(|get| get.starts_with(path));
        gets.cloned().collect()
    }
}

async fn upgrade(ws: WebSocketUpgrade, State(venue): State<Venue>, uri: Uri) -> Response {
    if uri.query() != Some("api_key=test-public-key") {
        venue.fail(format!("trade socket opened with query {:?}", uri.query()));
    }
    ws.on_upgrade(|socket| venue.serve(socket))
}

async fn http(State(venue): State<Venue>, uri: Uri, headers: HeaderMap, body: String) -> Response {
    let (path, query) = (uri.path(), uri.query().unwrap_or_default());
    let name = match path {
        "/refdata" => "refdata",
        "/user/positions" => "user_positions",
        "/user/trade" => "user_trades",
        "/user/historic-orders" => "historic_orders",
        "/user/leverage" => "leverage",
        _ => {
            venue.fail(format!("unexpected request {path}"));
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    if !query.contains("api_version=2026-10-29") {
        venue.fail(format!("GET {path} without api_version"));
    }
    if path != "/refdata" {
        let header = |name: &str| {
            let value = headers.get(name).and_then(|value| value.to_str().ok());
            value.unwrap_or_default()
        };
        if let Err(reason) = venue.check_auth(
            header("x-qfex-public-key"),
            header("x-qfex-nonce"),
            header("x-qfex-timestamp"),
            header("x-qfex-hmac-signature"),
        ) {
            venue.fail(format!("GET {path}: {reason}"));
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    let mut state = venue.state.lock();
    if name == "leverage" {
        // Hand-made: the docs do not show the answer, and the client reads the status only.
        // A leverage above 100 is refused, as the venue refuses one it does not offer.
        let body: Value = serde_json::from_str(&body).unwrap_or_default();
        let refused = body["leverage"]
            .as_f64()
            .is_none_or(|leverage| leverage > 100.0);
        state.posts.push(body);
        return if refused {
            (StatusCode::BAD_REQUEST, Json(fixture("problem"))).into_response()
        } else {
            Json(json!({})).into_response()
        };
    }
    state.gets.push(format!("{path}?{query}"));
    if name == "user_trades" && state.trade_rows > 0 {
        // Hand-made: copies of the first fixture trade with their own ids, paged by `offset`.
        let offset = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("offset="));
        let offset: usize = offset.map_or(0, |offset| offset.parse().unwrap());
        let trade = &fixture(name)["data"][0];
        let page = (offset..state.trade_rows.min(offset + 1000)).map(|number| {
            let mut trade = trade.clone();
            trade["id"] = json!(format!("T-{number}"));
            trade
        });
        return Json(json!({"data": page.collect::<Vec<_>>()})).into_response();
    }
    Json(fixture(name)).into_response()
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

fn aapl() -> InstrumentId {
    InstrumentId::from("AAPL-USD-PERP.QFEX")
}

fn client_id() -> ClientId {
    ClientId::from("QFEX")
}

fn trader_id() -> TraderId {
    TraderId::from("TESTER-001")
}

fn builder(order_type: OrderType, client_order_id: &str) -> OrderTestBuilder {
    let mut builder = OrderTestBuilder::new(order_type);
    builder
        .instrument_id(aapl())
        .client_order_id(ClientOrderId::from(client_order_id))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("1.50"));
    builder
}

fn limit(client_order_id: &str) -> OrderAny {
    builder(OrderType::Limit, client_order_id)
        .price(Price::from("200.25"))
        .build()
}

/// An `order_response` as in the docs, for the limit order of [`limit`].
fn row(order_id: &str, client_order_id: &str, status: &str) -> Value {
    json!({"order_response": {
        "order_id": order_id, "client_order_id": client_order_id, "symbol": "AAPL-USD",
        "status": status, "quantity": 1.5, "quantity_remaining": 1.5, "price": 200.25,
        "side": "BUY", "type": "LIMIT", "time_in_force": "GTC", "update_time": 1_792_756_800.123,
    }})
}

struct Harness {
    venue: Venue,
    client: QfexExecutionClient,
    events: UnboundedReceiver<ExecutionEvent>,
    cache: Rc<RefCell<Cache>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let failures = &self.venue.state.lock().failures;
            assert!(failures.is_empty(), "venue refused: {failures:?}");
        }
    }
}

impl Harness {
    /// Builds a client against a new venue stand-in and tries to connect it.
    async fn start(
        configure: impl FnOnce(&mut QfexExecClientConfig, &Venue),
    ) -> (Self, anyhow::Result<()>) {
        let (venue, http_url, ws_url) = Venue::start().await;
        // Credentials come from the config here, never from the environment.
        let mut config = QfexExecClientConfig {
            base_url_http: Some(http_url),
            base_url_ws: Some(ws_url),
            api_key: Some(API_KEY.to_string()),
            api_secret: Some(API_SECRET.to_string()),
            ..Default::default()
        };
        configure(&mut config, &venue);

        let (sender, events) = tokio::sync::mpsc::unbounded_channel();
        replace_exec_event_sender(sender);
        let cache = Rc::new(RefCell::new(Cache::default()));
        let core = ExecutionClientCore::new(
            trader_id(),
            client_id(),
            *QFEX_VENUE,
            OmsType::Netting,
            config.account_id,
            AccountType::Margin,
            None,
            cache.clone(),
        );
        let mut client = QfexExecutionClient::new(core, config).unwrap();
        client.start().unwrap();
        let connected = client.connect().await;
        let harness = Self {
            venue,
            client,
            events,
            cache,
        };
        (harness, connected)
    }

    /// Connects and takes the account state that `connect` reads over REST.
    async fn connect() -> (Self, AccountState) {
        let (mut harness, connected) = Self::start(|_, _| {}).await;
        connected.unwrap();
        let account = harness.next_account().await;
        harness.venue.wait_frames("subscribe", 1).await;
        (harness, account)
    }

    async fn next_event(&mut self) -> ExecutionEvent {
        tokio::time::timeout(Duration::from_secs(10), self.events.recv())
            .await
            .expect("no event within 10 s")
            .expect("event channel closed")
    }

    async fn next_order_event(&mut self) -> OrderEventAny {
        match self.next_event().await {
            ExecutionEvent::Order(event) => event,
            other => panic!("expected an order event, got {other:?}"),
        }
    }

    async fn next_account(&mut self) -> AccountState {
        match self.next_event().await {
            ExecutionEvent::Account(account) => account,
            other => panic!("expected an account state, got {other:?}"),
        }
    }

    async fn next_report(&mut self) -> ExecutionReport {
        match self.next_event().await {
            ExecutionEvent::Report(report) => report,
            other => panic!("expected a report, got {other:?}"),
        }
    }

    /// Nothing is waiting: the next event is the answer to a balance frame pushed now.
    async fn assert_no_pending_events(&mut self) {
        self.venue.push(&json!({"balance_response": {
            "available_balance": 1.0, "order_margin": 0.0, "position_margin": 0.0,
        }}));
        let account = self.next_account().await;
        assert_eq!(account.balances[0].total, Money::from("1 USDC"));
    }

    /// Takes the next order event and applies it to the cached order, as the engine would.
    async fn applied(&mut self) -> OrderEventAny {
        let event = self.next_order_event().await;
        self.cache.borrow_mut().update_order(&event).unwrap();
        event
    }

    fn submit(&self, order: &OrderAny) {
        self.cache
            .borrow_mut()
            .add_order(order.clone(), None, Some(client_id()), false)
            .unwrap();
        let cmd = SubmitOrder::from_order(
            order,
            trader_id(),
            Some(client_id()),
            None,
            UUID4::new(),
            UnixNanos::default(),
        );
        self.client.submit_order(cmd).unwrap();
    }

    /// Submits, lets the venue acknowledge with `order_id`, and returns the `add_order` params.
    async fn submit_accepted(&mut self, order: &OrderAny, order_id: &str) -> Value {
        let sent = self.venue.frames("add_order").len();
        self.submit(order);
        assert!(matches!(self.applied().await, OrderEventAny::Submitted(_)));
        let frames = self.venue.wait_frames("add_order", sent + 1).await;
        self.venue
            .push(&row(order_id, order.client_order_id().as_str(), "ACK"));
        let OrderEventAny::Accepted(accepted) = self.applied().await else {
            panic!("not accepted");
        };
        assert_eq!(accepted.venue_order_id, VenueOrderId::from(order_id));
        frames.last().unwrap().clone()
    }

    fn cancel_all(&self, strategy_id: StrategyId, order_side: Option<OrderSide>) {
        let cmd = CancelAllOrders::new(
            trader_id(),
            Some(client_id()),
            strategy_id,
            aapl(),
            order_side,
            UUID4::new(),
            UnixNanos::default(),
            None,
            None,
        );
        self.client.cancel_all_orders(cmd).unwrap();
    }

    fn cancel(&self, order: &OrderAny) {
        let cmd = CancelOrder::new(
            trader_id(),
            Some(client_id()),
            order.strategy_id(),
            order.instrument_id(),
            order.client_order_id(),
            None,
            UUID4::new(),
            UnixNanos::default(),
            None,
            None,
        );
        self.client.cancel_order(cmd).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_signs_rest_logs_in_and_subscribes() {
    let (mut h, connected) = Harness::start(|config, _| config.cancel_on_disconnect = true).await;
    connected.unwrap();
    assert!(h.client.is_connected());

    // user_positions.json: available 9500.25 + order margin 174.5 + position margin 26.1.
    let account = h.next_account().await;
    assert_eq!(account.balances[0].total, Money::from("9700.85 USDC"));
    assert_eq!(account.balances[0].free, Money::from("9500.25 USDC"));
    assert_eq!(h.venue.gets("/user/positions").len(), 1);

    let subscribe = h.venue.wait_frames("subscribe", 1).await;
    let mut channels: Vec<_> = subscribe[0]["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|channel| channel.as_str().unwrap().to_string())
        .collect();
    channels.sort();
    assert_eq!(
        channels,
        ["balances", "fills", "order_responses", "positions"]
    );
    let cod = h.venue.wait_frames("cancel_on_disconnect", 1).await;
    assert_eq!(cod, [json!({"cancel_on_disconnect": true})]);
    // `auth` came first: the stand-in fails any frame before it.
    assert_eq!(h.venue.state.lock().frames[0], json!({"type": "auth"}));

    h.assert_no_pending_events().await;
    h.client.disconnect().await.unwrap();
    assert!(!h.client.is_connected());
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_login_fails_the_connect() {
    let (h, connected) = Harness::start(|_, venue| venue.state.lock().refuse_login = true).await;
    let error = connected.unwrap_err().to_string();
    assert!(error.contains("refused the login"), "{error}");
    assert!(!h.client.is_connected());
    assert!(h.venue.frames("subscribe").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn order_is_accepted_filled_and_cancelled() {
    let (mut h, _) = Harness::connect().await;
    let order = limit("O-1");

    let add = h.submit_accepted(&order, ORDER_ID).await;
    assert_eq!(
        add,
        json!({"symbol": "AAPL-USD", "side": "BUY", "order_type": "LIMIT",
            "order_time_in_force": "GTC", "quantity": 1.5, "price": 200.25,
            "client_order_id": "O-1"})
    );

    let fill = json!({"fill_response": {
        "trade_id": "T-1", "order_id": ORDER_ID, "client_order_id": "O-1", "symbol": "AAPL-USD",
        "price": 200.25, "quantity": 0.5, "side": "BUY", "aggressor_side": "SELL", "fee": 0.02,
        "order_type": "LIMIT", "tif": "GTC", "timestamp": 1_792_756_801.5,
    }});
    h.venue.push(&fill);
    // The same trade again, as after a resubscribe: one fill only.
    h.venue.push(&fill);
    let OrderEventAny::Filled(filled) = h.applied().await else {
        panic!("not filled");
    };
    assert_eq!(filled.client_order_id, order.client_order_id());
    assert_eq!(filled.venue_order_id, VenueOrderId::from(ORDER_ID));
    assert_eq!(filled.last_qty, Quantity::from("0.50"));
    assert_eq!(filled.last_px, Price::from("200.25"));
    assert_eq!(filled.liquidity_side, LiquiditySide::Maker);
    assert_eq!(filled.commission, Some(Money::from("0.02 USDC")));

    h.cancel(&order);
    let cancel = h.venue.wait_frames("cancel_order", 1).await;
    assert_eq!(
        cancel,
        [json!({"order_id": ORDER_ID, "symbol": "AAPL-USD", "cancel_order_id_type": "order_id"})]
    );
    h.venue.push(&row(ORDER_ID, "O-1", "CANCELLED"));
    let OrderEventAny::Canceled(canceled) = h.applied().await else {
        panic!("not canceled");
    };
    assert_eq!(canceled.venue_order_id, Some(VenueOrderId::from(ORDER_ID)));
    h.assert_no_pending_events().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn modify_follows_the_new_order_id() {
    let (mut h, _) = Harness::connect().await;
    let order = limit("O-1");
    h.submit_accepted(&order, ORDER_ID).await;

    let cmd = ModifyOrder::new(
        trader_id(),
        Some(client_id()),
        order.strategy_id(),
        aapl(),
        order.client_order_id(),
        None,
        Some(Quantity::from("2.00")),
        Some(Price::from("201.00")),
        None,
        UUID4::new(),
        UnixNanos::default(),
        None,
        None,
    );
    h.client.modify_order(cmd).unwrap();
    let modify = h.venue.wait_frames("modify_order", 1).await;
    assert_eq!(
        modify,
        [
            json!({"order_id": ORDER_ID, "symbol": "AAPL-USD", "side": "BUY",
            "order_type": "LIMIT", "reduce_only": false, "quantity": 2.0, "price": 201.0})
        ]
    );

    // As in the docs: a new order id and no client order id.
    h.venue.push(&json!({"order_response": {
        "order_id": NEW_ORDER_ID, "client_order_id": "", "symbol": "AAPL-USD",
        "status": "MODIFIED", "quantity": 2.0, "quantity_remaining": 2.0, "price": 201.0,
        "side": "BUY", "type": "LIMIT", "time_in_force": "GTC",
    }}));
    let OrderEventAny::Updated(updated) = h.applied().await else {
        panic!("not updated");
    };
    assert_eq!(
        updated.venue_order_id,
        Some(VenueOrderId::from(NEW_ORDER_ID))
    );
    assert_eq!(updated.quantity, Quantity::from("2.00"));
    assert_eq!(updated.price, Some(Price::from("201.00")));

    h.cancel(&order);
    let cancel = h.venue.wait_frames("cancel_order", 1).await;
    assert_eq!(cancel[0]["order_id"], NEW_ORDER_ID);
    h.assert_no_pending_events().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_and_unsupported_orders() {
    let (mut h, _) = Harness::connect().await;

    // The venue has no stop-limit order: denied before anything is sent.
    let stop_limit = builder(OrderType::StopLimit, "O-1")
        .price(Price::from("200.25"))
        .trigger_price(Price::from("200.00"))
        .build();
    h.submit(&stop_limit);
    let OrderEventAny::Denied(denied) = h.next_order_event().await else {
        panic!("not denied");
    };
    assert!(denied.reason.contains("not supported"), "{}", denied.reason);

    let order = limit("O-2");
    h.submit(&order);
    assert!(matches!(
        h.next_order_event().await,
        OrderEventAny::Submitted(_)
    ));
    let add = h.venue.wait_frames("add_order", 1).await;
    assert_eq!(add.len(), 1, "the denied order was sent");
    h.venue.push(&json!({"err": {
        "error_code": "InvalidOrder", "message": "price out of range",
        "incoming_message": {"type": "add_order", "params": add[0]},
    }}));
    let OrderEventAny::Rejected(rejected) = h.next_order_event().await else {
        panic!("not rejected");
    };
    assert_eq!(rejected.client_order_id, order.client_order_id());
    assert_eq!(rejected.reason.as_str(), "InvalidOrder: price out of range");

    // A status that refuses the order, without an error frame.
    let order = limit("O-3");
    h.submit(&order);
    assert!(matches!(
        h.next_order_event().await,
        OrderEventAny::Submitted(_)
    ));
    h.venue.wait_frames("add_order", 2).await;
    h.venue.push(&row("", "O-3", "REJECTED_MARKET_CLOSED"));
    let OrderEventAny::Rejected(rejected) = h.next_order_event().await else {
        panic!("not rejected");
    };
    assert_eq!(rejected.reason.as_str(), "REJECTED_MARKET_CLOSED");
    h.assert_no_pending_events().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_order_uses_the_stop_requests() {
    let (mut h, _) = Harness::connect().await;
    let stop = builder(OrderType::StopMarket, "O-1")
        .side(OrderSide::Sell)
        .trigger_price(Price::from("150.00"))
        .build();

    let add = h.submit_accepted(&stop, ORDER_ID).await;
    assert_eq!(
        add,
        json!({"symbol": "AAPL-USD", "side": "SELL", "order_type": "STOP_LOSS",
            "order_time_in_force": "IOC", "quantity": 1.5, "price": 150.0,
            "client_order_id": "O-1"})
    );

    h.cancel(&stop);
    let cancel = h.venue.wait_frames("cancel_stop_order", 1).await;
    assert_eq!(cancel, [json!({"stop_order_id": ORDER_ID})]);
    assert!(h.venue.frames("cancel_order").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_all_cancels_the_orders_of_the_strategy_one_by_one() {
    let (mut h, _) = Harness::connect().await;
    let (first, second) = (limit("O-1"), limit("O-2"));
    h.submit_accepted(&first, ORDER_ID).await;
    h.submit_accepted(&second, NEW_ORDER_ID).await;

    // With a side filter: the venue's mass cancel has none.
    h.cancel_all(first.strategy_id(), Some(OrderSide::Buy));

    let cancels = h.venue.wait_frames("cancel_order", 2).await;
    let mut ids: Vec<_> = cancels.iter().map(|c| c["order_id"].clone()).collect();
    ids.sort_by_key(Value::to_string);
    assert_eq!(ids, [json!(ORDER_ID), json!(NEW_ORDER_ID)]);
    // The venue's own mass cancel would hit the other side too.
    assert!(h.venue.frames("cancel_all_orders").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_all_without_a_side_uses_the_mass_cancel_when_it_hits_nothing_else() {
    let (mut h, _) = Harness::connect().await;
    let first = limit("O-1");
    let other_strategy = StrategyId::from("S-002");
    let second = builder(OrderType::Limit, "O-2")
        .strategy_id(other_strategy)
        .price(Price::from("200.25"))
        .build();
    h.submit_accepted(&first, ORDER_ID).await;
    h.submit_accepted(&second, NEW_ORDER_ID).await;

    // Another strategy has an order on the symbol: the mass cancel would take it too.
    h.cancel_all(first.strategy_id(), None);
    let cancels = h.venue.wait_frames("cancel_order", 1).await;
    assert_eq!(cancels[0]["order_id"], ORDER_ID);
    h.venue.push(&row(ORDER_ID, "O-1", "CANCELLED"));
    assert!(matches!(h.applied().await, OrderEventAny::Canceled(_)));
    assert!(h.venue.frames("cancel_all_orders").is_empty());

    // Now the open orders of the symbol are the orders of the command: one frame.
    h.cancel_all(other_strategy, None);
    let mass = h.venue.wait_frames("cancel_all_orders", 1).await;
    assert_eq!(mass, [json!({"symbol": "AAPL-USD"})]);
    // As in the docs: one `CANCELLED` row for each order.
    h.venue.push(&row(NEW_ORDER_ID, "O-2", "CANCELLED"));
    let OrderEventAny::Canceled(canceled) = h.applied().await else {
        panic!("not canceled");
    };
    assert_eq!(canceled.client_order_id, second.client_order_id());
    assert_eq!(h.venue.frames("cancel_order").len(), 1);
    h.assert_no_pending_events().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn leverage_is_set_at_connect_and_on_request() {
    let crude = InstrumentId::from("CL-USD-PERP.QFEX");
    let (h, connected) = Harness::start(|config, _| {
        config.leverages = [(aapl(), dec!(5)), (crude, dec!(500))].into();
    })
    .await;
    // The stand-in refuses 500: a refused leverage does not stop the connect.
    connected.unwrap();
    let mut posts = h.venue.state.lock().posts.clone();
    posts.sort_by_key(Value::to_string);
    assert_eq!(
        posts,
        [
            json!({"symbol": "AAPL-USD", "leverage": 5.0}),
            json!({"symbol": "CL-USD", "leverage": 500.0})
        ]
    );

    h.client.set_leverage(aapl(), dec!(3)).await.unwrap();
    assert_eq!(
        h.venue.state.lock().posts[2],
        json!({"symbol": "AAPL-USD", "leverage": 3.0})
    );
    let refused = h.client.set_leverage(aapl(), dec!(500)).await;
    let refused = refused.unwrap_err().to_string();
    assert!(refused.contains("Bad Request"), "{refused}");
    // Not a QFEX symbol: nothing is sent.
    let unknown = InstrumentId::from("NOPE-USD-PERP.QFEX");
    let unknown = h.client.set_leverage(unknown, dec!(3)).await;
    assert!(unknown.unwrap_err().to_string().contains("no symbol"));
    assert_eq!(h.venue.state.lock().posts.len(), 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_follow_the_pages() {
    let (h, _) = Harness::connect().await;
    {
        // 1,001 open orders and 2,003 trades: two and three pages of 1,000.
        let mut state = h.venue.state.lock();
        let open = |number| row(&format!("order-{number}"), "", "ACK")["order_response"].clone();
        state.open_orders = (0..1001).map(open).collect();
        state.trade_rows = 2003;
    }
    let ts = UnixNanos::default();

    let cmd = GenerateOrderStatusReports::new(UUID4::new(), ts, true, None, None, None, None, None);
    let reports = h.client.generate_order_status_reports(&cmd).await.unwrap();
    assert_eq!(reports.len(), 1001);
    assert_eq!(
        h.venue.frames("get_user_orders"),
        [
            json!({"limit": 1000, "offset": 0}),
            json!({"limit": 1000, "offset": 1000})
        ]
    );

    let cmd = GenerateFillReports::new(UUID4::new(), ts, None, None, None, None, None, None);
    let fills = h.client.generate_fill_reports(cmd).await.unwrap();
    assert_eq!(fills.len(), 2003);
    assert_eq!(fills[2002].trade_id.to_string(), "T-2002");
    let pages = h.venue.gets("/user/trade");
    let offsets: Vec<_> = pages.iter().map(|page| page.contains("offset=")).collect();
    assert_eq!(offsets, [false, true, true], "{pages:?}");
    assert!(pages[2].contains("offset=2000"), "{pages:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_come_from_the_socket_and_rest() {
    let (h, _) = Harness::connect().await;
    let open = row(ORDER_ID, "ext-1", "ACK")["order_response"].clone();
    h.venue.state.lock().open_orders = vec![open];
    let ts = UnixNanos::default();

    let cmd = GenerateOrderStatusReports::new(UUID4::new(), ts, true, None, None, None, None, None);
    let reports = h.client.generate_order_status_reports(&cmd).await.unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].venue_order_id, VenueOrderId::from(ORDER_ID));
    assert_eq!(
        reports[0].client_order_id,
        Some(ClientOrderId::from("ext-1"))
    );
    assert_eq!(reports[0].order_status, OrderStatus::Accepted);
    assert_eq!(reports[0].price, Some(Price::from("200.25")));
    assert_eq!(
        h.venue.frames("get_user_orders"),
        [json!({"limit": 1000, "offset": 0})]
    );

    // Open orders over the socket plus the two of historic_orders.json.
    let cmd =
        GenerateOrderStatusReports::new(UUID4::new(), ts, false, None, None, None, None, None);
    let reports = h.client.generate_order_status_reports(&cmd).await.unwrap();
    let statuses: Vec<_> = reports.iter().map(|report| report.order_status).collect();
    assert_eq!(
        statuses,
        [
            OrderStatus::Accepted,
            OrderStatus::Filled,
            OrderStatus::Canceled
        ]
    );

    let closed = VenueOrderId::from("0b9d7e0c-2f6a-4c1e-8d3b-5a4f3e2d1c0b");
    let cmd =
        GenerateOrderStatusReport::new(UUID4::new(), ts, None, None, Some(closed), None, None);
    let report = h.client.generate_order_status_report(&cmd).await.unwrap();
    assert_eq!(report.unwrap().order_status, OrderStatus::Filled);
    let query = h.venue.gets("/user/historic-orders").pop().unwrap();
    assert!(
        query.contains("order_id=0b9d7e0c-2f6a-4c1e-8d3b-5a4f3e2d1c0b"),
        "{query}"
    );

    let cmd = GenerateFillReports::new(UUID4::new(), ts, None, None, None, None, None, None);
    let fills = h.client.generate_fill_reports(cmd).await.unwrap();
    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0].venue_order_id, closed);
    assert_eq!(fills[0].last_px, Price::from("348.05"));

    let cmd = GeneratePositionStatusReports::new(UUID4::new(), ts, None, None, None, None, None);
    let positions = h
        .client
        .generate_position_status_reports(&cmd)
        .await
        .unwrap();
    let aapl_position = positions
        .iter()
        .find(|p| p.instrument_id == aapl())
        .unwrap();
    assert_eq!(aapl_position.quantity, Quantity::from("1.50"));
    assert!(aapl_position.position_side.to_string().contains("SHORT"));

    let mass = h.client.generate_mass_status(None).await.unwrap().unwrap();
    assert_eq!(mass.order_reports().len(), 3);
    assert_eq!(mass.fill_reports().values().flatten().count(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnect_logs_in_again_and_catches_up() {
    let (mut h, _) = Harness::connect().await;
    let order = limit("O-1");
    h.submit_accepted(&order, ORDER_ID).await;
    // While the socket is down the order is filled: it is no longer open at the venue.
    h.venue.drop_connection.notify_waiters();

    h.venue.wait_frames("auth", 2).await;
    h.venue.wait_frames("subscribe", 2).await;
    h.venue.wait_frames("get_user_orders", 1).await;

    // What the socket did not deliver comes from REST: the two fills of user_trades.json,
    // the two closed orders of historic_orders.json, then the balance.
    for _ in 0..2 {
        assert!(matches!(h.next_report().await, ExecutionReport::Fill(_)));
    }
    for _ in 0..2 {
        assert!(matches!(h.next_report().await, ExecutionReport::Order(_)));
    }
    let account = h.next_account().await;
    assert_eq!(account.balances[0].total, Money::from("9700.85 USDC"));
    let trades = h.venue.gets("/user/trade").pop().unwrap();
    assert!(trades.contains("start="), "{trades}");

    // The new connection works.
    assert!(h.client.is_connected());
    h.venue.push(&row(ORDER_ID, "O-1", "CANCELLED"));
    assert!(matches!(
        h.next_order_event().await,
        OrderEventAny::Canceled(_)
    ));
}
