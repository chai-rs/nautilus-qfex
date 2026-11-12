//! Pulsed book: one recorded `level2` frame to snapshot deltas, into a book, and to a depth.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use nautilus_core::UnixNanos;
use nautilus_model::{enums::BookType, instruments::Instrument, orderbook::OrderBook};
use nautilus_qfex::{
    book::{parse_book_deltas, parse_book_depth},
    http::{
        models::{QfexRefData, QfexSymbol},
        parse::parse_instrument,
    },
    websocket::messages::QfexWsMessage,
};

const FRAMES: &str = include_str!("../tests/fixtures/ws/mds.jsonl");
const REFDATA: &str = include_str!("../tests/fixtures/http/refdata.json");

// Line of the fixture: the 20 + 20 level book of AAPL-USD.
const LEVEL2: usize = 1;

fn bench(c: &mut Criterion) {
    let ts = UnixNanos::default();
    let refdata: QfexRefData = serde_json::from_str(REFDATA).unwrap();
    let aapl = refdata
        .data
        .into_iter()
        .filter_map(|item| serde_json::from_value::<QfexSymbol>(item).ok())
        .find(|symbol| symbol.symbol == "AAPL-USD");
    let aapl = parse_instrument(&aapl.unwrap(), ts).unwrap();
    let QfexWsMessage::Level2(frame) =
        QfexWsMessage::parse(FRAMES.lines().nth(LEVEL2).unwrap()).unwrap()
    else {
        panic!("line {LEVEL2} is not a level2 frame");
    };
    let (bids, asks, seq) = (&frame.bid, &frame.ask, frame.sequence);
    let deltas = parse_book_deltas(&aapl, bids, asks, seq, ts, ts).unwrap();
    let mut book = OrderBook::new(aapl.id(), BookType::L2_MBP);

    let mut group = c.benchmark_group("book");
    group.bench_function("deltas_20", |b| {
        b.iter(|| parse_book_deltas(&aapl, black_box(bids), black_box(asks), seq, ts, ts).unwrap());
    });
    // Every pulse is a whole book: the same one again clears the book and fills it.
    group.bench_function("apply_pulse_20", |b| {
        b.iter(|| book.apply_deltas(black_box(&deltas)).unwrap());
    });
    group.bench_function("depth10", |b| {
        b.iter(|| parse_book_depth(&aapl, black_box(bids), black_box(asks), seq, ts, ts).unwrap());
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
