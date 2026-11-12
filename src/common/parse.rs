//! Parsing helpers shared by the HTTP and WebSocket layers.

use nautilus_core::{UnixNanos, datetime::iso8601_to_unix_nanos};
use nautilus_model::{
    identifiers::{InstrumentId, Symbol},
    types::{Price, Quantity},
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use crate::common::consts::{PERP_SUFFIX, QFEX_VENUE};

const NANOS_PER_SEC: Decimal = Decimal::from_parts(1_000_000_000, 0, 0, false, 0);

/// Converts a decimal to a [`Price`], rounding to `precision`.
///
/// # Errors
///
/// Returns an error if the value does not fit a [`Price`].
pub fn price_from_decimal(value: Decimal, precision: u8) -> anyhow::Result<Price> {
    Price::from_decimal_dp(value, precision)
        .map_err(|e| anyhow::anyhow!("invalid price {value}: {e}"))
}

/// Converts a decimal to a [`Quantity`], rounding to `precision`.
///
/// # Errors
///
/// Returns an error if the value is negative or does not fit a [`Quantity`].
pub fn quantity_from_decimal(value: Decimal, precision: u8) -> anyhow::Result<Quantity> {
    Quantity::from_decimal_dp(value, precision)
        .map_err(|e| anyhow::anyhow!("invalid quantity {value}: {e}"))
}

/// Returns the number of decimals of a tick or lot size (`0.01` is 2, `100` is 0).
///
/// # Errors
///
/// Returns an error if `step` is not positive.
pub fn precision_of(step: Decimal) -> anyhow::Result<u8> {
    anyhow::ensure!(step > Decimal::ZERO, "step {step} is not positive");
    Ok(u8::try_from(step.normalize().scale())?)
}

/// Parses an RFC 3339 timestamp (`2026-11-10T18:53:54.302717585Z`).
///
/// # Errors
///
/// Returns an error if `value` is not an RFC 3339 timestamp.
pub fn parse_rfc3339(value: &str) -> anyhow::Result<UnixNanos> {
    // A bare integer would parse as nanoseconds: the venue never sends one here.
    anyhow::ensure!(value.contains('T'), "invalid timestamp {value:?}");
    iso8601_to_unix_nanos(value)
}

/// Converts a numeric Unix time (trade socket `timestamp`, `update_time`; REST
/// `order_timestamp`). The docs show seconds with a fraction; the unit is taken from the
/// magnitude so that milliseconds, microseconds or nanoseconds would also be read correctly.
///
/// # Errors
///
/// Returns an error if `value` is negative or out of range.
pub fn unix_time_to_nanos(value: Decimal) -> anyhow::Result<UnixNanos> {
    let scale = match value.to_u64() {
        Some(v) if v >= 100_000_000_000_000_000 => Decimal::ONE,
        Some(v) if v >= 100_000_000_000_000 => Decimal::ONE_THOUSAND,
        Some(v) if v >= 100_000_000_000 => Decimal::from(1_000_000),
        _ => NANOS_PER_SEC,
    };
    value
        .checked_mul(scale)
        .and_then(|nanos| nanos.to_u64())
        .map(UnixNanos::from)
        .ok_or_else(|| anyhow::anyhow!("invalid Unix time {value}"))
}

/// Returns the instrument ID of a venue symbol: `AAPL-USD` is `AAPL-USD-PERP.QFEX`.
///
/// # Errors
///
/// Returns an error if `symbol` is not a usable symbol string.
pub fn instrument_id_of(symbol: &str) -> anyhow::Result<InstrumentId> {
    anyhow::ensure!(!symbol.is_empty(), "empty symbol");
    let symbol = Symbol::new_checked(format!("{symbol}{PERP_SUFFIX}"))
        .map_err(|e| anyhow::anyhow!("invalid symbol {symbol:?}: {e}"))?;
    Ok(InstrumentId::new(symbol, *QFEX_VENUE))
}

/// Returns the venue symbol of an instrument ID: `AAPL-USD-PERP.QFEX` is `AAPL-USD`.
///
/// # Errors
///
/// Returns an error if the ID is not a QFEX perpetual.
pub fn venue_symbol_of(instrument_id: &InstrumentId) -> anyhow::Result<&'static str> {
    instrument_id
        .symbol
        .inner()
        .as_str()
        .strip_suffix(PERP_SUFFIX)
        .filter(|_| instrument_id.venue == *QFEX_VENUE)
        .ok_or_else(|| anyhow::anyhow!("{instrument_id} is not a QFEX perpetual"))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;

    #[rstest]
    #[case(dec!(0.01), 2)]
    #[case(dec!(0.00001), 5)]
    #[case(dec!(100), 0)]
    #[case(dec!(1.000), 0)]
    fn test_precision_of(#[case] step: Decimal, #[case] expected: u8) {
        assert_eq!(precision_of(step).unwrap(), expected);
    }

    #[rstest]
    fn test_precision_of_rejects_zero() {
        assert!(precision_of(Decimal::ZERO).is_err());
    }

    #[rstest]
    fn test_parse_rfc3339() {
        assert_eq!(
            parse_rfc3339("2026-11-10T18:53:54.302717585Z").unwrap(),
            UnixNanos::from(1_794_336_834_302_717_585)
        );
        assert_eq!(
            parse_rfc3339("2026-11-10T15:04:00Z").unwrap(),
            UnixNanos::from(1_794_323_040_000_000_000)
        );
        assert!(parse_rfc3339("1794323040").is_err());
        assert!(parse_rfc3339("not a time").is_err());
    }

    #[rstest]
    #[case(dec!(1769439642.8484282), 1_769_439_642_848_428_200)]
    #[case(dec!(1769439642848), 1_769_439_642_848_000_000)]
    #[case(dec!(1769439642848428), 1_769_439_642_848_428_000)]
    #[case(dec!(1769439642848428200), 1_769_439_642_848_428_200)]
    fn test_unix_time_to_nanos(#[case] value: Decimal, #[case] expected: u64) {
        assert_eq!(
            unix_time_to_nanos(value).unwrap(),
            UnixNanos::from(expected)
        );
    }

    #[rstest]
    fn test_unix_time_to_nanos_rejects_bad_values() {
        assert!(unix_time_to_nanos(dec!(-1)).is_err());
        assert!(unix_time_to_nanos(Decimal::MAX).is_err());
    }

    #[rstest]
    fn test_symbol_round_trip() {
        let id = instrument_id_of("AAPL-USD").unwrap();
        assert_eq!(id.to_string(), "AAPL-USD-PERP.QFEX");
        assert_eq!(venue_symbol_of(&id).unwrap(), "AAPL-USD");
        assert!(venue_symbol_of(&InstrumentId::from("AAPL-USD.QFEX")).is_err());
        assert!(venue_symbol_of(&InstrumentId::from("AAPL-USD-PERP.OTHER")).is_err());
        assert!(instrument_id_of("").is_err());
    }
}
