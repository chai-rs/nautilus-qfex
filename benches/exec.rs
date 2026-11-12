//! Order path before the socket: a Nautilus order to `add_order`, then to its frame.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use nautilus_model::{
    enums::{OrderSide, OrderType},
    identifiers::InstrumentId,
    orders::OrderTestBuilder,
    types::{Price, Quantity},
};
use nautilus_qfex::{execution::order::add_order, websocket::messages::QfexTradeRequest};
use ustr::Ustr;

fn bench(c: &mut Criterion) {
    let symbol = Ustr::from("AAPL-USD");
    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("AAPL-USD-PERP.QFEX"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("1.50"))
        .price(Price::from("200.25"))
        .build();
    let request = QfexTradeRequest::AddOrder(add_order(&order, symbol).unwrap());

    let mut group = c.benchmark_group("exec");
    group.bench_function("add_order", |b| {
        b.iter(|| add_order(black_box(&order), symbol).unwrap());
    });
    group.bench_function("add_order_frame", |b| {
        b.iter(|| black_box(&request).to_frame());
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
