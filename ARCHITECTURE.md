# nautilus-qfex

<!-- bloop:managed architecture -->

## Business

An internal adapter that lets NautilusTrader 0.67.0 trade QFEX perpetuals. Its user is mocha (`bidsize-ai/mocha`), which pins a commit of this repo and runs QFEX market data and orders through a Rust `LiveNode`. The value is that mocha's strategies and actors treat QFEX like any other Nautilus venue instead of talking to its REST and WebSocket APIs directly.

## What makes it different

The usual alternative is no adapter: mocha calls QFEX itself.

- **Native to Nautilus** data client, execution client and factories for NautilusTrader 0.67.0, with feature parity against `nautilus-risex` and the venue-supported parts of `nautilus-lighter`, so strategies need no QFEX-specific code
- **Emulates what the venue lacks, and says so** order book deltas, bars, cancel all and instrument status are built from other calls, and every limit of that emulation is written down in `README.md`
- **Offline-verified, honest about the rest** every test runs against fixtures and a local mock server; `docs/STATUS.md` separates what was checked against the live venue from what follows the documentation only

## Goals

| goal | key result | target | by | how it is measured |
| --- | --- | --- | --- | --- |
| G1 mocha actors see the market data socket go down and come back | `SocketStateChanged` events from venue `QFEX`, endpoint `market_data`, in order `Connected` → `Disconnected` → `Connected` | 3 of 3 | 2026-12-04 | `cargo test --test data_client socket_state`, and mocha pinning the merged rev |
| G2 an order path that has run against the venue, not only against the mock | UAT steps of `docs/STATUS.md` done: login, submit, cancel, one fill | 4 of 4 (now 0) | when a UAT key is available | the Verification table of `docs/STATUS.md` |
| G3 every change is gated | `./checks.sh` passes on every PR, with at least 162 tests | 100 % of PRs | ongoing | GitHub Actions `ci.yml` (has not run on GitHub yet) |

## Scope

**v1 does**

- Public data at parity: instruments, book, quotes, trades, bars, mark and index price, funding, instrument status
- Execution over the trade WebSocket: limit, market, stop, modify, cancel, cancel all, cancel on disconnect, leverage
- Reports and reconciliation, including catch-up after reconnect
- Socket state events an actor can subscribe to
- Python binding, fuzz targets, benches, `qfex-flatten`, `qfex-trades-probe`

**v1 does not**

- TWAP, subaccounts, transfers, deposits and withdrawals, builder codes
- Wallet (web3) authentication, OAuth
- A key-revoke binary (keys are revoked in the QFEX web UI)
- REST trade history, or order types the venue does not have (stop limit, trailing stop, GTD)
- A Python wheel that a `TradingNode` can load
- The WebSocket `candle` channel (its frames are not documented)

## Dataflow

1. `wss://mds.qfex.com` sends `level2`, `bbo`, `trade`, `mark_price`, `underlier` and `funding` frames to `websocket::client`
2. `websocket::handler` decodes them; `common::parse` and `book` turn them into Nautilus quotes, trades, book deltas, mark and index prices and funding rates
3. `data::QfexDataClient` publishes those on the Nautilus message bus, where mocha's actors and strategies read them
4. REST `/refdata`, `/candles`, `/funding` and `/md/orderbook` reach `http::client`; `http::parse` turns them into instruments, bars, funding history and book snapshots for the data client
5. A strategy's order command goes to `execution::QfexExecutionClient`; `dispatch` and `order` build the frame, `signing` signs it with HMAC, and the trade WebSocket sends it to QFEX
6. The trade WebSocket returns `order_responses`, `fills` and `balances`, and REST `/user/*` returns reports; the execution client turns them into order events, fill events, account state and reports for the Nautilus engine
7. Socket state: the market data socket publishes `SocketStateChanged` (endpoint `market_data`, venue `QFEX`) through a sink from one `SocketControl` that `data::QfexDataClient` keeps across `close()` (since `362bcb0`). The trade socket publishes nothing yet and only logs a reconnect (`2026-11-27-trade-socket-state`). Under a `LiveNode`, a controller reconnect command for the data client, endpoint `market_data`, reaches the socket: `Accepted` while connected, `Closed` before connect and after disconnect, because `data::market_data_socket` registers the socket's reconnect request with every sink it takes (since `f9b4079`)

## Modules

| module | owns which data | talks to |
| --- | --- | --- |
| `common` | venue enums, URLs, constants, credential (secret zeroized), shared parse | nothing (no I/O) |
| `signing` | HMAC headers and the WebSocket auth payload | `common` (no I/O) |
| `book` | the pulsed 20-level book and its deltas | `common` (no I/O) |
| `http` | REST client, response models, HTTP errors | QFEX REST, `signing`, `common` |
| `websocket` | connections, heartbeat, reconnect, resubscribe, MDS and trade messages | QFEX WebSockets, `signing`, `common` |
| `data` | subscriptions, bar and instrument status polling, market data events | `http`, `websocket`, `book`, Nautilus message bus |
| `execution` | open orders, venue order id mapping (modify gives a new id), seen fill trade ids, reports | `http`, `websocket`, Nautilus execution engine |
| `config`, `factories`, `python` | configuration and client construction for Rust and PyO3 | `data`, `execution` |

## Functional requirements

- FR-1 Load instruments from `/refdata` and refresh their status every 60 s
- FR-2 Subscribe and unsubscribe quotes, trades, book, mark price, index price, funding and bars
- FR-3 Request bar history, funding history and a book snapshot
- FR-4 Submit, modify and cancel every order type the venue supports, and deny the rest before sending
- FR-5 Emit order events, fill events and account state
- FR-6 Produce order, fill and position reports and mass status
- FR-7 Reconnect, resubscribe, and catch up on what was missed during the gap
- FR-8 Publish `SocketStateChanged` for a socket to subscribed actors (market data socket first)
- FR-9 Configure and build the clients from Rust and from Python
- FR-10 Cancel every order and close every position with `qfex-flatten`, after a confirmation

## Journeys

| journey | who | FRs |
| --- | --- | --- |
| J1 Watch the market: build a `LiveNode`, load instruments, subscribe, receive quotes, trades and book | mocha strategy | FR-1, FR-2, FR-3, FR-9 |
| J2 Trade: submit an order, get ack and fill, modify or cancel, see the account update | mocha strategy | FR-4, FR-5, FR-9 |
| J3 Restart: the node starts again and reconciles from order, fill and position reports | mocha operator | FR-6, FR-9 |
| J4 Network drop: the socket breaks, the actor sees `Disconnected`, the adapter reconnects and resubscribes, the actor sees `Connected`, missed data is caught up | mocha actor | FR-7, FR-8 |
| J5 Flatten: run `qfex-flatten`, read the frames it will send, press Enter, everything is closed | operator | FR-10 |

## Non-functional requirements

| concern | number | how it is checked |
| --- | --- | --- |
| latency p95 | decode + apply of one `level2` pulse ≤ 50 µs (measured about 11 µs) | `cargo bench --bench book` and `--bench data` |
| reconnect signalling | `Disconnected` within 1 s of detecting the drop; `Connected` after resubscribe | `tests/data_client.rs` against the mock server |
| liveness | the adapter answers WebSocket pings; a live peer is never treated as dead | unit tests of `websocket::client` |
| secrets | no secret in logs, `Debug` output or git | unit tests of `Debug` redaction; review |
| no live venue in tests | `cargo test` and CI run fully offline; no authenticated request or order goes to QFEX from a test or CI | tests bind `127.0.0.1` only; review |
| concurrent users | not applicable: one node per process | none |
| RPO / RTO | not applicable: the adapter keeps no durable state | none |
| data regulation | none | none |

## Tech stack

| layer | chosen | why |
| --- | --- | --- |
| language | Rust 1.99.1, edition 2024, pinned in `rust-toolchain.toml` | NautilusTrader's core is Rust |
| engine | NautilusTrader 0.67.0, every crate pinned `=0.67.0` | the version mocha runs |
| async and network | tokio, tokio-tungstenite (rustls), `nautilus-network` (sockudo) | what the other 0.67.0 adapters use |
| tests | `cargo test`, rstest, axum mock server, recorded fixtures | offline and deterministic |
| benches and fuzz | criterion; cargo-fuzz on nightly | parity with `nautilus-risex` |
| gate | `cargo fmt --check`, `cargo clippy --all-targets -D warnings` with and without `--features examples`, `cargo test` | the same steps as `ci.yml` |

## Codebase shape

```
src/
  common/      venue constants, enums, URLs, credential, shared parse   (no I/O)
  signing/     HMAC headers and WebSocket auth payload                  (no I/O)
  book/        pulsed 20-level book -> Nautilus deltas                  (no I/O)
  http/        REST client, models, parse
  websocket/   market data and trade sockets, heartbeat, reconnect, messages
  data/        QfexDataClient (Nautilus DataClient)
  execution/   QfexExecutionClient (Nautilus ExecutionClient)
  config/  factories/  python/   wiring for the Rust LiveNode and PyO3
tests/  benches/  fuzz/  bin/  examples/
```

**Allowed import direction** `config`/`factories`/`python` → `data`/`execution` → `http`/`websocket` → `signing`/`book` → `common`. `common`, `signing` and `book` do no I/O. `data` and `execution` reach the venue only through `http` and `websocket`. No tool enforces this yet; review does.

**Why this shape** It is not the bloop default (DDD + hexagonal). The whole crate is already an adapter: its ports are NautilusTrader's traits (`DataClient`, `ExecutionClient`, the factories) and its domain model is `nautilus-model`. The crate follows the module layout of the other Nautilus adapters (`nautilus-risex`, `nautilus-lighter`) so a change there can be ported here line by line. See `docs/adr/0001-codebase-shape.md`.

<!-- bloop:end architecture -->
