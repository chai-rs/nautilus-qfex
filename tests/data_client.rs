//! The data client against a local stand-in for the public REST API and the MDS socket.
//!
//! No network and no credentials. The REST bodies and the socket frames are the ones recorded
//! from the public hosts on 10 Nov 2026 (`tests/fixtures`).

use std::{sync::Arc, time::Duration};

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
    clients::DataClient,
    live::runner::{replace_data_event_sender, replace_system_event_sender},
    messages::{
        DataEvent, SystemEvent,
        data::{
            DataResponse, RequestBars, RequestBookSnapshot, SubscribeBookDeltas,
            SubscribeBookDepth, SubscribeInstrumentStatus, SubscribeMarkPrices, SubscribeQuotes,
            SubscribeTrades, UnsubscribeQuotes,
        },
        system::SocketState,
    },
};
use nautilus_core::{UUID4, UnixNanos};
use nautilus_live::{SocketReconnectRegistry, SocketReconnectRequestOutcome};
use nautilus_model::{
    data::{BarType, Data},
    enums::{AggressorSide, BookAction, BookType, MarketStatusAction},
    identifiers::{ClientId, InstrumentId, Venue as VenueId},
    types::Price,
};
use nautilus_qfex::{config::data::QfexDataClientConfig, data::QfexDataClient};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc::UnboundedReceiver};

const WS_FRAMES: &str = include_str!("fixtures/ws/mds.jsonl");

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/http/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Clone, Default)]
struct Venue {
    /// Socket requests as `(type, channel, symbol)`.
    requests: Arc<Mutex<Vec<(String, String, String)>>>,
    failures: Arc<Mutex<Vec<String>>>,
    drop_connection: Arc<Notify>,
}

impl Venue {
    /// Acknowledges a subscribe and sends the first recorded frame of each channel and symbol.
    async fn serve(self, mut socket: WebSocket) {
        loop {
            let message = tokio::select! {
                () = self.drop_connection.notified() => return,
                message = socket.recv() => message,
            };
            let Some(Ok(Message::Text(text))) = message else {
                return;
            };
            let request: Value = serde_json::from_str(&text).unwrap();
            let kind = request["type"].as_str().unwrap_or_default().to_string();
            if kind == "ping" {
                continue;
            }
            let list = |name: &str| -> Vec<String> {
                let items = request[name].as_array().into_iter().flatten();
                items
                    .map(|item| item.as_str().unwrap().to_string())
                    .collect()
            };
            let (channels, symbols) = (list("channels"), list("symbols"));
            let mut answers = vec![];
            if kind == "subscribe" {
                answers.push(
                    json!({"type": "subscribed", "channels": channels, "symbols": symbols})
                        .to_string(),
                );
            }
            for channel in &channels {
                for symbol in &symbols {
                    self.requests
                        .lock()
                        .push((kind.clone(), channel.clone(), symbol.clone()));
                    let recorded = WS_FRAMES.lines().find(|line| {
                        let frame: Value = serde_json::from_str(line).unwrap();
                        frame["type"] == channel.as_str() && frame["symbol"] == symbol.as_str()
                    });
                    if kind == "subscribe" {
                        answers.extend(recorded.map(str::to_string));
                    }
                }
            }
            for answer in answers {
                if socket.send(Message::Text(answer.into())).await.is_err() {
                    return;
                }
            }
        }
    }

    async fn wait_for_requests(&self, count: usize) -> Vec<(String, String, String)> {
        for _ in 0..500 {
            let seen = self.requests.lock().clone();
            if seen.len() >= count {
                return seen;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "venue saw {:?}, expected {count} requests",
            self.requests.lock()
        );
    }
}

async fn http(State(venue): State<Venue>, uri: Uri, headers: HeaderMap) -> Response {
    let name = match uri.path() {
        "/refdata" => "refdata",
        "/md/orderbook/AAPL-USD" => "orderbook",
        "/candles/AAPL-USD" => "candles",
        path => {
            venue.failures.lock().push(format!("unexpected GET {path}"));
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    if !uri
        .query()
        .unwrap_or_default()
        .contains("api_version=2026-10-29")
    {
        venue
            .failures
            .lock()
            .push(format!("{uri} without api_version"));
    }
    // Public data must work without credentials: nothing may be signed.
    if headers
        .keys()
        .any(|name| name.as_str().starts_with("x-qfex-"))
    {
        venue
            .failures
            .lock()
            .push(format!("{uri} carries auth headers"));
    }
    Json(fixture(name)).into_response()
}

async fn start_venue() -> (QfexDataClientConfig, Venue) {
    let venue = Venue::default();
    let app = Router::new()
        .route(
            "/ws",
            get(
                |ws: WebSocketUpgrade, State(venue): State<Venue>| async move {
                    ws.on_upgrade(|socket| venue.serve(socket))
                },
            ),
        )
        .fallback(http)
        .with_state(venue.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let config = QfexDataClientConfig {
        base_url_http: Some(format!("http://{addr}")),
        base_url_ws: Some(format!("ws://{addr}/ws")),
        ..Default::default()
    };
    (config, venue)
}

async fn next_event(events: &mut UnboundedReceiver<DataEvent>) -> DataEvent {
    tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("no data event within 5 s")
        .expect("data event channel closed")
}

async fn next_data(events: &mut UnboundedReceiver<DataEvent>) -> Data {
    match next_event(events).await {
        DataEvent::Data(data) => data,
        other => panic!("expected data, was {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_subscribe_reconnect_request_and_disconnect() {
    let (config, venue) = start_venue().await;
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    replace_data_event_sender(sender);
    let client_id = ClientId::from("QFEX");
    let aapl = InstrumentId::from("AAPL-USD-PERP.QFEX");
    let crude = InstrumentId::from("CL-USD-PERP.QFEX");
    let (id, ts) = (UUID4::new, UnixNanos::default());
    let mut client = QfexDataClient::new(client_id, config).unwrap();

    // Connect loads the tradable instruments of the refdata fixture (17 rows, 2 delisted).
    client.connect().await.unwrap();
    assert!(client.is_connected());
    let mut instruments = 0;
    while let Ok(event) = events.try_recv() {
        assert!(
            matches!(event, DataEvent::Data(Data::Instrument(_))),
            "{event:?}"
        );
        instruments += 1;
    }
    assert_eq!(instruments, 15);

    // The pulsed book: every `level2` frame is a whole book, so it starts with a clear.
    client
        .subscribe_book_deltas(SubscribeBookDeltas::new(
            aapl,
            BookType::L2_MBP,
            Some(client_id),
            None,
            id(),
            ts,
            None,
            false,
            None,
            None,
        ))
        .unwrap();
    let Data::BookDeltas(deltas) = next_data(&mut events).await else {
        panic!("expected book deltas");
    };
    assert_eq!(deltas.instrument_id, aapl);
    assert_eq!(deltas.deltas[0].action, BookAction::Clear);
    assert_eq!(deltas.deltas.len(), 41);

    client
        .subscribe_quotes(SubscribeQuotes::new(
            aapl,
            Some(client_id),
            None,
            id(),
            ts,
            None,
            None,
        ))
        .unwrap();
    let Data::Quote(quote) = next_data(&mut events).await else {
        panic!("expected a quote");
    };
    assert_eq!(quote.bid_price, Price::from("348.08"));
    assert!(quote.ask_price > quote.bid_price);

    client
        .subscribe_trades(SubscribeTrades::new(
            crude,
            Some(client_id),
            None,
            id(),
            ts,
            None,
            None,
        ))
        .unwrap();
    let Data::Trade(trade) = next_data(&mut events).await else {
        panic!("expected a trade");
    };
    assert_eq!(trade.instrument_id, crude);
    assert_eq!(trade.price, Price::from("85.32"));
    assert_eq!(trade.aggressor_side, AggressorSide::Buy);

    client
        .subscribe_mark_prices(SubscribeMarkPrices::new(
            aapl,
            Some(client_id),
            None,
            id(),
            ts,
            None,
            None,
        ))
        .unwrap();
    assert!(matches!(next_data(&mut events).await, Data::MarkPrice(_)));
    let sent = |kind: &str, channel: &str, symbol: &str| {
        (kind.to_string(), channel.to_string(), symbol.to_string())
    };
    let subscribed = [
        sent("subscribe", "level2", "AAPL-USD"),
        sent("subscribe", "bbo", "AAPL-USD"),
        sent("subscribe", "trade", "CL-USD"),
        sent("subscribe", "mark_price", "AAPL-USD"),
    ];
    assert_eq!(venue.wait_for_requests(4).await, subscribed);

    // The venue drops the socket: the client connects again and asks for everything again.
    venue.drop_connection.notify_waiters();
    let mut again = venue.wait_for_requests(8).await.split_off(4);
    again.sort();
    let mut expected = subscribed.to_vec();
    expected.sort();
    assert_eq!(again, expected);
    for _ in 0..4 {
        next_data(&mut events).await;
    }

    client
        .request_book_snapshot(RequestBookSnapshot::new(
            aapl,
            None,
            Some(client_id),
            id(),
            ts,
            None,
        ))
        .unwrap();
    match next_event(&mut events).await {
        DataEvent::Response(DataResponse::Book(response)) => {
            assert_eq!(response.instrument_id, aapl);
            // The REST book lists empty levels too (`348` with size `0`): they are left out.
            assert_eq!(response.data.best_bid_price(), Some(Price::from("348.01")));
            assert!(response.data.best_ask_price() > response.data.best_bid_price());
        }
        other => panic!("expected a book response, was {other:?}"),
    }

    let bar_type = BarType::from("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL");
    client
        .request_bars(RequestBars::new(
            bar_type,
            None,
            None,
            None,
            Some(client_id),
            id(),
            ts,
            None,
        ))
        .unwrap();
    match next_event(&mut events).await {
        DataEvent::Response(DataResponse::Bars(response)) => {
            assert_eq!(response.bar_type, bar_type);
            assert!(!response.data.is_empty());
        }
        other => panic!("expected bars, was {other:?}"),
    }

    client
        .unsubscribe_quotes(&UnsubscribeQuotes::new(
            aapl,
            Some(client_id),
            None,
            id(),
            ts,
            None,
            None,
        ))
        .unwrap();
    assert_eq!(
        venue.wait_for_requests(9).await[8],
        sent("unsubscribe", "bbo", "AAPL-USD")
    );

    client.disconnect().await.unwrap();
    assert!(client.is_disconnected());
    let failures = venue.failures.lock();
    assert!(failures.is_empty(), "{failures:?}");
}

/// A connected client; the instrument events of the connect are taken off the stream.
async fn connected() -> (QfexDataClient, UnboundedReceiver<DataEvent>) {
    let (config, _) = start_venue().await;
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    replace_data_event_sender(sender);
    let mut client = QfexDataClient::new(ClientId::from("QFEX"), config).unwrap();
    client.connect().await.unwrap();
    while events.try_recv().is_ok() {}
    (client, events)
}

#[tokio::test(flavor = "multi_thread")]
async fn book_depth_is_ten_levels_of_the_pulsed_book() {
    let (mut client, mut events) = connected().await;
    let aapl = InstrumentId::from("AAPL-USD-PERP.QFEX");

    client
        .subscribe_book_depth(SubscribeBookDepth::new(
            aapl,
            BookType::L2_MBP,
            Some(ClientId::from("QFEX")),
            None,
            UUID4::new(),
            UnixNanos::default(),
            None,
            false,
            None,
            None,
        ))
        .unwrap();

    // The recorded `level2` frame has 20 levels a side. It also parses to deltas, which
    // nobody subscribed to: the depth is the only event.
    let Data::BookDepth(depth) = next_data(&mut events).await else {
        panic!("expected a depth");
    };
    assert_eq!(depth.instrument_id, aapl);
    assert_eq!((depth.bids.len(), depth.asks.len()), (10, 10));
    assert_eq!(depth.bids[0].price, Price::from("348.08"));
    assert_eq!(depth.bids[9].price, Price::from("347.77"));
    assert_eq!(depth.asks[0].price, Price::from("348.11"));
    assert_eq!(depth.asks[9].price, Price::from("348.39"));
    assert!(events.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn instrument_status_comes_from_refdata() {
    let (mut client, mut events) = connected().await;
    let subscribe = |symbol: &str| {
        SubscribeInstrumentStatus::new(
            InstrumentId::from(symbol),
            Some(ClientId::from("QFEX")),
            None,
            UUID4::new(),
            UnixNanos::default(),
            None,
            None,
        )
    };

    // The recorded `/refdata` has AAPL-USD as ACTIVE and KIA-KRW as INACTIVE.
    client
        .subscribe_instrument_status(subscribe("AAPL-USD-PERP.QFEX"))
        .unwrap();
    let Data::InstrumentStatus(status) = next_data(&mut events).await else {
        panic!("expected a status");
    };
    assert_eq!(status.action, MarketStatusAction::Trading);
    assert_eq!(status.is_trading, Some(true));

    // The second subscription reads `/refdata` again: the unchanged status of the first
    // instrument, which comes first in the list, is not sent a second time.
    client
        .subscribe_instrument_status(subscribe("KIA-KRW-PERP.QFEX"))
        .unwrap();
    let Data::InstrumentStatus(status) = next_data(&mut events).await else {
        panic!("expected a status");
    };
    assert_eq!(status.instrument_id.to_string(), "KIA-KRW-PERP.QFEX");
    assert_eq!(status.action, MarketStatusAction::Suspend);
    assert_eq!(status.is_trading, Some(false));
    assert!(events.try_recv().is_err());
}

/// The next socket state of the data client, checked to come from its market data socket.
async fn next_state(
    system: &mut UnboundedReceiver<SystemEvent>,
    client_id: ClientId,
) -> SocketState {
    let event = tokio::time::timeout(Duration::from_secs(10), system.recv())
        .await
        .expect("no socket state within 10 s")
        .expect("system event channel closed");
    let SystemEvent::SocketState(change) = event;
    assert_eq!(change.client_id, client_id);
    assert_eq!(change.venue, Some(VenueId::from("QFEX")));
    assert_eq!(change.endpoint.as_str(), "market_data");
    change.state
}

#[tokio::test(flavor = "multi_thread")]
async fn socket_state_follows_the_market_data_socket() {
    let (config, venue) = start_venue().await;
    let (sender, _events) = tokio::sync::mpsc::unbounded_channel();
    replace_data_event_sender(sender);
    // The control reads the system event sender when the client is built.
    let (sender, mut system) = tokio::sync::mpsc::unbounded_channel();
    replace_system_event_sender(sender);
    let client_id = ClientId::from("QFEX-DATA");
    let mut client = QfexDataClient::new(client_id, config).unwrap();

    client.connect().await.unwrap();
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );

    venue.drop_connection.notify_one();
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Disconnected
    );
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );

    // A deliberate disconnect is not a loss.
    client.disconnect().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(system.try_recv().is_err(), "disconnect reported a state");

    // `close()` built a new socket: it still reports.
    client.connect().await.unwrap();
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );
    client.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnect_command_reconnects_the_market_data_socket() {
    let (config, venue) = start_venue().await;
    let (sender, _events) = tokio::sync::mpsc::unbounded_channel();
    replace_data_event_sender(sender);
    let (sender, mut system) = tokio::sync::mpsc::unbounded_channel();
    replace_system_event_sender(sender);
    let client_id = ClientId::from("QFEX-DATA");
    // A `LiveNode` builds its data clients inside its registry scope.
    let registry = SocketReconnectRegistry::default();
    let mut client = registry
        .scope(|| QfexDataClient::new(client_id, config))
        .unwrap();
    let request = || {
        registry
            .handle(client_id, "market_data".into())
            .expect("market_data has a reconnect handle")
            .request_reconnect()
    };
    assert_eq!(
        request(),
        SocketReconnectRequestOutcome::Closed,
        "before connect"
    );

    client.connect().await.unwrap();
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );
    let aapl = InstrumentId::from("AAPL-USD-PERP.QFEX");
    client
        .subscribe_quotes(SubscribeQuotes::new(
            aapl,
            Some(client_id),
            None,
            UUID4::new(),
            UnixNanos::default(),
            None,
            None,
        ))
        .unwrap();
    let subscribed = venue.wait_for_requests(1).await;

    assert_eq!(request(), SocketReconnectRequestOutcome::Accepted);
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Disconnected
    );
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );
    let seen = venue.wait_for_requests(2 * subscribed.len()).await;
    assert_eq!(
        seen[subscribed.len()..],
        subscribed[..],
        "subscriptions sent again"
    );

    client.disconnect().await.unwrap();
    assert_ne!(
        request(),
        SocketReconnectRequestOutcome::Accepted,
        "after disconnect"
    );

    // `close()` built a new socket: the command reaches it too.
    client.connect().await.unwrap();
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );
    assert_eq!(request(), SocketReconnectRequestOutcome::Accepted);
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Disconnected
    );
    assert_eq!(
        next_state(&mut system, client_id).await,
        SocketState::Connected
    );
    client.disconnect().await.unwrap();
}
