//! QFEX venue enums mirrored from REST and WebSocket payloads.

use nautilus_model::{
    data::BarType,
    enums::{
        AggregationSource, AggressorSide, AssetClass, BarAggregation, OrderSide, OrderType,
        PriceType, TimeInForce,
    },
};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

/// QFEX API environment.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Display,
    PartialEq,
    Eq,
    Hash,
    AsRefStr,
    EnumIter,
    EnumString,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[strum(ascii_case_insensitive, serialize_all = "lowercase")]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(
        eq,
        eq_int,
        module = "nautilus_qfex",
        from_py_object,
        rename_all = "SCREAMING_SNAKE_CASE"
    )
)]
pub enum QfexEnvironment {
    /// Production (`qfex.com`).
    #[default]
    Mainnet,
    /// User acceptance testing (`qfex.io`).
    Uat,
}

/// Order direction. The docs show both `BUY` and `buy` in payloads.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum QfexSide {
    #[serde(alias = "buy", alias = "Buy")]
    Buy,
    #[serde(alias = "sell", alias = "Sell")]
    Sell,
}

impl QfexSide {
    #[must_use]
    pub const fn as_order_side(self) -> OrderSide {
        match self {
            Self::Buy => OrderSide::Buy,
            Self::Sell => OrderSide::Sell,
        }
    }

    #[must_use]
    pub const fn as_aggressor_side(self) -> AggressorSide {
        match self {
            Self::Buy => AggressorSide::Buy,
            Self::Sell => AggressorSide::Sell,
        }
    }
}

impl TryFrom<OrderSide> for QfexSide {
    type Error = anyhow::Error;

    fn try_from(side: OrderSide) -> anyhow::Result<Self> {
        match side {
            OrderSide::Buy => Ok(Self::Buy),
            OrderSide::Sell => Ok(Self::Sell),
        }
    }
}

/// Order type. `add_order` accepts the first five; the rest appear only in responses.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum QfexOrderType {
    Limit,
    Market,
    /// Add liquidity only (post-only).
    Alo,
    TakeProfit,
    StopLoss,
    /// Market order generated when a stop order triggers.
    StopMarket,
    TwapLimit,
    #[serde(other)]
    Unknown,
}

impl QfexOrderType {
    /// Whether this is a stop order (staged at the venue, triggered on the mark price).
    #[must_use]
    pub const fn is_stop(self) -> bool {
        matches!(self, Self::TakeProfit | Self::StopLoss)
    }

    /// Returns the Nautilus order type and whether the order is post-only.
    #[must_use]
    pub const fn as_order_type(self) -> (OrderType, bool) {
        match self {
            Self::Limit | Self::TwapLimit | Self::Unknown => (OrderType::Limit, false),
            Self::Alo => (OrderType::Limit, true),
            Self::Market | Self::StopMarket => (OrderType::Market, false),
            Self::TakeProfit => (OrderType::MarketIfTouched, false),
            Self::StopLoss => (OrderType::StopMarket, false),
        }
    }
}

/// Time in force. Market and stop orders are IOC only, ALO is GTC only.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum QfexTimeInForce {
    Gtc,
    Ioc,
    Fok,
}

impl QfexTimeInForce {
    #[must_use]
    pub const fn as_time_in_force(self) -> TimeInForce {
        match self {
            Self::Gtc => TimeInForce::Gtc,
            Self::Ioc => TimeInForce::Ioc,
            Self::Fok => TimeInForce::Fok,
        }
    }
}

impl TryFrom<TimeInForce> for QfexTimeInForce {
    type Error = anyhow::Error;

    fn try_from(tif: TimeInForce) -> anyhow::Result<Self> {
        match tif {
            TimeInForce::Gtc => Ok(Self::Gtc),
            TimeInForce::Ioc => Ok(Self::Ioc),
            TimeInForce::Fok => Ok(Self::Fok),
            other => anyhow::bail!("time in force {other} is not supported by QFEX"),
        }
    }
}

/// What an [`QfexOrderStatus`] means for the order it is about.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QfexStatusKind {
    /// The order rests or was taken in.
    Accepted,
    /// A fill happened; the fill itself arrives on the `fills` channel.
    Filled,
    /// A fill happened and the rest of the order is gone.
    FilledRestCanceled,
    /// The order was replaced; the response carries the new order id.
    Modified,
    Canceled,
    /// A modify request was refused; the order is unchanged.
    ModifyRejected,
    /// A cancel request was refused.
    CancelRejected,
    /// The request was refused. Which request is decided by the state of the order.
    Rejected,
}

/// Status of an `order_response` (Enums page, 11 Nov 2026).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, EnumIter, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum QfexOrderStatus {
    Ack,
    IocCancelled,
    InvalidPrice,
    InvalidQuantity,
    NotFound,
    IocPartiallyFilled,
    UserNotFound,
    PermissionDenied,
    /// Sent for partial fills too: `quantity_remaining` tells them apart.
    Filled,
    Modified,
    Cancelled,
    CancelledStp,
    Rejected,
    NoSuchOrder,
    InvalidOrderType,
    BadSymbol,
    PriceLessThanMinPrice,
    PriceGreaterThanMaxPrice,
    CannotModifyNoSuchOrder,
    CannotModifyPartialFill,
    CannotModifyAloWouldCross,
    FailedMarginCheck,
    RejectedMustChangeValueModify,
    NoChangeNoUpdate,
    InvalidTickSizePrecisionPrice,
    InvalidTickSizePrecisionQuantity,
    QuantityLessThanMinQuantity,
    QuantityGreaterThanMaxQuantity,
    RejectedGreaterThanMaxPriceBand,
    RejectedLessThanMinPriceBand,
    InvalidTimeInForce,
    RejectedWouldBreachMaxPosition,
    RejectedWouldBreachMaxNotional,
    RejectedMarketClosed,
    UserInLiquidation,
    RejectedFailedToProcess,
    InvalidTakeProfitPrice,
    InvalidStopLossPrice,
    InvalidClientOrderId,
    RateLimited,
    RejectedTooManyOpenOrders,
    RejectedOpenInterestLimit,
    RejectedTotalOpenInterestLimit,
    RejectedLessThanMinNotional,
    /// A status this adapter does not know; treated as a rejection.
    #[serde(other)]
    Unknown,
}

impl QfexOrderStatus {
    #[must_use]
    pub const fn kind(self) -> QfexStatusKind {
        match self {
            Self::Ack => QfexStatusKind::Accepted,
            Self::Filled => QfexStatusKind::Filled,
            Self::IocPartiallyFilled => QfexStatusKind::FilledRestCanceled,
            Self::Modified => QfexStatusKind::Modified,
            Self::Cancelled | Self::CancelledStp | Self::IocCancelled => QfexStatusKind::Canceled,
            Self::CannotModifyNoSuchOrder
            | Self::CannotModifyPartialFill
            | Self::CannotModifyAloWouldCross
            | Self::RejectedMustChangeValueModify
            | Self::NoChangeNoUpdate => QfexStatusKind::ModifyRejected,
            Self::NoSuchOrder | Self::NotFound => QfexStatusKind::CancelRejected,
            _ => QfexStatusKind::Rejected,
        }
    }
}

/// Symbol status in `/refdata`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum QfexSymbolStatus {
    Active,
    Inactive,
    Delisted,
    #[serde(other)]
    Unknown,
}

/// Maps the `product_category` of `/refdata` to an asset class.
#[must_use]
pub fn asset_class_of(product_category: &str) -> AssetClass {
    match product_category {
        "EQUITY" => AssetClass::Equity,
        "INDEX" => AssetClass::Index,
        "COMMODITY" => AssetClass::Commodity,
        "FX" => AssetClass::FX,
        "CRYPTO" => AssetClass::Cryptocurrency,
        _ => AssetClass::Alternative,
    }
}

/// Error code of a WebSocket error frame (Errors page, 11 Nov 2026).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, EnumIter, Serialize, Deserialize)]
pub enum QfexWsErrorCode {
    RateLimited,
    #[serde(rename = "InvalidJSONFormat")]
    #[strum(serialize = "InvalidJSONFormat")]
    InvalidJsonFormat,
    AlreadyAuthenticated,
    InvalidParameter,
    PermissionDenied,
    ServerError,
    InvalidOrder,
    InvalidOrderId,
    KycRequired,
    TncRequired,
    #[serde(other)]
    Unknown,
}

/// Public market data channel of `wss://mds.qfex.com`.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, AsRefStr, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum QfexMdsChannel {
    /// 20 levels per side, a full snapshot every pulse.
    Level2,
    Bbo,
    Trade,
    MarkPrice,
    /// Price of the underlying (index price).
    Underlier,
    Funding,
}

/// Private channel of the trade WebSocket.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, EnumIter, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum QfexTradeChannel {
    OrderResponses,
    Fills,
    Positions,
    Balances,
}

/// Candle resolution of `GET /candles/{symbol}`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, AsRefStr, EnumIter, Serialize, Deserialize)]
pub enum QfexBarInterval {
    #[serde(rename = "1MIN")]
    #[strum(serialize = "1MIN")]
    Minute1,
    #[serde(rename = "5MINS")]
    #[strum(serialize = "5MINS")]
    Minute5,
    #[serde(rename = "15MINS")]
    #[strum(serialize = "15MINS")]
    Minute15,
    #[serde(rename = "30MINS")]
    #[strum(serialize = "30MINS")]
    Minute30,
    #[serde(rename = "1HOUR")]
    #[strum(serialize = "1HOUR")]
    Hour1,
    #[serde(rename = "4HOURS")]
    #[strum(serialize = "4HOURS")]
    Hour4,
    #[serde(rename = "1DAY")]
    #[strum(serialize = "1DAY")]
    Day1,
}

impl QfexBarInterval {
    /// Length of one bar in nanoseconds.
    #[must_use]
    pub const fn as_nanos(self) -> u64 {
        const MINUTE: u64 = 60_000_000_000;
        match self {
            Self::Minute1 => MINUTE,
            Self::Minute5 => 5 * MINUTE,
            Self::Minute15 => 15 * MINUTE,
            Self::Minute30 => 30 * MINUTE,
            Self::Hour1 => 60 * MINUTE,
            Self::Hour4 => 240 * MINUTE,
            Self::Day1 => 1440 * MINUTE,
        }
    }
}

impl TryFrom<&BarType> for QfexBarInterval {
    type Error = anyhow::Error;

    fn try_from(bar_type: &BarType) -> anyhow::Result<Self> {
        let spec = bar_type.spec();
        anyhow::ensure!(
            spec.price_type == PriceType::Last
                && bar_type.aggregation_source() == AggregationSource::External,
            "QFEX serves LAST-EXTERNAL bars only, was {bar_type}"
        );
        match (spec.step.get(), spec.aggregation) {
            (1, BarAggregation::Minute) => Ok(Self::Minute1),
            (5, BarAggregation::Minute) => Ok(Self::Minute5),
            (15, BarAggregation::Minute) => Ok(Self::Minute15),
            (30, BarAggregation::Minute) => Ok(Self::Minute30),
            (1, BarAggregation::Hour) => Ok(Self::Hour1),
            (4, BarAggregation::Hour) => Ok(Self::Hour4),
            (1, BarAggregation::Day) => Ok(Self::Day1),
            _ => anyhow::bail!("bar type {bar_type} has no QFEX candle resolution"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use strum::IntoEnumIterator;

    use super::*;

    #[rstest]
    #[case("\"BUY\"", QfexSide::Buy)]
    #[case("\"sell\"", QfexSide::Sell)]
    fn test_side_accepts_both_cases(#[case] json: &str, #[case] expected: QfexSide) {
        assert_eq!(serde_json::from_str::<QfexSide>(json).unwrap(), expected);
    }

    #[rstest]
    fn test_wire_names() {
        assert_eq!(
            serde_json::to_string(&QfexOrderType::TakeProfit).unwrap(),
            "\"TAKE_PROFIT\""
        );
        assert_eq!(
            serde_json::to_string(&QfexTimeInForce::Gtc).unwrap(),
            "\"GTC\""
        );
        assert_eq!(serde_json::to_string(&QfexSide::Buy).unwrap(), "\"BUY\"");
        assert_eq!(QfexBarInterval::Minute5.as_ref(), "5MINS");
        assert_eq!(QfexMdsChannel::MarkPrice.as_ref(), "mark_price");
        assert_eq!(QfexTradeChannel::OrderResponses.as_ref(), "order_responses");
        assert_eq!(
            serde_json::from_str::<QfexWsErrorCode>("\"InvalidJSONFormat\"").unwrap(),
            QfexWsErrorCode::InvalidJsonFormat
        );
    }

    #[rstest]
    #[case("ACK", QfexStatusKind::Accepted)]
    #[case("FILLED", QfexStatusKind::Filled)]
    #[case("IOC_PARTIALLY_FILLED", QfexStatusKind::FilledRestCanceled)]
    #[case("MODIFIED", QfexStatusKind::Modified)]
    #[case("CANCELLED_STP", QfexStatusKind::Canceled)]
    #[case("IOC_CANCELLED", QfexStatusKind::Canceled)]
    #[case("CANNOT_MODIFY_ALO_WOULD_CROSS", QfexStatusKind::ModifyRejected)]
    #[case("NO_SUCH_ORDER", QfexStatusKind::CancelRejected)]
    #[case("FAILED_MARGIN_CHECK", QfexStatusKind::Rejected)]
    #[case("INVALID_TICK_SIZE_PRECISION_PRICE", QfexStatusKind::Rejected)]
    #[case("SOMETHING_NEW", QfexStatusKind::Rejected)]
    fn test_order_status_kind(#[case] wire: &str, #[case] expected: QfexStatusKind) {
        let status: QfexOrderStatus = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
        assert_eq!(status.kind(), expected);
    }

    #[rstest]
    fn test_order_status_names_round_trip() {
        for status in QfexOrderStatus::iter().filter(|s| *s != QfexOrderStatus::Unknown) {
            let json = format!("\"{}\"", status.as_ref());
            assert_eq!(
                serde_json::from_str::<QfexOrderStatus>(&json).unwrap(),
                status
            );
        }
    }

    #[rstest]
    #[case(
        "AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL",
        Some(QfexBarInterval::Minute1)
    )]
    #[case(
        "AAPL-USD-PERP.QFEX-4-HOUR-LAST-EXTERNAL",
        Some(QfexBarInterval::Hour4)
    )]
    #[case("AAPL-USD-PERP.QFEX-1-DAY-LAST-EXTERNAL", Some(QfexBarInterval::Day1))]
    #[case("AAPL-USD-PERP.QFEX-2-MINUTE-LAST-EXTERNAL", None)]
    #[case("AAPL-USD-PERP.QFEX-1-MINUTE-BID-EXTERNAL", None)]
    #[case("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-INTERNAL", None)]
    fn test_bar_interval(#[case] bar_type: &str, #[case] expected: Option<QfexBarInterval>) {
        let bar_type = BarType::from(bar_type);
        assert_eq!(QfexBarInterval::try_from(&bar_type).ok(), expected);
    }
}
