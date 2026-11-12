//! Pulsed order book.
//!
//! QFEX does not send book deltas. The `level2` channel sends the whole visible book (20
//! levels per side, checked on 11 Nov 2026) every 500 ms, and `GET /md/orderbook` returns
//! the same shape. Each message therefore replaces the book: it becomes a snapshot (`Clear`,
//! then one `Add` per level). There is nothing to keep in sync and no gap to recover from.

use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{BookOrder, DEPTH10_LEN, OrderBookDelta, OrderBookDeltas, OrderBookDepth, QuoteTick},
    enums::{BookAction, OrderSide, RecordFlag},
    instruments::{Instrument, InstrumentAny},
};
use rust_decimal::Decimal;

use crate::common::parse::{price_from_decimal, quantity_from_decimal};

/// One venue book level: `[price, size]`.
pub type QfexLevel = (Decimal, Decimal);

/// Converts one side to orders, best price first. Empty levels (size zero) are dropped.
fn orders(
    instrument: &InstrumentAny,
    side: OrderSide,
    levels: &[QfexLevel],
) -> anyhow::Result<Vec<BookOrder>> {
    let mut orders = Vec::with_capacity(levels.len());
    for (price, size) in levels {
        let size = quantity_from_decimal(*size, instrument.size_precision())?;
        if size.is_positive() {
            let price = price_from_decimal(*price, instrument.price_precision())?;
            // Order ID 0: an L2_MBP book keys its levels by price.
            orders.push(BookOrder::new(side, price, size, 0));
        }
    }
    // The venue sends best first; sorting makes the top-of-book and depth outputs not
    // depend on it.
    match side {
        OrderSide::Buy => orders.sort_by_key(|order| std::cmp::Reverse(order.price)),
        _ => orders.sort_by_key(|order| order.price),
    }
    Ok(orders)
}

/// Builds the snapshot deltas of one full book message.
///
/// # Errors
///
/// Returns an error if a price or size does not fit the instrument.
pub fn parse_book_deltas(
    instrument: &InstrumentAny,
    bids: &[QfexLevel],
    asks: &[QfexLevel],
    sequence: u64,
    ts_event: UnixNanos,
    ts_init: UnixNanos,
) -> anyhow::Result<OrderBookDeltas> {
    let instrument_id = instrument.id();
    let flags = RecordFlag::F_SNAPSHOT as u8;
    let mut deltas = vec![OrderBookDelta::clear(
        instrument_id,
        sequence,
        ts_event,
        ts_init,
    )];
    deltas[0].flags = flags;

    let bids = orders(instrument, OrderSide::Buy, bids)?;
    let asks = orders(instrument, OrderSide::Sell, asks)?;
    for order in bids.into_iter().chain(asks) {
        deltas.push(OrderBookDelta::new_checked(
            instrument_id,
            BookAction::Add,
            order,
            flags,
            sequence,
            ts_event,
            ts_init,
        )?);
    }
    if let Some(last) = deltas.last_mut() {
        last.flags |= RecordFlag::F_LAST as u8;
    }
    OrderBookDeltas::new_checked(instrument_id, deltas)
}

/// Builds the top ten levels of one full book message. The venue sends no order counts, so
/// every level counts one order.
///
/// # Errors
///
/// Returns an error if a price or size does not fit the instrument.
pub fn parse_book_depth(
    instrument: &InstrumentAny,
    bids: &[QfexLevel],
    asks: &[QfexLevel],
    sequence: u64,
    ts_event: UnixNanos,
    ts_init: UnixNanos,
) -> anyhow::Result<OrderBookDepth> {
    let mut bids = orders(instrument, OrderSide::Buy, bids)?;
    let mut asks = orders(instrument, OrderSide::Sell, asks)?;
    bids.truncate(DEPTH10_LEN);
    asks.truncate(DEPTH10_LEN);
    let (bid_counts, ask_counts) = (vec![1_u32; bids.len()], vec![1_u32; asks.len()]);
    OrderBookDepth::new_checked(
        instrument.id(),
        bids,
        asks,
        bid_counts,
        ask_counts,
        RecordFlag::F_SNAPSHOT as u8,
        sequence,
        ts_event,
        ts_init,
    )
}

/// Builds a quote from the best level of each side. `None` when a side is empty.
///
/// # Errors
///
/// Returns an error if a price or size does not fit the instrument.
pub fn parse_quote(
    instrument: &InstrumentAny,
    bids: &[QfexLevel],
    asks: &[QfexLevel],
    ts_event: UnixNanos,
    ts_init: UnixNanos,
) -> anyhow::Result<Option<QuoteTick>> {
    let bids = orders(instrument, OrderSide::Buy, bids)?;
    let asks = orders(instrument, OrderSide::Sell, asks)?;
    let (Some(bid), Some(ask)) = (bids.first(), asks.first()) else {
        return Ok(None);
    };
    QuoteTick::new_checked(
        instrument.id(),
        bid.price,
        ask.price,
        bid.size,
        ask.size,
        ts_event,
        ts_init,
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use nautilus_model::{
        enums::BookType,
        orderbook::OrderBook,
        types::{Price, Quantity},
    };
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::http::{
        models::{QfexOrderBook, tests::fixture},
        parse::tests::instrument,
    };

    fn levels(prices: &[(Decimal, Decimal)]) -> Vec<QfexLevel> {
        prices.to_vec()
    }

    fn apply(book: &mut OrderBook, deltas: &OrderBookDeltas) {
        book.apply_deltas(deltas).unwrap();
    }

    #[rstest]
    fn test_each_message_replaces_the_book() {
        let aapl = instrument("AAPL-USD");
        let mut book = OrderBook::new(aapl.id(), BookType::L2_MBP);

        let first = parse_book_deltas(
            &aapl,
            &levels(&[(dec!(348.08), dec!(4.5)), (dec!(348.07), dec!(2))]),
            &levels(&[(dec!(348.10), dec!(1)), (dec!(348.11), dec!(3))]),
            1,
            UnixNanos::from(1),
            UnixNanos::from(2),
        )
        .unwrap();
        apply(&mut book, &first);
        assert_eq!(book.best_bid_price(), Some(Price::from("348.08")));
        assert_eq!(book.bids(None).count(), 2);

        // The next pulse has other levels: nothing of the first one may stay.
        let second = parse_book_deltas(
            &aapl,
            &levels(&[(dec!(348.00), dec!(7))]),
            &levels(&[(dec!(348.20), dec!(9))]),
            2,
            UnixNanos::from(3),
            UnixNanos::from(4),
        )
        .unwrap();
        apply(&mut book, &second);
        assert_eq!(book.best_bid_price(), Some(Price::from("348.00")));
        assert_eq!(book.best_ask_size(), Some(Quantity::from("9.000")));
        assert_eq!(book.bids(None).count(), 1);
        assert_eq!(book.asks(None).count(), 1);
    }

    #[rstest]
    fn test_flags_and_empty_levels() {
        let aapl = instrument("AAPL-USD");
        let deltas = parse_book_deltas(
            &aapl,
            &levels(&[(dec!(348), dec!(0)), (dec!(347.99), dec!(1))]),
            &[],
            7,
            UnixNanos::default(),
            UnixNanos::default(),
        )
        .unwrap();

        // Clear + one bid: the level of size zero is dropped.
        assert_eq!(deltas.deltas.len(), 2);
        assert_eq!(deltas.deltas[0].action, BookAction::Clear);
        assert_eq!(deltas.deltas[0].flags, RecordFlag::F_SNAPSHOT as u8);
        assert_eq!(
            deltas.deltas[1].flags,
            RecordFlag::F_SNAPSHOT as u8 | RecordFlag::F_LAST as u8
        );
        assert_eq!(deltas.sequence, 7);

        // An empty book is a lone `Clear`, which then carries the last flag.
        let empty = parse_book_deltas(
            &aapl,
            &[],
            &[],
            8,
            UnixNanos::default(),
            UnixNanos::default(),
        )
        .unwrap();
        assert_eq!(empty.deltas.len(), 1);
        assert_ne!(empty.deltas[0].flags & RecordFlag::F_LAST as u8, 0);
    }

    #[rstest]
    fn test_rest_snapshot_depth_and_quote() {
        let aapl = instrument("AAPL-USD");
        let snapshot: QfexOrderBook = fixture("orderbook");
        let ts = UnixNanos::from(snapshot.timestamp * 1_000_000);

        let depth = parse_book_depth(&aapl, &snapshot.bids, &snapshot.asks, 0, ts, ts).unwrap();
        assert_eq!(depth.bids.len(), DEPTH10_LEN);
        assert_eq!(depth.bids[0].price, Price::from("348.01"));
        // The second bid of the capture has size zero and is skipped.
        assert!(depth.bids[1].price < Price::from("348.00"));
        assert!(depth.asks[0].price > depth.bids[0].price);

        let quote = parse_quote(&aapl, &snapshot.bids, &snapshot.asks, ts, ts)
            .unwrap()
            .unwrap();
        assert_eq!(quote.bid_price, depth.bids[0].price);
        assert_eq!(quote.ask_price, depth.asks[0].price);
        assert_eq!(quote.bid_size, Quantity::from("10.651"));

        assert_eq!(
            parse_quote(&aapl, &snapshot.bids, &[], ts, ts).unwrap(),
            None
        );
    }

    #[rstest]
    fn test_unsorted_input_is_ordered_and_bad_values_are_errors() {
        let aapl = instrument("AAPL-USD");
        let bids = levels(&[(dec!(1), dec!(1)), (dec!(3), dec!(1)), (dec!(2), dec!(1))]);
        let asks = levels(&[(dec!(6), dec!(1)), (dec!(4), dec!(1))]);
        let quote = parse_quote(
            &aapl,
            &bids,
            &asks,
            UnixNanos::default(),
            UnixNanos::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(quote.bid_price, Price::from("3.00"));
        assert_eq!(quote.ask_price, Price::from("4.00"));

        let negative = levels(&[(dec!(1), dec!(-1))]);
        assert!(
            parse_book_deltas(
                &aapl,
                &negative,
                &[],
                0,
                UnixNanos::default(),
                UnixNanos::default()
            )
            .is_err()
        );
    }
}
