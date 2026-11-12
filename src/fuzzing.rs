//! The bodies of the fuzz targets in `fuzz/`.
//!
//! They live here so that the unit tests below can run them with a fixed seed: `cargo-fuzz`
//! needs a nightly toolchain, the tests do not. Every function must return for every input;
//! a panic is a finding.

use std::sync::LazyLock;

use nautilus_core::{UUID4, UnixNanos, time::get_atomic_clock_realtime};
use nautilus_live::ExecutionEventEmitter;
use nautilus_model::{
    data::BarType,
    enums::{AccountType, BookType, OrderSide, TimeInForce},
    identifiers::{AccountId, ClientOrderId, StrategyId, TraderId},
    instruments::{Instrument, InstrumentAny},
    orderbook::OrderBook,
    orders::{LimitOrder, OrderAny},
    types::{Price, Quantity},
};
use rust_decimal::Decimal;
use serde_json::Value;
use ustr::Ustr;

use crate::{
    book::{QfexLevel, parse_book_deltas, parse_book_depth, parse_quote},
    common::{
        credential::QfexCredential,
        enums::{QfexOrderType, QfexSide, QfexTimeInForce},
    },
    execution::dispatch::OrderDispatch,
    http::{
        models::{
            QfexBalance, QfexCandles, QfexFees, QfexFundingRates, QfexHistoricOrders,
            QfexOrderBook, QfexProblem, QfexRefData, QfexSymbol, QfexTrades, QfexUserPositions,
        },
        parse::{
            parse_balance, parse_bar, parse_fill_report, parse_funding_rate,
            parse_historic_order_status_report, parse_instrument, parse_position_status_report,
        },
    },
    signing::{HEADER_NONCE, HEADER_SIGNATURE, QfexAuth},
    websocket::{
        handler::parse_mds_message,
        messages::{QfexAddOrder, QfexModifyOrder, QfexTradeRequest, QfexWsMessage},
    },
};

/// The client order id of the one order the trade target tracks.
pub const TRACKED_ORDER: &str = "maker-sell-001";

/// The symbols the targets know, with the tick and lot size of `AAPL-USD` on 10 Nov 2026.
const SYMBOLS: [&str; 2] = ["AAPL-USD", "US100-USD"];

static INSTRUMENTS: LazyLock<[InstrumentAny; 2]> = LazyLock::new(|| {
    SYMBOLS.map(|symbol| {
        let row = serde_json::json!({
            "symbol": symbol, "tick_size": "0.01", "lot_size": "0.001", "min_quantity": "0.01",
            "max_quantity": "10000000", "base_asset": "AAPL", "quote_asset": "USD",
            "margin_asset": "USDC", "default_max_leverage": 20, "status": "ACTIVE",
        });
        let symbol: QfexSymbol = serde_json::from_value(row).expect("valid symbol row");
        parse_instrument(&symbol, UnixNanos::default()).expect("valid instrument")
    })
});

fn instrument_of(symbol: &Ustr) -> Option<InstrumentAny> {
    let at = SYMBOLS.iter().position(|known| *known == symbol.as_str())?;
    Some(INSTRUMENTS[at].clone())
}

/// A REST body, read as every response model and converted to Nautilus types.
pub fn rest_response(text: &str) {
    let instrument = &INSTRUMENTS[0];
    let account_id = AccountId::from("QFEX-001");
    let ts = UnixNanos::default();

    if let Ok(refdata) = serde_json::from_str::<QfexRefData>(text) {
        for row in refdata.data {
            if let Ok(symbol) = serde_json::from_value::<QfexSymbol>(row) {
                let _ = parse_instrument(&symbol, ts);
            }
        }
    }

    if let Ok(book) = serde_json::from_str::<QfexOrderBook>(text) {
        let ts_event = UnixNanos::from(book.timestamp.saturating_mul(1_000_000));
        let _ = parse_book_deltas(instrument, &book.bids, &book.asks, 0, ts_event, ts);
    }

    if let Ok(candles) = serde_json::from_str::<QfexCandles>(text) {
        let bar_type = BarType::from("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL");
        for candle in &candles.candles {
            let _ = parse_bar(candle, bar_type, instrument, ts);
        }
    }

    if let Ok(rates) = serde_json::from_str::<QfexFundingRates>(text) {
        for rate in &rates.data {
            let _ = parse_funding_rate(rate, instrument.id(), ts);
        }
    }

    if let Ok(user) = serde_json::from_str::<QfexUserPositions>(text) {
        let _ = parse_balance(&user.balance);
        for position in &user.positions {
            let _ = parse_position_status_report(position, account_id, instrument, ts);
        }
    }

    if let Ok(trades) = serde_json::from_str::<QfexTrades>(text) {
        for trade in &trades.data {
            let _ = parse_fill_report(trade, account_id, instrument, ts);
        }
    }

    if let Ok(orders) = serde_json::from_str::<QfexHistoricOrders>(text) {
        for order in &orders.data {
            let _ = parse_historic_order_status_report(order, account_id, instrument, ts);
        }
    }

    let _ = serde_json::from_str::<QfexFees>(text);
    let _ = serde_json::from_str::<QfexProblem>(text);
}

/// A frame of the market data socket.
pub fn mds_message(text: &str) {
    if let Ok(message) = QfexWsMessage::parse(text) {
        let _ = parse_mds_message(&message, instrument_of, UnixNanos::default());
    }
}

/// A frame of the trade socket, handled twice (the venue repeats frames after a reconnect)
/// by a session that tracks one open order, [`TRACKED_ORDER`].
pub fn trade_message(text: &str) {
    let Ok(message) = QfexWsMessage::parse(text) else {
        return;
    };
    let account_id = AccountId::from("QFEX-001");
    let trader_id = TraderId::from("FUZZ-001");
    let clock = get_atomic_clock_realtime();
    let mut emitter =
        ExecutionEventEmitter::new(clock, trader_id, account_id, AccountType::Margin, None);
    // The receiver stays alive so that every event has somewhere to go.
    let (sender, _events) = tokio::sync::mpsc::unbounded_channel();
    emitter.set_sender(sender);
    let instruments = SYMBOLS
        .map(Ustr::from)
        .into_iter()
        .zip(INSTRUMENTS.iter().cloned());
    let dispatch = OrderDispatch::new(account_id, emitter, instruments.collect());
    dispatch.track(&OrderAny::Limit(LimitOrder::new(
        trader_id,
        StrategyId::from("S-001"),
        INSTRUMENTS[0].id(),
        ClientOrderId::from(TRACKED_ORDER),
        OrderSide::Sell,
        Quantity::from("1.000"),
        Price::from("200.25"),
        TimeInForce::Gtc,
        None,
        false,
        false,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        UUID4::new(),
        UnixNanos::default(),
    )));

    let ts = UnixNanos::default();
    for _ in 0..2 {
        match &message {
            QfexWsMessage::Order(row) => {
                let _ = dispatch.order_report(row, ts);
                dispatch.handle_order(row, ts);
            }
            QfexWsMessage::Fill(fill) => dispatch.handle_fill(fill, ts),
            QfexWsMessage::Orders(orders) => {
                for row in orders.orders.iter().chain(&orders.stop_orders) {
                    let _ = dispatch.order_report(row, ts);
                }
            }
            QfexWsMessage::Balance(balance) => {
                let _ = parse_balance(&QfexBalance {
                    available_balance: balance.available_balance,
                    order_margin: balance.order_margin,
                    position_margin: balance.position_margin,
                });
            }
            QfexWsMessage::Error(error) => {
                dispatch.handle_error(error, ts);
            }
            _ => {}
        }
    }
}

/// One pulse of the book, applied twice: every pulse is a whole book, so the second one must
/// leave the book as the first one did.
///
/// # Panics
///
/// Panics if the second pulse changes the book.
pub fn book_apply(bids: &[QfexLevel], asks: &[QfexLevel], sequence: u64) {
    let instrument = &INSTRUMENTS[0];
    let ts = UnixNanos::default();
    let _ = parse_book_depth(instrument, bids, asks, sequence, ts, ts);
    let _ = parse_quote(instrument, bids, asks, ts, ts);
    let Ok(deltas) = parse_book_deltas(instrument, bids, asks, sequence, ts, ts) else {
        return;
    };
    let mut book = OrderBook::new(instrument.id(), BookType::L2_MBP);
    if book.apply_deltas(&deltas).is_err() {
        return;
    }
    let first = (book.best_bid_price(), book.best_ask_price());
    if book.apply_deltas(&deltas).is_ok() {
        assert_eq!((book.best_bid_price(), book.best_ask_price()), first);
    }
}

/// The REST headers and the WebSocket `auth` frame of one signature.
///
/// # Panics
///
/// Panics if the signature is not 64 lower-case hex digits, or the frame does not carry the
/// nonce, the timestamp and the public key unchanged.
pub fn auth(key: &str, secret: &str, nonce: &str, unix_ts: u64) {
    let credential = QfexCredential::new(key, secret);
    let Ok(auth) = QfexAuth::sign(&credential, nonce, unix_ts) else {
        return;
    };
    let headers = auth.headers();
    assert_eq!(headers.len(), 4);
    assert_eq!(headers[HEADER_NONCE], nonce);
    let signature = &headers[HEADER_SIGNATURE];
    let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
    assert!(signature.len() == 64 && signature.chars().all(hex));

    let frame: Value = serde_json::from_str(&auth.ws_message()).expect("auth frame is JSON");
    let hmac = &frame["params"]["hmac"];
    assert_eq!(hmac["public_key"], key);
    assert_eq!(hmac["nonce"], nonce);
    assert_eq!(hmac["unix_ts"], unix_ts);
    assert_eq!(hmac["signature"], signature.as_str());
}

/// The numbers of an order as they go on the wire.
///
/// # Panics
///
/// Panics if a frame is not JSON, or a value of up to 15 significant digits and 22 decimals
/// (every real price and size) does not read back as the same number.
pub fn wire_format(quantity: Decimal, price: Decimal) {
    let symbol = Ustr::from("AAPL-USD");
    let add = QfexTradeRequest::AddOrder(QfexAddOrder {
        symbol,
        side: QfexSide::Buy,
        order_type: QfexOrderType::Limit,
        order_time_in_force: QfexTimeInForce::Gtc,
        quantity,
        price: Some(price),
        client_order_id: "O-1".to_string(),
        reduce_only: false,
    });
    let modify = QfexTradeRequest::ModifyOrder(QfexModifyOrder {
        order_id: "5b309929-206f-40ec-804d-cbe46e81afc1".to_string(),
        symbol,
        side: QfexSide::Buy,
        order_type: QfexOrderType::Limit,
        reduce_only: false,
        quantity,
        price: Some(price),
    });
    for request in [add, modify] {
        let text = request.to_frame();
        let frame: Value = serde_json::from_str(&text).expect("request frame is JSON");
        for (name, value) in [("quantity", quantity), ("price", price)] {
            // The wire number is an f64. `Decimal::to_f64` divides by 10^scale, and an f64
            // holds a power of ten exactly up to 10^22 only.
            if value.mantissa().unsigned_abs() >= 1_000_000_000_000_000 || value.scale() > 22 {
                continue;
            }
            let sent = frame["params"][name].to_string();
            let read = Decimal::from_scientific(&sent).or_else(|_| sent.parse());
            assert_eq!(read.ok(), Some(value), "{name} {value} was sent as {sent}");
        }
    }
}

/// A decimal from raw parts, for the targets whose input is numbers. The scale wraps at 29:
/// `Decimal::from_parts` panics above 28.
#[must_use]
pub fn decimal((lo, mid, hi, negative, scale): (u32, u32, u32, bool, u8)) -> Decimal {
    Decimal::from_parts(lo, mid, hi, negative, u32::from(scale) % 29)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const ROUNDS: usize = 400;

    /// A linear congruential generator (the constants of Knuth's MMIX): no dependency, and
    /// the same inputs on every run.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 16
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }

        fn pick<'a, T: ?Sized>(&mut self, items: &[&'a T]) -> &'a T {
            items[self.below(items.len())]
        }

        fn decimal(&mut self) -> Decimal {
            // Half of them small (a real price or size), half of them anything.
            let small = self.next().is_multiple_of(2);
            let word = |rng: &mut Self| rng.next() as u32;
            let (lo, mid, hi) = (word(self), word(self), word(self));
            let (negative, scale) = (self.next().is_multiple_of(4), self.next() as u8);
            if small {
                decimal((lo, mid & 0xFFFF, 0, negative, scale % 10))
            } else {
                decimal((lo, mid, hi, negative, scale))
            }
        }

        fn levels(&mut self) -> Vec<QfexLevel> {
            (0..self.below(25))
                .map(|_| (self.decimal(), self.decimal()))
                .collect()
        }

        fn text(&mut self) -> String {
            let alphabet: Vec<char> = "aZ09 :-_\"\\{}\n\u{e9}\u{1f600}".chars().collect();
            (0..self.below(130))
                .map(|_| alphabet[self.below(alphabet.len())])
                .collect()
        }
    }

    /// Values a parser has to survive in place of any field.
    const NASTY: &[&str] = &[
        "null",
        "true",
        "\"\"",
        "\"x\"",
        "[]",
        "{}",
        "0",
        "-1",
        "0.0000000000000000000000000001",
        "79228162514264337593543950335",
        "-79228162514264337593543950335",
        "\"79228162514264337593543950335\"",
        "1e400",
        "18446744073709551615",
        "\"9999-12-31T23:59:59.999999999Z\"",
        "\"1970-01-01T00:00:00Z\"",
        "\"maker-sell-001\"",
        "\"AAPL-USD\"",
        "\"FILLED\"",
        "\"MODIFIED\"",
    ];

    fn leaves<'a>(value: &'a mut Value, out: &mut Vec<&'a mut Value>) {
        match value {
            Value::Object(map) => map.values_mut().for_each(|item| leaves(item, out)),
            Value::Array(items) => items.iter_mut().for_each(|item| leaves(item, out)),
            leaf => out.push(leaf),
        }
    }

    /// A recorded frame with up to three fields replaced, sometimes cut short, sometimes with
    /// one byte overwritten.
    fn mutated(rng: &mut Lcg, fixtures: &[&str]) -> String {
        let mut value: Value = serde_json::from_str(rng.pick(fixtures)).unwrap();
        for _ in 0..=rng.below(3) {
            let mut fields = vec![];
            leaves(&mut value, &mut fields);
            if !fields.is_empty() {
                let field = rng.below(fields.len());
                *fields[field] = serde_json::from_str(rng.pick(NASTY)).unwrap_or(Value::Null);
            }
        }
        let mut bytes = value.to_string().into_bytes();
        match rng.below(8) {
            0 => bytes.truncate(rng.below(bytes.len())),
            1 => {
                let at = rng.below(bytes.len());
                bytes[at] = b" \"{}[]:,-e9"[rng.below(11)];
            }
            _ => {}
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// A recorded frame with every number replaced by the same value: the names, symbols and
    /// states stay, so the frame still reaches the arithmetic.
    fn flooded(fixture: &str, nasty: &str) -> String {
        let mut value: Value = serde_json::from_str(fixture).unwrap();
        let mut fields = vec![];
        leaves(&mut value, &mut fields);
        for field in fields {
            let numeric = |text: &str| text.parse::<f64>().is_ok();
            if field.is_number() || field.as_str().is_some_and(numeric) {
                *field = serde_json::from_str(nasty).unwrap_or(Value::Null);
            }
        }
        value.to_string()
    }

    /// Runs `target` on the flooded and on the randomly mutated fixtures.
    fn run(target: fn(&str), fixtures: &[&str], seed: u64) {
        for fixture in fixtures {
            for nasty in NASTY {
                target(&flooded(fixture, nasty));
            }
        }
        let mut rng = Lcg(seed);
        for _ in 0..ROUNDS {
            target(&mutated(&mut rng, fixtures));
            target(&rng.text());
        }
    }

    fn lines(text: &'static str) -> Vec<&'static str> {
        text.lines().collect()
    }

    #[rstest]
    fn test_rest_response_survives_mutated_bodies() {
        let fixtures = [
            include_str!("../tests/fixtures/http/candles.json"),
            include_str!("../tests/fixtures/http/funding.json"),
            include_str!("../tests/fixtures/http/historic_orders.json"),
            include_str!("../tests/fixtures/http/orderbook.json"),
            include_str!("../tests/fixtures/http/problem.json"),
            include_str!("../tests/fixtures/http/refdata.json"),
            include_str!("../tests/fixtures/http/user_fees.json"),
            include_str!("../tests/fixtures/http/user_positions.json"),
            include_str!("../tests/fixtures/http/user_trades.json"),
        ];
        run(rest_response, &fixtures, 1);
    }

    #[rstest]
    fn test_mds_message_survives_mutated_frames() {
        let fixtures = lines(include_str!("../tests/fixtures/ws/mds.jsonl"));
        run(mds_message, &fixtures, 2);
    }

    #[rstest]
    fn test_trade_message_survives_mutated_frames() {
        let fixtures = lines(include_str!("../tests/fixtures/ws/trade.jsonl"));
        run(trade_message, &fixtures, 3);
    }

    #[rstest]
    fn test_book_apply_survives_any_levels() {
        let mut rng = Lcg(4);
        for _ in 0..ROUNDS {
            let (bids, asks) = (rng.levels(), rng.levels());
            book_apply(&bids, &asks, rng.next());
        }
    }

    #[rstest]
    fn test_auth_survives_any_strings() {
        let mut rng = Lcg(5);
        for _ in 0..ROUNDS {
            let (key, secret, nonce) = (rng.text(), rng.text(), rng.text());
            auth(&key, &secret, &nonce, rng.next());
        }
        auth("", "", "n", u64::MAX);
    }

    #[rstest]
    fn test_wire_format_survives_any_decimals() {
        let mut rng = Lcg(6);
        for _ in 0..ROUNDS {
            wire_format(rng.decimal(), rng.decimal());
        }
        wire_format(Decimal::MAX, Decimal::MIN);
    }

    /// The input of the first crash that the CI fuzz job found (12 Nov 2026).
    #[rstest]
    fn test_wire_format_survives_25_decimals() {
        wire_format(
            decimal((2_307_590_656, 58_753, 0, true, 112)),
            decimal((0, 28_160, 0, true, 45)),
        );
    }
}
