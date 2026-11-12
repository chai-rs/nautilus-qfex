//! Models of the QFEX REST payloads. Only the fields the adapter reads are declared.
//!
//! Numbers arrive as strings on the public endpoints and as JSON numbers on the
//! authenticated ones; [`Decimal`] reads both.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use ustr::Ustr;

use crate::common::enums::{
    QfexOrderStatus, QfexOrderType, QfexSide, QfexSymbolStatus, QfexTimeInForce,
};

/// Reads a value, giving `None` when it is absent, null or not a known variant.
///
/// For fields the adapter can do without: an unexpected value must not drop the message.
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

/// `GET /refdata`. Items stay raw so that one unreadable symbol does not hide the others.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexRefData {
    pub data: Vec<serde_json::Value>,
}

/// One symbol of `GET /refdata`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexSymbol {
    pub symbol: Ustr,
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub min_quantity: Decimal,
    pub max_quantity: Decimal,
    pub base_asset: Ustr,
    pub quote_asset: Ustr,
    pub margin_asset: Ustr,
    pub default_max_leverage: Decimal,
    pub status: QfexSymbolStatus,
    #[serde(default)]
    pub product_category: String,
}

/// `GET /md/orderbook/{ticker_id}`. Levels are `[price, size]`; a size of zero is an empty
/// level.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexOrderBook {
    pub ticker_id: Ustr,
    /// Unix milliseconds.
    pub timestamp: u64,
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
}

/// `GET /candles/{symbol}`, newest first.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexCandles {
    pub candles: Vec<QfexCandle>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QfexCandle {
    /// Open time of the candle (RFC 3339).
    pub started_at: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    /// Zero on every candle captured on 11 Nov 2026; see `usd_volume`.
    #[serde(default)]
    pub base_token_volume: Decimal,
    #[serde(default)]
    pub usd_volume: Decimal,
}

/// `GET /funding/{symbol}`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexFundingRates {
    pub data: Vec<QfexFundingRate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QfexFundingRate {
    pub interval_minutes: u16,
    /// Start of the funding window (RFC 3339).
    pub window_start: String,
    pub rate: Decimal,
}

/// Error body (`application/problem+json`).
#[derive(Debug, Clone, Deserialize)]
pub struct QfexProblem {
    pub title: Option<String>,
    pub status: Option<u16>,
    pub detail: Option<String>,
}

/// `GET /user/positions`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexUserPositions {
    pub balance: QfexBalance,
    #[serde(default)]
    pub positions: Vec<QfexPosition>,
}

/// Account balance: REST `balance` and the `balance_response` of the trade socket.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QfexBalance {
    pub available_balance: Decimal,
    #[serde(default)]
    pub order_margin: Decimal,
    #[serde(default)]
    pub position_margin: Decimal,
}

/// One position: REST `positions` and the `position_response` of the trade socket.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QfexPosition {
    pub symbol: Ustr,
    /// Signed size: negative is short.
    pub position: Decimal,
    #[serde(default)]
    pub average_price: Decimal,
}

/// `GET /user/trade`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexTrades {
    pub data: Vec<QfexTrade>,
}

/// One fill of the account.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexTrade {
    pub id: String,
    /// Unix time; the docs show seconds with a fraction.
    pub order_timestamp: Decimal,
    pub symbol: Ustr,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: QfexSide,
    #[serde(default)]
    pub fee: Decimal,
    #[serde(default, deserialize_with = "lenient")]
    pub order_type: Option<QfexOrderType>,
    pub order_id: String,
}

/// `GET /user/historic-orders`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexHistoricOrders {
    pub data: Vec<QfexHistoricOrder>,
}

/// One order that reached a final state.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexHistoricOrder {
    pub order_id: String,
    pub terminal_status: QfexOrderStatus,
    /// Time of the final status (RFC 3339).
    pub status_ts: String,
    pub symbol: Ustr,
    pub side: QfexSide,
    #[serde(rename = "type")]
    pub order_type: QfexOrderType,
    #[serde(default, deserialize_with = "lenient")]
    pub time_in_force: Option<QfexTimeInForce>,
    pub quantity: Decimal,
    #[serde(default)]
    pub price: Decimal,
    #[serde(default)]
    pub client_order_id: String,
    #[serde(default)]
    pub filled_qty: Decimal,
    pub avg_price: Option<Decimal>,
}

/// `GET /user/fees`.
#[derive(Debug, Clone, Deserialize)]
pub struct QfexFees {
    pub fees: ahash::AHashMap<Ustr, QfexFeeRate>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct QfexFeeRate {
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
}

/// Body of `POST /user/leverage`.
#[derive(Debug, Clone, Serialize)]
pub struct QfexLeverageRequest {
    pub symbol: Ustr,
    #[serde(with = "rust_decimal::serde::float")]
    pub leverage: Decimal,
}

#[cfg(test)]
pub(crate) mod tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;

    pub(crate) fn fixture_json(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/http/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(path).unwrap()
    }

    pub(crate) fn fixture<T: DeserializeOwned>(name: &str) -> T {
        serde_json::from_str(&fixture_json(name)).unwrap()
    }

    #[rstest]
    fn test_refdata() {
        let refdata: QfexRefData = fixture("refdata");
        let symbols: Vec<QfexSymbol> = refdata
            .data
            .into_iter()
            .map(|item| serde_json::from_value(item).unwrap())
            .collect();
        let aapl = symbols.iter().find(|s| s.symbol == "AAPL-USD").unwrap();
        assert_eq!(aapl.tick_size, dec!(0.01));
        assert_eq!(aapl.lot_size, dec!(0.001));
        assert_eq!(aapl.default_max_leverage, dec!(20));
        assert_eq!(aapl.status, QfexSymbolStatus::Active);
        assert_eq!(aapl.margin_asset, "USDC");
    }

    #[rstest]
    fn test_order_book() {
        let book: QfexOrderBook = fixture("orderbook");
        assert_eq!(book.ticker_id, "AAPL-USD");
        assert_eq!(book.timestamp, 1_794_336_612_802);
        assert_eq!(book.bids[0], (dec!(348.01), dec!(10.651)));
        assert!(book.bids.iter().any(|(_, size)| size.is_zero()));
    }

    #[rstest]
    fn test_candles_and_funding() {
        let candles: QfexCandles = fixture("candles");
        assert_eq!(candles.candles[0].started_at, "2026-11-10T15:04:00Z");
        assert_eq!(candles.candles[0].usd_volume, dec!(29046.90685));

        // Rates are JSON numbers, one of them in exponent form.
        let funding: QfexFundingRates = fixture("funding");
        assert_eq!(funding.data[0].interval_minutes, 60);
        assert_eq!(funding.data[0].rate, dec!(-0.00000186));
        assert!(funding.data.iter().any(|r| r.rate == dec!(-0.00000097)));
    }

    // Hand-made from the OpenAPI schemas: no authenticated response was ever requested.
    #[rstest]
    fn test_authenticated_models() {
        let account: QfexUserPositions = fixture("user_positions");
        assert_eq!(account.balance.available_balance, dec!(9500.25));
        assert_eq!(account.positions[0].position, dec!(-1.5));

        let trades: QfexTrades = fixture("user_trades");
        assert_eq!(trades.data[0].side, QfexSide::Buy);
        assert_eq!(trades.data[0].order_type, Some(QfexOrderType::Limit));
        assert_eq!(trades.data[0].order_timestamp, dec!(1769439642.8484282));

        let orders: QfexHistoricOrders = fixture("historic_orders");
        assert_eq!(orders.data[0].time_in_force, Some(QfexTimeInForce::Gtc));
        assert_eq!(orders.data[1].time_in_force, None);
        assert_eq!(orders.data[1].avg_price, None);

        let fees: QfexFees = fixture("user_fees");
        assert_eq!(fees.fees[&Ustr::from("AAPL-USD")].taker_fee, dec!(0.0005));
    }

    #[rstest]
    fn test_leverage_request_sends_a_number() {
        let body = QfexLeverageRequest {
            symbol: Ustr::from("AAPL-USD"),
            leverage: dec!(5),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"symbol":"AAPL-USD","leverage":5.0}"#
        );
    }
}
