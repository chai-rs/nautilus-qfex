//! Conversion of QFEX REST models to Nautilus types.

use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{Bar, BarType, FundingRateUpdate},
    enums::{LiquiditySide, OrderStatus, PositionSide, TriggerType},
    identifiers::{AccountId, ClientOrderId, InstrumentId, Symbol, TradeId, VenueOrderId},
    instruments::{Instrument, InstrumentAny, PerpetualContract},
    reports::{FillReport, OrderStatusReport, PositionStatusReport},
    types::{AccountBalance, Currency, Money},
};
use rust_decimal::Decimal;

use super::models::{
    QfexBalance, QfexCandle, QfexFundingRate, QfexHistoricOrder, QfexPosition, QfexSymbol,
    QfexTrade,
};
use crate::common::{
    enums::{QfexOrderStatus, QfexOrderType, QfexStatusKind, QfexTimeInForce, asset_class_of},
    parse::{
        instrument_id_of, parse_rfc3339, precision_of, price_from_decimal, quantity_from_decimal,
        unix_time_to_nanos,
    },
};

const NANOS_PER_MINUTE: u64 = 60_000_000_000;

/// Maps a venue asset code to a currency. QFEX quotes in `USD` and margins in `USDC` one to
/// one, so `USD` is read as USDC: balances and PnL then share one currency.
fn currency_of(code: &str) -> Currency {
    match code {
        "USD" | "USDC" => Currency::USDC(),
        _ => Currency::try_from_str(code).unwrap_or_else(|| Currency::get_or_create_crypto(code)),
    }
}

/// Parses a symbol of `/refdata` into a [`PerpetualContract`].
///
/// `min_price` and `max_price` of the venue are a price band that moves with the market, so
/// they are not copied to the instrument.
///
/// # Errors
///
/// Returns an error if a step, a quantity limit or the leverage is not usable.
pub fn parse_instrument(symbol: &QfexSymbol, ts_init: UnixNanos) -> anyhow::Result<InstrumentAny> {
    anyhow::ensure!(
        symbol.default_max_leverage > Decimal::ZERO,
        "invalid leverage for {}",
        symbol.symbol
    );
    let price_precision = precision_of(symbol.tick_size)?;
    let size_precision = precision_of(symbol.lot_size)?;
    let size_increment = quantity_from_decimal(symbol.lot_size, size_precision)?;

    let instrument = PerpetualContract::builder()
        .instrument_id(instrument_id_of(&symbol.symbol)?)
        .raw_symbol(Symbol::new_checked(symbol.symbol).map_err(|e| anyhow::anyhow!("{e}"))?)
        .underlying(symbol.base_asset)
        .asset_class(asset_class_of(&symbol.product_category))
        .quote_currency(currency_of(&symbol.quote_asset))
        .settlement_currency(currency_of(&symbol.margin_asset))
        .is_inverse(false)
        .price_precision(price_precision)
        .size_precision(size_precision)
        .price_increment(price_from_decimal(symbol.tick_size, price_precision)?)
        .size_increment(size_increment)
        .lot_size(size_increment)
        .min_quantity(quantity_from_decimal(symbol.min_quantity, size_precision)?)
        .max_quantity(quantity_from_decimal(symbol.max_quantity, size_precision)?)
        .margin_init(Decimal::ONE / symbol.default_max_leverage)
        .ts_event(ts_init)
        .ts_init(ts_init)
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(InstrumentAny::PerpetualContract(instrument))
}

/// Parses a candle. The event time is the open time of the candle, the only time the venue
/// sends.
///
/// The base volume was zero on every candle captured on 11 Nov 2026, so the volume falls
/// back to `usdVolume / close`.
///
/// # Errors
///
/// Returns an error if a value does not fit the instrument or the time is not RFC 3339.
pub fn parse_bar(
    candle: &QfexCandle,
    bar_type: BarType,
    instrument: &InstrumentAny,
    ts_init: UnixNanos,
) -> anyhow::Result<Bar> {
    let price = |value| price_from_decimal(value, instrument.price_precision());
    // A bar with the open or close outside its high-low range is rejected by Nautilus.
    let high = candle.high.max(candle.open).max(candle.close);
    let low = candle.low.min(candle.open).min(candle.close);
    let volume = if candle.base_token_volume.is_zero() && candle.close > Decimal::ZERO {
        candle
            .usd_volume
            .checked_div(candle.close)
            .unwrap_or_default()
    } else {
        candle.base_token_volume
    };
    Bar::new_checked(
        bar_type,
        price(candle.open)?,
        price(high)?,
        price(low)?,
        price(candle.close)?,
        quantity_from_decimal(volume, instrument.size_precision())?,
        parse_rfc3339(&candle.started_at)?,
        ts_init,
    )
}

/// Parses a historic funding rate. The event time is the end of the funding window.
///
/// # Errors
///
/// Returns an error if the window start is not RFC 3339.
pub fn parse_funding_rate(
    record: &QfexFundingRate,
    instrument_id: InstrumentId,
    ts_init: UnixNanos,
) -> anyhow::Result<FundingRateUpdate> {
    let window = u64::from(record.interval_minutes) * NANOS_PER_MINUTE;
    Ok(FundingRateUpdate::new(
        instrument_id,
        record.rate,
        Some(record.interval_minutes).filter(|minutes| *minutes > 0),
        None,
        UnixNanos::from(
            parse_rfc3339(&record.window_start)?
                .as_u64()
                .saturating_add(window),
        ),
        ts_init,
    ))
}

/// Parses the account balance. The venue reports the free amount and the two margins; the
/// total is their sum (assumed: the docs give no formula).
///
/// # Errors
///
/// Returns an error if an amount does not fit the currency.
pub fn parse_balance(balance: &QfexBalance) -> anyhow::Result<AccountBalance> {
    let total = balance
        .available_balance
        .checked_add(balance.order_margin)
        .and_then(|sum| sum.checked_add(balance.position_margin))
        .ok_or_else(|| anyhow::anyhow!("balance is out of range"))?;
    AccountBalance::from_total_and_free(total, balance.available_balance, Currency::USDC())
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Parses a position. The size is signed: negative is short.
///
/// # Errors
///
/// Returns an error if the size does not fit the instrument.
pub fn parse_position_status_report(
    position: &QfexPosition,
    account_id: AccountId,
    instrument: &InstrumentAny,
    ts_init: UnixNanos,
) -> anyhow::Result<PositionStatusReport> {
    let quantity = quantity_from_decimal(position.position.abs(), instrument.size_precision())?;
    let position_side = if quantity.is_zero() {
        PositionSide::Flat
    } else if position.position.is_sign_negative() {
        PositionSide::Short
    } else {
        PositionSide::Long
    };

    Ok(PositionStatusReport::new(
        account_id,
        instrument.id(),
        position_side,
        quantity,
        ts_init,
        ts_init,
        None,
        None,
        (position.average_price > Decimal::ZERO).then_some(position.average_price),
    ))
}

/// Maker or taker, from the order type: the REST trade has no liquidity field.
pub(crate) const fn liquidity_side_of(order_type: Option<QfexOrderType>) -> LiquiditySide {
    match order_type {
        Some(QfexOrderType::Market | QfexOrderType::StopMarket) => LiquiditySide::Taker,
        Some(QfexOrderType::Alo) => LiquiditySide::Maker,
        _ => LiquiditySide::NoLiquiditySide,
    }
}

/// Parses a fill of `GET /user/trade`. The fee is kept signed (a negative fee is a rebate)
/// and is in the margin asset.
///
/// # Errors
///
/// Returns an error if a value does not fit the instrument or the trade id is not usable.
pub fn parse_fill_report(
    trade: &QfexTrade,
    account_id: AccountId,
    instrument: &InstrumentAny,
    ts_init: UnixNanos,
) -> anyhow::Result<FillReport> {
    Ok(FillReport::new(
        account_id,
        instrument.id(),
        VenueOrderId::new_checked(&trade.order_id).map_err(|e| anyhow::anyhow!("{e}"))?,
        TradeId::new_checked(&trade.id).map_err(|e| anyhow::anyhow!("{e}"))?,
        trade.side.as_order_side(),
        quantity_from_decimal(trade.quantity, instrument.size_precision())?,
        price_from_decimal(trade.price, instrument.price_precision())?,
        Money::from_decimal(trade.fee, instrument.settlement_currency())
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        liquidity_side_of(trade.order_type),
        None,
        None,
        unix_time_to_nanos(trade.order_timestamp)?,
        ts_init,
        None,
    ))
}

/// Parses an order of `GET /user/historic-orders`: an order in a final state.
///
/// The values of `terminal_status` are not listed in the docs, so the status is taken from
/// the quantities first: fully filled is `Filled`, a known rejection with nothing filled is
/// `Rejected`, everything else is `Canceled`.
///
/// # Errors
///
/// Returns an error if a value does not fit the instrument.
pub fn parse_historic_order_status_report(
    order: &QfexHistoricOrder,
    account_id: AccountId,
    instrument: &InstrumentAny,
    ts_init: UnixNanos,
) -> anyhow::Result<OrderStatusReport> {
    let price_precision = instrument.price_precision();
    let quantity = quantity_from_decimal(order.quantity, instrument.size_precision())?;
    let filled_qty = quantity_from_decimal(order.filled_qty, instrument.size_precision())?;
    let rejected = order.terminal_status != QfexOrderStatus::Unknown
        && order.terminal_status.kind() == QfexStatusKind::Rejected;
    let order_status = if quantity.is_positive() && filled_qty >= quantity {
        OrderStatus::Filled
    } else if rejected && filled_qty.is_zero() {
        OrderStatus::Rejected
    } else {
        OrderStatus::Canceled
    };
    let (order_type, post_only) = order.order_type.as_order_type();
    let ts_last = parse_rfc3339(&order.status_ts)?;

    let mut report = OrderStatusReport::new(
        account_id,
        instrument.id(),
        ClientOrderId::new_checked(&order.client_order_id).ok(),
        VenueOrderId::new_checked(&order.order_id).map_err(|e| anyhow::anyhow!("{e}"))?,
        Some(order.side.as_order_side()),
        order_type,
        order
            .time_in_force
            .unwrap_or(QfexTimeInForce::Gtc)
            .as_time_in_force(),
        order_status,
        quantity,
        filled_qty,
        ts_last,
        ts_last,
        ts_init,
        None,
    )
    .with_post_only(post_only);

    if order.price > Decimal::ZERO {
        let price = price_from_decimal(order.price, price_precision)?;
        // The `price` of a stop order is its trigger price.
        report = if order.order_type.is_stop() {
            report
                .with_trigger_price(price)
                .with_trigger_type(TriggerType::MarkPrice)
        } else {
            report.with_price(price)
        };
    }
    if let Some(avg_price) = order.avg_price.filter(|price| *price > Decimal::ZERO) {
        report = report.with_avg_px(avg_price);
    }
    Ok(report)
}

#[cfg(test)]
pub(crate) mod tests {
    use nautilus_model::{
        enums::{AssetClass, OrderSide, OrderType, TimeInForce},
        types::{Price, Quantity},
    };
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::http::models::{
        QfexCandles, QfexFundingRates, QfexHistoricOrders, QfexRefData, QfexTrades,
        QfexUserPositions, tests::fixture,
    };

    fn symbols() -> Vec<QfexSymbol> {
        fixture::<QfexRefData>("refdata")
            .data
            .into_iter()
            .map(|item| serde_json::from_value(item).unwrap())
            .collect()
    }

    pub(crate) fn instrument(symbol: &str) -> InstrumentAny {
        let symbols = symbols();
        let symbol = symbols.iter().find(|s| s.symbol == symbol).unwrap();
        parse_instrument(symbol, UnixNanos::default()).unwrap()
    }

    fn account_id() -> AccountId {
        AccountId::new("QFEX-001")
    }

    #[rstest]
    fn test_parse_instruments() {
        // Every symbol of the capture parses, whatever its status or quote currency.
        for symbol in symbols() {
            let instrument = parse_instrument(&symbol, UnixNanos::default()).unwrap();
            assert_eq!(instrument.settlement_currency(), Currency::USDC());
            assert!(!instrument.is_inverse());
        }

        let aapl = instrument("AAPL-USD");
        assert_eq!(aapl.id(), InstrumentId::from("AAPL-USD-PERP.QFEX"));
        assert_eq!(aapl.raw_symbol().as_str(), "AAPL-USD");
        assert_eq!(aapl.asset_class(), AssetClass::Equity);
        assert_eq!(aapl.price_increment(), Price::from("0.01"));
        assert_eq!(aapl.size_increment(), Quantity::from("0.001"));
        assert_eq!(aapl.min_quantity(), Some(Quantity::from("0.010")));
        assert_eq!(aapl.max_quantity(), Some(Quantity::from("10000000.000")));
        assert_eq!(aapl.quote_currency(), Currency::USDC());
        assert_eq!(aapl.margin_init(), dec!(0.05));
        assert_eq!(aapl.max_price(), None);
    }

    #[rstest]
    fn test_non_usd_quote_keeps_its_currency() {
        let symbols = symbols();
        let krw = symbols.iter().find(|s| s.quote_asset == "KRW").unwrap();
        let instrument = parse_instrument(krw, UnixNanos::default()).unwrap();
        assert_eq!(instrument.quote_currency().code.as_str(), "KRW");
        assert_eq!(instrument.settlement_currency(), Currency::USDC());
    }

    #[rstest]
    fn test_parse_instrument_rejects_unusable_values() {
        let mut symbol = symbols().remove(0);
        symbol.tick_size = Decimal::ZERO;
        assert!(parse_instrument(&symbol, UnixNanos::default()).is_err());

        let mut symbol = symbols().remove(0);
        symbol.default_max_leverage = Decimal::ZERO;
        assert!(parse_instrument(&symbol, UnixNanos::default()).is_err());
    }

    #[rstest]
    fn test_parse_bar() {
        let aapl = instrument("AAPL-USD");
        let bar_type = BarType::from("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL");
        let candles: QfexCandles = fixture("candles");

        let bar = parse_bar(&candles.candles[0], bar_type, &aapl, UnixNanos::from(9)).unwrap();

        assert_eq!(bar.open, Price::from("348.35"));
        assert_eq!(bar.high, Price::from("348.52"));
        assert_eq!(bar.low, Price::from("348.34"));
        assert_eq!(bar.close, Price::from("348.49"));
        // 29046.90685 / 348.49, rounded to the lot size.
        assert_eq!(bar.volume, Quantity::from("83.351"));
        assert_eq!(bar.ts_event, UnixNanos::from(1_794_323_040_000_000_000));
        assert_eq!(bar.ts_init, UnixNanos::from(9));
    }

    #[rstest]
    fn test_parse_bar_widens_the_range() {
        let aapl = instrument("AAPL-USD");
        let bar_type = BarType::from("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL");
        let mut candle = fixture::<QfexCandles>("candles").candles.remove(0);
        candle.open = dec!(400);
        candle.close = dec!(300);
        candle.base_token_volume = dec!(2);

        let bar = parse_bar(&candle, bar_type, &aapl, UnixNanos::default()).unwrap();

        assert_eq!(bar.high, Price::from("400.00"));
        assert_eq!(bar.low, Price::from("300.00"));
        assert_eq!(bar.volume, Quantity::from("2.000"));
    }

    #[rstest]
    fn test_parse_funding_rate() {
        let aapl = instrument("AAPL-USD");
        let funding: QfexFundingRates = fixture("funding");

        let update = parse_funding_rate(&funding.data[0], aapl.id(), UnixNanos::from(9)).unwrap();

        assert_eq!(update.rate, dec!(-0.00000186));
        assert_eq!(update.interval, Some(60));
        // 2026-11-10T00:00:00Z plus the one-hour window.
        assert_eq!(update.ts_event, UnixNanos::from(1_794_272_400_000_000_000));
    }

    #[rstest]
    fn test_parse_balance() {
        let account: QfexUserPositions = fixture("user_positions");
        let balance = parse_balance(&account.balance).unwrap();
        assert_eq!(balance.free.as_decimal(), dec!(9500.25));
        assert_eq!(balance.locked.as_decimal(), dec!(200.6));
        assert_eq!(balance.total.as_decimal(), dec!(9700.85));
    }

    #[rstest]
    fn test_parse_position_status_report() {
        let aapl = instrument("AAPL-USD");
        let account: QfexUserPositions = fixture("user_positions");

        let short =
            parse_position_status_report(&account.positions[0], account_id(), &aapl, 1.into())
                .unwrap();
        assert_eq!(short.position_side, PositionSide::Short);
        assert_eq!(short.quantity, Quantity::from("1.500"));
        assert_eq!(short.avg_px_open, Some(dec!(348.1)));

        let flat =
            parse_position_status_report(&account.positions[1], account_id(), &aapl, 1.into())
                .unwrap();
        assert_eq!(flat.position_side, PositionSide::Flat);
        assert_eq!(flat.avg_px_open, None);
    }

    #[rstest]
    fn test_parse_fill_report() {
        let aapl = instrument("AAPL-USD");
        let trades: QfexTrades = fixture("user_trades");

        let maker = parse_fill_report(&trades.data[0], account_id(), &aapl, 1.into()).unwrap();
        assert_eq!(maker.order_side, OrderSide::Buy);
        assert_eq!(maker.last_qty, Quantity::from("0.500"));
        assert_eq!(maker.last_px, Price::from("348.05"));
        assert_eq!(maker.commission, Money::new(0.0348, Currency::USDC()));
        assert_eq!(maker.liquidity_side, LiquiditySide::NoLiquiditySide);
        assert_eq!(maker.ts_event, UnixNanos::from(1_769_439_642_848_428_200));
        assert_eq!(
            maker.trade_id.as_str(),
            "7f1c2a34-5b6d-4e8f-9a0b-1c2d3e4f5a6b"
        );

        let taker = parse_fill_report(&trades.data[1], account_id(), &aapl, 1.into()).unwrap();
        assert_eq!(taker.order_side, OrderSide::Sell);
        assert_eq!(taker.liquidity_side, LiquiditySide::Taker);
    }

    #[rstest]
    fn test_parse_historic_order_status_report() {
        let aapl = instrument("AAPL-USD");
        let orders: QfexHistoricOrders = fixture("historic_orders");

        let filled =
            parse_historic_order_status_report(&orders.data[0], account_id(), &aapl, 1.into())
                .unwrap();
        assert_eq!(filled.order_status, OrderStatus::Filled);
        assert_eq!(filled.order_type, OrderType::Limit);
        assert_eq!(filled.time_in_force, TimeInForce::Gtc);
        assert_eq!(filled.price, Some(Price::from("348.05")));
        assert_eq!(filled.avg_px, Some(dec!(348.05)));
        assert_eq!(
            filled.client_order_id,
            Some(ClientOrderId::new("O-20260126-150042-001-000-1"))
        );

        // A cancelled stop order: its price is the trigger price and it has no client id.
        let stop =
            parse_historic_order_status_report(&orders.data[1], account_id(), &aapl, 1.into())
                .unwrap();
        assert_eq!(stop.order_status, OrderStatus::Canceled);
        assert_eq!(stop.order_type, OrderType::StopMarket);
        assert_eq!(stop.trigger_price, Some(Price::from("340.00")));
        assert_eq!(stop.trigger_type, Some(TriggerType::MarkPrice));
        assert_eq!(stop.price, None);
        assert_eq!(stop.client_order_id, None);
    }

    #[rstest]
    #[case(QfexOrderStatus::FailedMarginCheck, dec!(0), OrderStatus::Rejected)]
    #[case(QfexOrderStatus::Unknown, dec!(0), OrderStatus::Canceled)]
    #[case(QfexOrderStatus::IocPartiallyFilled, dec!(0.2), OrderStatus::Canceled)]
    #[case(QfexOrderStatus::Unknown, dec!(0.5), OrderStatus::Filled)]
    fn test_historic_order_status(
        #[case] terminal_status: QfexOrderStatus,
        #[case] filled_qty: Decimal,
        #[case] expected: OrderStatus,
    ) {
        let aapl = instrument("AAPL-USD");
        let mut order = fixture::<QfexHistoricOrders>("historic_orders")
            .data
            .remove(0);
        order.terminal_status = terminal_status;
        order.filled_qty = filled_qty;

        let report =
            parse_historic_order_status_report(&order, account_id(), &aapl, 1.into()).unwrap();

        assert_eq!(report.order_status, expected);
    }
}
