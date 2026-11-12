# Status

Date: 11 Nov 2026. Plan: `docs/PLAN.md`.

## Summary

The crate builds and all offline tests pass. **No authenticated request went to QFEX, on UAT or
on mainnet, and no order was sent.** The only venue contact was public, read-only data: the
public fixtures in `tests/fixtures/` and one run of `data_tester`.

Public market data is verified against the live venue. **Authentication and execution are
UNVERIFIED against the venue, by design**: the task did not allow a key. They follow the
documentation (downloaded 11 Nov 2026) and are tested against a local mock server whose frames
are written by hand from the examples in the docs.

## Verification

Run on 11 Nov 2026, Rust 1.99.1, macOS.

| Check | Result |
|---|---|
| `cargo fmt --check` | Clean |
| `cargo build --all-targets --features examples` | Passes |
| `cargo test` | 145 passed, 0 failed: 135 unit, 1 data client, 9 execution client |
| `cargo clippy --all-targets --features examples -- -D warnings` | Clean |
| `cargo clippy --features python -- -D warnings` (with the patch, see Python) | Clean |
| `cargo test --features python --test python` (with the patch) | 1 passed |
| `cargo check --manifest-path fuzz/Cargo.toml` | Passes (type check only) |
| `cargo fmt --check --manifest-path fuzz/Cargo.toml` | Clean |

The tests use dummy key strings and a mock server on `127.0.0.1`. They open no connection to
the venue.

`data_tester` ran once without a key against public mainnet data on 11 Nov 2026 (10 Nov 20:04
UTC), for 25 s: 3,192 log lines, 0 `ERROR`, 0 `WARN`. 330 instruments loaded. For `AAPL-USD` and
`US100-USD`: 45 + 45 quotes, 45 + 45 book depth updates, 24 + 23 mark prices, 15 + 22 index
prices, 2 + 2 funding rates, 20 trades on `US100-USD` (none on `AAPL-USD` in that window), and 1
one-minute bar on `AAPL-USD`. The venue acknowledged every subscription.

`exec_tester` compiles. It was never run.

## State by feature

"Live" means checked against the public venue. "Mock" means unit tests and the local mock
server only.

| Feature | Verified how |
|---|---|
| `common` (enums, symbols, URLs, credential) | Unit tests; enum values of public data seen live |
| `http` public (`/refdata`, order book, candles, funding, problem body, `api_version`) | Live: fixtures recorded from the venue, and the `data_tester` run |
| `http` rate limit | UNVERIFIED: the docs give no REST limit; 10 requests per second is a guess. HTTP 429 with `retry-after` is retried (mock) |
| `http` authenticated (`/user/positions`, `/user/trade`, historic orders, fees, leverage) | UNVERIFIED on the venue: no key. Mock; the fixtures are hand-made from the docs |
| `signing` | HMAC checked against `openssl` and RFC 4231 case 2. UNVERIFIED on the venue (see Signing) |
| `websocket` MDS client (subscribe, ping, reconnect, resubscribe) | Live for subscribe and data. Reconnect and resubscribe: mock |
| `websocket` trade client (auth, session start, reconnect) | UNVERIFIED on the venue. Mock |
| `book` (pulsed book) | Live: recorded `level2` frames; one book per 500 ms seen in the run |
| `data` client | Live: the `data_tester` run. Mock: `tests/data_client.rs` on recorded frames |
| `execution` client (submit, modify, cancel, cancel all, stop orders, events) | UNVERIFIED on the venue. Mock: `tests/exec_client.rs` |
| Reconciliation (reports, catch-up after reconnect) | UNVERIFIED on the venue. Mock |
| `config`, `factories` | Unit tests; the data factory runs in `data_tester` |
| `examples/data_tester.rs` | Run once, keyless |
| `examples/exec_tester.rs` | Compiles only |
| Python binding | Compiles and 1 test with the patch. Loadable wheel: UNVERIFIED |
| Fuzz package | Type check only. No target ran |

## Signing

QFEX publishes no test vector, so no part of the signature is confirmed by the venue.

| Part | State |
|---|---|
| HMAC-SHA256 primitive | Checked: RFC 4231 test case 2 |
| Signature of `{nonce}:{unix_ts}`, lower-case hex | Checked against `openssl dgst -sha256 -hmac` with a dummy key. The message layout itself comes from the docs |
| REST header names and values | Docs only |
| WebSocket auth message (`unix_ts` as an integer) | Docs only |
| `?api_key=` on the trade socket URL | Docs only |
| Nonce: UUID4 without dashes, at most 100 characters | Length rule from the docs; the format is our choice |

## The UAT gate

Do these on UAT (`qfex.io`) with a UAT key, in this order, before any mainnet order:

1. `GET /user/positions`: proves the HMAC headers. A 401 here means the signature is wrong.
2. Connect the trade socket: proves the auth message and shows the real login answer.
3. Submit one post-only limit far from the market, then cancel it. Check that the ACK row
   carries the `client_order_id`.
4. Modify a resting order. Check the new order id, and whether a `CANCELLED` row arrives for
   the old id.
5. One small market order. Check the fill frame: trade id, fee sign, `aggressor_side`.
6. One stop order: is it accepted, where does `get_user_orders` list it, and which order id do
   its fills carry.
7. Drop the connection with an open order. Check the catch-up and `cancel_on_disconnect`.
8. Replace the hand-made fixtures with the recorded frames.

## Not verified against the venue

Authentication and execution as a whole. The points below are the assumptions inside them.

- The venue echoes `client_order_id` on ACK rows. The ACK example in the docs has an empty id.
  Without the echo, an ACK becomes an external order report.
- The id of a stop order is the `order_id` of its ACK row. Fills of a triggered stop order may
  carry another order id.
- Stop orders may not be accepted at all: live `/refdata` lists `LIMIT, MARKET, ALO` only for
  every symbol, while the docs describe `TAKE_PROFIT` and `STOP_LOSS`.
- After a modify, a `MODIFIED` row without client order id is matched to the oldest modify in
  flight. Whether the venue also sends `CANCELLED` for the old id is not known.
- Modify sends the new total quantity, not the open rest.
- Trade ids are unique per execution. The fee is in the margin asset (USDC); a negative fee is
  a rebate. Liquidity side comes from `aggressor_side`.
- The shape of a refused login. An error frame before login with no request type (or `auth`)
  stops the client.
- `get_user_orders` answers carry no request id: any answer serves the oldest waiting request.
- Open stop orders are read from both `orders` and `stop_orders`.
- REST `start` / `end` filters and the newest-first order of user trades and historic orders.
- Whether `terminal_status` of a historic order uses the order-status values.
- Account balance: total = available + order margin + position margin. No formula in the docs.
- Position frames on the socket are ignored. Position reports come from REST. A flat position
  gives no report.
- Prices and sizes go out as JSON numbers through `f64`. The docs show numbers; whether the
  venue takes every tick size this way is not known.
- Trade `side` on the public stream is taken as the taker side.
- The UAT market-data host `wss://mds.qfex.io` is assumed by analogy; the docs do not name it.
- The trade-socket rate limit (12,000 weight units per 60 s in the docs) is not enforced by the
  client.

## Not done

Follow-ups, out of scope here: benches, CI workflow, flatten binary, TWAP, subaccounts,
transfers, deposits and withdrawals, builder codes, wallet (web3) authentication, OAuth.

Also not done: `request_trades` (the venue has no public REST trades), WebSocket candles (no
channel in the docs), paging of reports beyond 1,000 rows, the venue's own `cancel_all_orders`.

## Shortcuts in the code

| Place | Limit |
|---|---|
| `src/http/client.rs:152` | REST rate limit of 10 requests per second is a guess |
| `src/http/client.rs:338` | One request per bar history call; no cap on candles is documented |
| `src/execution/mod.rs:91` | One page of 1,000 rows per REST report request |
| `src/execution/mod.rs:175` | One page of open orders (1,000) |
| `src/execution/dispatch.rs:122` | Closed orders stay tracked until the map is full |
| `src/execution/dispatch.rs:252` | A `MODIFIED` row without client order id goes to the oldest modify in flight |

## Python

The `python` feature compiles and its test passes, with one condition. The published
`nautilus-core` 0.67.0 needs `pyo3-stub-gen =0.20.0`, and that crate does not compile with
`pyo3` 0.30.1. NautilusTrader carries a local patch for it (`patches/pyo3-stub-gen`, commit
`d53849b216e20f2c122ca01b4dd0da377638b186`). The published crates do not carry the patch, so
the build needs it on the command line. The command is in `README.md`. Nothing is vendored in
this repository.

A command with the patch rewrites the `pyo3-stub-gen` entry of `Cargo.lock`; the next command
without the patch restores it.

Not confirmed: a route to a wheel that a Python `TradingNode` can load. This crate has no
`cdylib` target and no top-level module. A separate extension module has its own copy of the
Nautilus PyO3 registry, so the official wheel cannot see its factories. The Rust `LiveNode` does
not have this problem.

The module function `qfex` registers its extractors in a process-wide registry. A second call
in one process fails.

## Fuzz

Six targets type-check on 1.99.1: `rest_parse`, `mds_parse`, `trade_parse`, `book_apply`,
`auth`, `wire_format`. **No target ran**: `cargo-fuzz` and a nightly toolchain are not installed
here. There is no seed corpus.

The target bodies are in `src/fuzzing.rs`. Six unit tests run them on 400 inputs each from a
fixed-seed generator (fixtures with numeric values replaced by extreme ones, cut or damaged
fixtures, random text). These tests found one panic, now fixed: the balance sum overflowed the
decimal type (`parse_balance`, REST `/user/positions` and the socket balance frame). This is a
small substitute, not a fuzz campaign.

The `auth` target takes the timestamp as a `u64`, the type of the signer, not as a string.
