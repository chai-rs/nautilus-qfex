//! Turns market data frames into Nautilus data.

use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{Data, FundingRateUpdate, IndexPriceUpdate, MarkPriceUpdate, TradeTick},
    enums::AggressorSide,
    identifiers::TradeId,
    instruments::{Instrument, InstrumentAny},
};
use ustr::Ustr;

use super::messages::QfexWsMessage;
use crate::{
    book::{parse_book_deltas, parse_book_depth, parse_quote},
    common::parse::{parse_rfc3339, price_from_decimal, quantity_from_decimal},
};

/// The index price has more decimals than the tick size; 9 fits every build of [`Price`].
///
/// [`Price`]: nautilus_model::types::Price
const MAX_INDEX_PRECISION: u32 = 9;

/// Converts one market data frame. `level2` gives book deltas (a whole snapshot) and a depth,
/// `bbo` a quote, the rest one item each; frames that carry no data give nothing.
///
/// # Errors
///
/// Returns an error if the symbol is unknown or a value does not parse.
pub fn parse_mds_message(
    message: &QfexWsMessage,
    instrument_of: impl Fn(&Ustr) -> Option<InstrumentAny>,
    ts_init: UnixNanos,
) -> anyhow::Result<Vec<Data>> {
    let instrument = |symbol: &Ustr| {
        instrument_of(symbol).ok_or_else(|| anyhow::anyhow!("no instrument for symbol {symbol}"))
    };

    Ok(match message {
        QfexWsMessage::Level2(book) => {
            let instrument = instrument(&book.symbol)?;
            let ts_event = parse_rfc3339(&book.time)?;
            let (bid, ask, seq) = (&book.bid, &book.ask, book.sequence);
            vec![
                Data::BookDeltas(Box::new(parse_book_deltas(
                    &instrument,
                    bid,
                    ask,
                    seq,
                    ts_event,
                    ts_init,
                )?)),
                Data::BookDepth(Box::new(parse_book_depth(
                    &instrument,
                    bid,
                    ask,
                    seq,
                    ts_event,
                    ts_init,
                )?)),
            ]
        }
        QfexWsMessage::Bbo(book) => {
            let instrument = instrument(&book.symbol)?;
            let ts_event = parse_rfc3339(&book.time)?;
            parse_quote(&instrument, &book.bid, &book.ask, ts_event, ts_init)?
                .map(Data::Quote)
                .into_iter()
                .collect()
        }
        QfexWsMessage::Trade(trade) => {
            let instrument = instrument(&trade.symbol)?;
            vec![Data::Trade(TradeTick::new_checked(
                instrument.id(),
                price_from_decimal(trade.price, instrument.price_precision())?,
                quantity_from_decimal(trade.size, instrument.size_precision())?,
                trade
                    .side
                    .map_or(AggressorSide::NoAggressor, |side| side.as_aggressor_side()),
                TradeId::new_checked(&trade.trade_id)?,
                parse_rfc3339(&trade.time)?,
                ts_init,
            )?)]
        }
        QfexWsMessage::MarkPrice(mark) => {
            let instrument = instrument(&mark.symbol)?;
            vec![Data::MarkPrice(MarkPriceUpdate::new(
                instrument.id(),
                price_from_decimal(mark.price, instrument.price_precision())?,
                parse_rfc3339(&mark.time)?,
                ts_init,
            ))]
        }
        QfexWsMessage::Underlier(index) => {
            let instrument = instrument(&index.symbol)?;
            let precision = index.price.scale().min(MAX_INDEX_PRECISION) as u8;
            vec![Data::IndexPrice(IndexPriceUpdate::new(
                instrument.id(),
                price_from_decimal(index.price, precision)?,
                parse_rfc3339(&index.time)?,
                ts_init,
            ))]
        }
        QfexWsMessage::Funding(funding) => {
            let instrument = instrument(&funding.symbol)?;
            let ts_event = parse_rfc3339(&funding.time)?;
            let next = UnixNanos::from(
                ts_event
                    .as_u64()
                    .saturating_add(funding.time_remaining.saturating_mul(1_000_000_000)),
            );
            vec![Data::FundingRate(FundingRateUpdate::new(
                instrument.id(),
                funding.funding_rate,
                Some(crate::http::client::FUNDING_INTERVAL_MINUTES),
                Some(next),
                ts_event,
                ts_init,
            ))]
        }
        _ => vec![],
    })
}

#[cfg(test)]
mod tests {
    use nautilus_model::{enums::BookType, orderbook::OrderBook, types::Price};
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::{http::parse::tests::instrument, websocket::messages::tests::fixture_lines};

    fn lookup(symbol: &Ustr) -> Option<InstrumentAny> {
        matches!(symbol.as_str(), "AAPL-USD" | "CL-USD" | "US100-USD")
            .then(|| instrument(symbol.as_str()))
    }

    fn replay() -> Vec<Data> {
        fixture_lines("mds")
            .iter()
            .flat_map(|line| {
                let message = QfexWsMessage::parse(line).unwrap();
                parse_mds_message(&message, lookup, UnixNanos::from(1)).unwrap()
            })
            .collect()
    }

    #[rstest]
    fn test_captured_frames_give_every_data_type() {
        let data = replay();
        let ts = UnixNanos::from(1_794_336_834_302_717_585);
        let id = instrument("AAPL-USD").id();

        // Every `level2` frame is a whole book: applying them in a row never leaves stale levels.
        let mut book = OrderBook::new(id, BookType::L2_MBP);
        let mut snapshots = 0;
        for item in &data {
            if let Data::BookDeltas(deltas) = item {
                book.apply_deltas(deltas).unwrap();
                snapshots += 1;
                assert_eq!(book.bids(None).count(), 20);
                assert_eq!(book.asks(None).count(), 20);
                assert!(book.best_bid_price().unwrap() < book.best_ask_price().unwrap());
            }
        }
        assert_eq!(snapshots, 3);

        let Some(Data::BookDepth(depth)) = data.iter().find(|d| matches!(d, Data::BookDepth(_)))
        else {
            panic!("no depth");
        };
        assert_eq!(depth.bids[0].price, Price::from("348.08"));
        assert_eq!(depth.ts_event, ts);

        let Some(Data::Quote(quote)) = data.iter().find(|d| matches!(d, Data::Quote(_))) else {
            panic!("no quote");
        };
        assert_eq!(
            (quote.bid_price, quote.ask_price),
            (Price::from("348.08"), Price::from("348.11"))
        );
        assert_eq!(quote.ts_event, ts);

        let Some(Data::Trade(trade)) = data.iter().find(|d| matches!(d, Data::Trade(_))) else {
            panic!("no trade");
        };
        assert_eq!(trade.instrument_id, instrument("CL-USD").id());
        assert_eq!(trade.aggressor_side, AggressorSide::Buy);
        assert_eq!(
            trade.trade_id.to_string(),
            "b99c2a90-5ddf-585a-91ce-e6b8e498a899"
        );

        let Some(Data::MarkPrice(mark)) = data.iter().find(|d| matches!(d, Data::MarkPrice(_)))
        else {
            panic!("no mark price");
        };
        assert_eq!(mark.value, Price::from("348.09"));

        let Some(Data::IndexPrice(index)) = data.iter().find(|d| matches!(d, Data::IndexPrice(_)))
        else {
            panic!("no index price");
        };
        assert_eq!(index.value, Price::from("348.10127732"));

        let Some(Data::FundingRate(funding)) =
            data.iter().find(|d| matches!(d, Data::FundingRate(_)))
        else {
            panic!("no funding rate");
        };
        assert_eq!(funding.rate, dec!(0.00001));
        assert_eq!(funding.interval, Some(60));
        assert_eq!(
            funding.next_funding_ns,
            Some(UnixNanos::from(funding.ts_event.as_u64() + 385_000_000_000))
        );
    }

    #[rstest]
    fn test_unknown_symbol_is_an_error_and_other_frames_give_nothing() {
        let line = fixture_lines("mds")
            .into_iter()
            .find(|line| line.contains(r#""type":"bbo""#))
            .unwrap()
            .replace("AAPL-USD", "NOPE-USD");
        let message = QfexWsMessage::parse(&line).unwrap();
        let error = parse_mds_message(&message, lookup, UnixNanos::default()).unwrap_err();
        assert_eq!(error.to_string(), "no instrument for symbol NOPE-USD");

        let none = parse_mds_message(&QfexWsMessage::Subscribed, lookup, UnixNanos::default());
        assert!(none.unwrap().is_empty());
    }
}
