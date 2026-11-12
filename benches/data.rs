//! WebSocket data path: recorded market data frames to `QfexWsMessage`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use nautilus_qfex::websocket::messages::QfexWsMessage;

const FRAMES: &str = include_str!("../tests/fixtures/ws/mds.jsonl");

// Name, line of the fixture, frame type: the 20 + 20 level book and the top of book of
// AAPL-USD, and a US100-USD trade.
const CASES: [(&str, usize, &str); 3] = [
    ("level2_20", 1, "level2"),
    ("bbo", 2, "bbo"),
    ("trade", 16, "trade"),
];

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");
    for (name, line, kind) in CASES {
        let text = FRAMES.lines().nth(line).unwrap();
        assert!(
            text.contains(&format!(r#""type":"{kind}""#)),
            "line {line} is not a {kind} frame"
        );
        group.bench_function(name, |b| {
            b.iter(|| QfexWsMessage::parse(black_box(text)).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
