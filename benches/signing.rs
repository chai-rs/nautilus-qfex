//! Signing: the HMAC of one authentication, the REST headers and the WebSocket `auth` frame.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use nautilus_qfex::{common::credential::QfexCredential, signing::QfexAuth};

// The dummy values of the unit tests: not the key of any account.
const API_KEY: &str = "dummy-public-key";
const FAKE_SECRET: &str = "not-a-real-secret";
const NONCE: &str = "0123456789abcdef0123456789abcdef";
const UNIX_TS: u64 = 1_794_336_834;

fn bench(c: &mut Criterion) {
    let credential = QfexCredential::new(API_KEY, FAKE_SECRET);
    let auth = QfexAuth::sign(&credential, NONCE, UNIX_TS).unwrap();

    let mut group = c.benchmark_group("signing");
    group.bench_function("hmac_auth", |b| {
        b.iter(|| QfexAuth::sign(&credential, black_box(NONCE), black_box(UNIX_TS)).unwrap());
    });
    group.bench_function("rest_headers", |b| {
        b.iter(|| black_box(&auth).headers());
    });
    group.bench_function("ws_auth_frame", |b| {
        b.iter(|| black_box(&auth).ws_message());
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
