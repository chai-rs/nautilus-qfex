//! Frames of the two QFEX sockets: market data (`mds`) and trade.
//!
//! MDS frames carry a `type`; trade frames are an object with one key that names the payload
//! (`{"order_response":{..}}`). [`QfexWsMessage::parse`] reads both.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use ustr::Ustr;

use crate::{
    book::QfexLevel,
    common::enums::{
        QfexMdsChannel, QfexOrderStatus, QfexOrderType, QfexSide, QfexTimeInForce,
        QfexTradeChannel, QfexWsErrorCode,
    },
    http::models::lenient,
};

/// `level2` (20 levels a side) and `bbo` (one level a side). Both are whole snapshots.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsBook {
    #[serde(default)]
    pub sequence: u64,
    /// RFC 3339.
    pub time: String,
    pub symbol: Ustr,
    #[serde(default)]
    pub bid: Vec<QfexLevel>,
    #[serde(default)]
    pub ask: Vec<QfexLevel>,
}

/// Public `trade`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsTrade {
    #[serde(default)]
    pub sequence: u64,
    pub trade_id: String,
    pub time: String,
    pub symbol: Ustr,
    pub size: Decimal,
    pub price: Decimal,
    /// Taker side (assumed: the docs do not say). Upper case live, lower case in the docs.
    #[serde(default, deserialize_with = "lenient")]
    pub side: Option<QfexSide>,
}

/// `mark_price` and `underlier` (index price).
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsPrice {
    pub time: String,
    pub symbol: Ustr,
    pub price: Decimal,
}

/// `funding`: the implied rate of the hour that is running.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsFunding {
    pub time: String,
    pub symbol: Ustr,
    pub funding_rate: Decimal,
    /// Seconds until the rate is paid.
    #[serde(default)]
    pub time_remaining: u64,
}

/// `order_response`, and one row of `all_orders_response`.
///
/// Only `status` is required: the docs show no rejection example, so nothing else is relied on.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsOrder {
    #[serde(default)]
    pub order_id: String,
    #[serde(default)]
    pub client_order_id: String,
    #[serde(default)]
    pub symbol: Ustr,
    pub status: QfexOrderStatus,
    #[serde(default, deserialize_with = "lenient")]
    pub quantity: Option<Decimal>,
    #[serde(default, deserialize_with = "lenient")]
    pub quantity_remaining: Option<Decimal>,
    /// Limit price; trigger price of a stop order.
    #[serde(default, deserialize_with = "lenient")]
    pub price: Option<Decimal>,
    #[serde(default, deserialize_with = "lenient")]
    pub side: Option<QfexSide>,
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    pub order_type: Option<QfexOrderType>,
    #[serde(default, deserialize_with = "lenient")]
    pub time_in_force: Option<QfexTimeInForce>,
    #[serde(default, deserialize_with = "lenient")]
    pub reduce_only: Option<bool>,
    /// Unix seconds, with a fraction.
    #[serde(default, deserialize_with = "lenient")]
    pub update_time: Option<Decimal>,
}

/// `fill_response`: one execution of an own order.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsFill {
    pub trade_id: String,
    pub order_id: String,
    #[serde(default)]
    pub client_order_id: String,
    pub symbol: Ustr,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: QfexSide,
    #[serde(default, deserialize_with = "lenient")]
    pub aggressor_side: Option<QfexSide>,
    #[serde(default)]
    pub fee: Decimal,
    #[serde(default, deserialize_with = "lenient")]
    pub order_type: Option<QfexOrderType>,
    /// Unix seconds, with a fraction.
    pub timestamp: Decimal,
}

/// `balance_response`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsBalance {
    pub available_balance: Decimal,
    #[serde(default)]
    pub order_margin: Decimal,
    #[serde(default)]
    pub position_margin: Decimal,
}

/// `position_response`. `position` is signed: negative is short.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsPosition {
    pub symbol: Ustr,
    pub position: Decimal,
    #[serde(default)]
    pub average_price: Decimal,
}

/// `all_orders_response`: the answer to `get_user_orders` and `get_order`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct QfexWsOrders {
    #[serde(default)]
    pub orders: Vec<QfexWsOrder>,
    #[serde(default)]
    pub stop_orders: Vec<QfexWsOrder>,
}

/// An error frame. The docs show three shapes: `{"err":{..}}`, `{"type":"error",..}` and
/// `{"type":"Err",..}`; all of them land here.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexWsError {
    #[serde(default, deserialize_with = "lenient")]
    pub error_code: Option<QfexWsErrorCode>,
    #[serde(default)]
    pub message: String,
    /// The request the venue refused, when it sends it back.
    #[serde(default)]
    pub incoming_message: Value,
}

impl QfexWsError {
    /// `client_order_id` of the refused request, if any.
    #[must_use]
    pub fn client_order_id(&self) -> Option<&str> {
        self.incoming_message["params"]["client_order_id"]
            .as_str()
            .filter(|id| !id.is_empty())
    }

    /// `type` of the refused request (`add_order`, `cancel_order`, ...).
    #[must_use]
    pub fn request_type(&self) -> Option<&str> {
        self.incoming_message["type"].as_str()
    }
}

/// A frame from either socket.
#[derive(Debug, Clone)]
pub enum QfexWsMessage {
    /// The connection was re-established; not a venue frame.
    Reconnected,
    /// `{"type":"auth","result":"success"}`.
    Authenticated,
    /// A subscribe or unsubscribe acknowledgement.
    Subscribed,
    Level2(QfexWsBook),
    Bbo(QfexWsBook),
    Trade(QfexWsTrade),
    MarkPrice(QfexWsPrice),
    Underlier(QfexWsPrice),
    Funding(QfexWsFunding),
    Order(Box<QfexWsOrder>),
    Fill(Box<QfexWsFill>),
    Balance(QfexWsBalance),
    Position(QfexWsPosition),
    Orders(QfexWsOrders),
    Error(QfexWsError),
    /// A frame the adapter has no use for; holds its `type` or its first key.
    Other(String),
}

fn payload<T: DeserializeOwned>(value: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(value)
}

impl QfexWsMessage {
    /// Reads one text frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the text is not a JSON object or a known payload has the wrong
    /// shape.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let mut frame: serde_json::Map<String, Value> = serde_json::from_str(text)?;

        let kind = frame.get("type").and_then(Value::as_str).map(str::to_owned);
        if let Some(kind) = kind {
            let frame = Value::Object(frame);
            return Ok(match kind.as_str() {
                "level2" => Self::Level2(payload(frame)?),
                "bbo" => Self::Bbo(payload(frame)?),
                "trade" => Self::Trade(payload(frame)?),
                "mark_price" => Self::MarkPrice(payload(frame)?),
                "underlier" => Self::Underlier(payload(frame)?),
                "funding" => Self::Funding(payload(frame)?),
                "subscribed" | "unsubscribed" => Self::Subscribed,
                "auth" if frame["result"] == "success" => Self::Authenticated,
                // A failed auth is an error whatever its shape.
                "auth" | "error" | "Err" => Self::Error(payload(frame)?),
                _ => Self::Other(kind),
            });
        }

        let mut take = |key: &str| frame.remove(key);
        Ok(if let Some(value) = take("order_response") {
            Self::Order(payload(value)?)
        } else if let Some(value) = take("fill_response") {
            Self::Fill(payload(value)?)
        } else if let Some(value) = take("balance_response") {
            Self::Balance(payload(value)?)
        } else if let Some(value) = take("position_response") {
            Self::Position(payload(value)?)
        } else if let Some(value) = take("all_orders_response") {
            Self::Orders(payload(value)?)
        } else if let Some(value) = take("err") {
            Self::Error(payload(value)?)
        } else if take("subscribed").is_some() || take("unsubscribed").is_some() {
            Self::Subscribed
        } else {
            Self::Other(frame.keys().next().cloned().unwrap_or_default())
        })
    }
}

/// `subscribe` / `unsubscribe` on the market data socket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QfexMdsRequest<'a> {
    Subscribe {
        channels: &'a [QfexMdsChannel],
        symbols: &'a [Ustr],
    },
    Unsubscribe {
        channels: &'a [QfexMdsChannel],
        symbols: &'a [Ustr],
    },
}

/// How `cancel_order` reads its `order_id` field.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QfexCancelIdType {
    OrderId,
    ClientOrderId,
}

/// `add_order`. For `TAKE_PROFIT` / `STOP_LOSS`, `price` is the trigger price.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QfexAddOrder {
    pub symbol: Ustr,
    pub side: QfexSide,
    pub order_type: QfexOrderType,
    pub order_time_in_force: QfexTimeInForce,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    #[serde(
        with = "rust_decimal::serde::float_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<Decimal>,
    pub client_order_id: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reduce_only: bool,
}

/// `modify_order`. The venue answers `MODIFIED` with a new `order_id`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QfexModifyOrder {
    pub order_id: String,
    pub symbol: Ustr,
    pub side: QfexSide,
    pub order_type: QfexOrderType,
    /// Must repeat the flag of the resting order.
    pub reduce_only: bool,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    #[serde(
        with = "rust_decimal::serde::float_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<Decimal>,
}

/// `modify_stop_order`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QfexModifyStopOrder {
    pub stop_order_id: String,
    pub symbol: Ustr,
    pub side: QfexSide,
    pub order_type: QfexOrderType,
    #[serde(with = "rust_decimal::serde::float")]
    pub quantity: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
}

/// A request on the trade socket: `{"type": .., "params": {..}}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "params", rename_all = "snake_case")]
pub enum QfexTradeRequest {
    Subscribe {
        channels: Vec<QfexTradeChannel>,
    },
    AddOrder(QfexAddOrder),
    ModifyOrder(QfexModifyOrder),
    CancelOrder {
        order_id: String,
        symbol: Ustr,
        cancel_order_id_type: QfexCancelIdType,
    },
    CancelAllOrders {
        #[serde(skip_serializing_if = "Option::is_none")]
        symbol: Option<Ustr>,
    },
    CancelOnDisconnect {
        cancel_on_disconnect: bool,
    },
    ModifyStopOrder(QfexModifyStopOrder),
    CancelStopOrder {
        stop_order_id: String,
    },
    /// Open orders; the answer is `all_orders_response`.
    GetUserOrders {
        limit: u32,
        offset: u32,
    },
}

impl QfexTradeRequest {
    /// The frame to send.
    #[must_use]
    pub fn to_frame(&self) -> String {
        // Only strings, numbers and booleans: serialising cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;

    /// Lines of `tests/fixtures/ws/{name}.jsonl`, one frame a line.
    pub(crate) fn fixture_lines(name: &str) -> Vec<String> {
        let path = format!(
            "{}/tests/fixtures/ws/{name}.jsonl",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The first frame of a fixture that parses to the wanted variant.
    pub(crate) fn first<T>(name: &str, pick: impl Fn(QfexWsMessage) -> Option<T>) -> T {
        fixture_lines(name)
            .iter()
            .find_map(|line| pick(QfexWsMessage::parse(line).unwrap()))
            .unwrap()
    }

    #[rstest]
    fn test_every_captured_mds_frame_parses() {
        let mut counts = std::collections::BTreeMap::new();
        for line in fixture_lines("mds") {
            let name = match QfexWsMessage::parse(&line).unwrap() {
                QfexWsMessage::Subscribed => "subscribed",
                QfexWsMessage::Level2(_) => "level2",
                QfexWsMessage::Bbo(_) => "bbo",
                QfexWsMessage::Trade(_) => "trade",
                QfexWsMessage::MarkPrice(_) => "mark_price",
                QfexWsMessage::Underlier(_) => "underlier",
                QfexWsMessage::Funding(_) => "funding",
                QfexWsMessage::Other(kind) => {
                    assert_eq!(kind, "open_interest");
                    "other"
                }
                other => panic!("unexpected {other:?}"),
            };
            *counts.entry(name).or_insert(0) += 1;
        }
        for name in [
            "subscribed",
            "level2",
            "bbo",
            "trade",
            "mark_price",
            "underlier",
            "funding",
            "other",
        ] {
            assert!(counts.contains_key(name), "no {name} frame in the fixture");
        }
    }

    #[rstest]
    fn test_mds_fields() {
        let book = first("mds", |m| match m {
            QfexWsMessage::Level2(book) => Some(book),
            _ => None,
        });
        assert_eq!(book.symbol, "AAPL-USD");
        assert_eq!(book.sequence, 1);
        assert_eq!((book.bid.len(), book.ask.len()), (20, 20));
        assert_eq!(book.bid[0], (dec!(348.08), dec!(4.5)));

        let trade = first("mds", |m| match m {
            QfexWsMessage::Trade(trade) => Some(trade),
            _ => None,
        });
        assert_eq!(trade.trade_id, "b99c2a90-5ddf-585a-91ce-e6b8e498a899");
        assert_eq!((trade.price, trade.size), (dec!(85.32), dec!(5.833)));
        assert_eq!(trade.side, Some(QfexSide::Buy));

        let funding = first("mds", |m| match m {
            QfexWsMessage::Funding(funding) => Some(funding),
            _ => None,
        });
        assert_eq!(funding.funding_rate, dec!(0.00001));
        assert_eq!(funding.time_remaining, 385);
    }

    #[rstest]
    fn test_every_trade_socket_frame_of_the_docs_parses() {
        let names: Vec<&str> = fixture_lines("trade")
            .iter()
            .map(|line| match QfexWsMessage::parse(line).unwrap() {
                QfexWsMessage::Authenticated => "auth",
                QfexWsMessage::Subscribed => "subscribed",
                QfexWsMessage::Order(_) => "order",
                QfexWsMessage::Fill(_) => "fill",
                QfexWsMessage::Balance(_) => "balance",
                QfexWsMessage::Position(_) => "position",
                QfexWsMessage::Orders(_) => "orders",
                QfexWsMessage::Error(_) => "error",
                QfexWsMessage::Other(_) => "other",
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            [
                "auth",
                "subscribed",
                "order",
                "order",
                "order",
                "order",
                "fill",
                "balance",
                "position",
                "orders",
                "error",
                "error",
                "error",
                "other",
                "other",
            ]
        );
    }

    #[rstest]
    fn test_trade_socket_fields() {
        let order = first("trade", |m| match m {
            QfexWsMessage::Order(order) => Some(order),
            _ => None,
        });
        assert_eq!(order.order_id, "5b309929-206f-40ec-804d-cbe46e81afc1");
        assert_eq!(order.status, QfexOrderStatus::Ack);
        assert_eq!(order.order_type, Some(QfexOrderType::Limit));
        assert_eq!(order.quantity_remaining, Some(dec!(1)));
        assert_eq!(order.reduce_only, None);

        let fill = first("trade", |m| match m {
            QfexWsMessage::Fill(fill) => Some(fill),
            _ => None,
        });
        assert_eq!((fill.price, fill.quantity), (dec!(25242), dec!(0.004)));
        assert_eq!(fill.fee, dec!(0.00045));
        assert_eq!(fill.timestamp, dec!(1769439642.8484282));

        let error = first("trade", |m| match m {
            QfexWsMessage::Error(error) => Some(error),
            _ => None,
        });
        assert_eq!(error.error_code, Some(QfexWsErrorCode::RateLimited));
        assert_eq!(error.client_order_id(), Some("my-order-001"));
        assert_eq!(error.request_type(), Some("add_order"));
    }

    #[rstest]
    #[case("not json")]
    #[case("[1,2]")]
    #[case(r#"{"type":"level2","symbol":"AAPL-USD"}"#)]
    #[case(r#"{"order_response":{"order_id":"x"}}"#)]
    #[case(r#"{"fill_response":7}"#)]
    fn test_bad_frames_are_errors(#[case] text: &str) {
        assert!(QfexWsMessage::parse(text).is_err());
    }

    #[rstest]
    #[case(r#"{"type":"auth","result":"failure","message":"bad signature"}"#)]
    #[case(r#"{"type":"error","message":"nope"}"#)]
    fn test_failed_auth_is_an_error(#[case] text: &str) {
        assert!(matches!(
            QfexWsMessage::parse(text).unwrap(),
            QfexWsMessage::Error(_)
        ));
    }

    #[rstest]
    fn test_requests_match_the_docs() {
        let symbol = Ustr::from("AAPL-USD");
        let add = QfexAddOrder {
            symbol,
            side: QfexSide::Buy,
            order_type: QfexOrderType::Limit,
            order_time_in_force: QfexTimeInForce::Gtc,
            quantity: dec!(1.5),
            price: Some(dec!(200.01)),
            client_order_id: "O-1".to_string(),
            reduce_only: false,
        };
        let market = QfexAddOrder {
            order_type: QfexOrderType::Market,
            order_time_in_force: QfexTimeInForce::Ioc,
            price: None,
            reduce_only: true,
            ..add.clone()
        };
        let cases = [
            (
                QfexTradeRequest::AddOrder(add),
                json!({"type":"add_order","params":{"symbol":"AAPL-USD","side":"BUY","order_type":"LIMIT","order_time_in_force":"GTC","quantity":1.5,"price":200.01,"client_order_id":"O-1"}}),
            ),
            (
                QfexTradeRequest::AddOrder(market),
                json!({"type":"add_order","params":{"symbol":"AAPL-USD","side":"BUY","order_type":"MARKET","order_time_in_force":"IOC","quantity":1.5,"client_order_id":"O-1","reduce_only":true}}),
            ),
            (
                QfexTradeRequest::ModifyOrder(QfexModifyOrder {
                    order_id: "id".to_string(),
                    symbol,
                    side: QfexSide::Sell,
                    order_type: QfexOrderType::Alo,
                    reduce_only: false,
                    quantity: dec!(2),
                    price: Some(dec!(200)),
                }),
                json!({"type":"modify_order","params":{"order_id":"id","symbol":"AAPL-USD","side":"SELL","order_type":"ALO","reduce_only":false,"quantity":2.0,"price":200.0}}),
            ),
            (
                QfexTradeRequest::CancelOrder {
                    order_id: "id".to_string(),
                    symbol,
                    cancel_order_id_type: QfexCancelIdType::OrderId,
                },
                json!({"type":"cancel_order","params":{"order_id":"id","symbol":"AAPL-USD","cancel_order_id_type":"order_id"}}),
            ),
            (
                QfexTradeRequest::CancelAllOrders { symbol: None },
                json!({"type":"cancel_all_orders","params":{}}),
            ),
            (
                QfexTradeRequest::CancelAllOrders {
                    symbol: Some(symbol),
                },
                json!({"type":"cancel_all_orders","params":{"symbol":"AAPL-USD"}}),
            ),
            (
                QfexTradeRequest::CancelOnDisconnect {
                    cancel_on_disconnect: true,
                },
                json!({"type":"cancel_on_disconnect","params":{"cancel_on_disconnect":true}}),
            ),
            (
                QfexTradeRequest::CancelStopOrder {
                    stop_order_id: "id".to_string(),
                },
                json!({"type":"cancel_stop_order","params":{"stop_order_id":"id"}}),
            ),
            (
                QfexTradeRequest::ModifyStopOrder(QfexModifyStopOrder {
                    stop_order_id: "id".to_string(),
                    symbol,
                    side: QfexSide::Buy,
                    order_type: QfexOrderType::TakeProfit,
                    quantity: dec!(2),
                    price: dec!(200),
                }),
                json!({"type":"modify_stop_order","params":{"stop_order_id":"id","symbol":"AAPL-USD","side":"BUY","order_type":"TAKE_PROFIT","quantity":2.0,"price":200.0}}),
            ),
            (
                QfexTradeRequest::Subscribe {
                    channels: vec![QfexTradeChannel::Fills],
                },
                json!({"type":"subscribe","params":{"channels":["fills"]}}),
            ),
            (
                QfexTradeRequest::GetUserOrders {
                    limit: 1000,
                    offset: 0,
                },
                json!({"type":"get_user_orders","params":{"limit":1000,"offset":0}}),
            ),
        ];
        for (request, expected) in cases {
            let frame: Value = serde_json::from_str(&request.to_frame()).unwrap();
            assert_eq!(frame, expected);
        }

        let mds = QfexMdsRequest::Subscribe {
            channels: &[QfexMdsChannel::Level2, QfexMdsChannel::MarkPrice],
            symbols: &[symbol],
        };
        assert_eq!(
            serde_json::to_string(&mds).unwrap(),
            r#"{"type":"subscribe","channels":["level2","mark_price"],"symbols":["AAPL-USD"]}"#
        );
    }
}
