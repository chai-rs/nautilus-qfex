# nautilus-qfex

QFEX perpetuals exchange adapter for NautilusTrader 0.67.0 (Rust). The parity target is
`nautilus-risex`. On 12 Nov 2026 the features of `nautilus-lighter` 0.67.0 that the venue
supports were added: the venue's mass cancel, leverage, report paging, instrument status.

**Status: not run against the venue with a key.** All tests run offline against fixtures and a
local mock server. Public market data ran once against mainnet without a key. No authenticated
request and no order went to QFEX; the HMAC signature has no test vector from QFEX. Read
`docs/STATUS.md` before you connect an account.

## Warnings

- `examples/exec_tester.rs` has `DRY_RUN = true`. With `false` it submits orders on **mainnet**
  with real funds. Start on UAT (`QfexEnvironment::Uat` in both configs).
- The execution path follows the documentation alone. Do the UAT steps of `docs/STATUS.md`
  before any mainnet order.
- `qfex-flatten` cancels every order and closes every position of the account. It prints the
  frames it will send and waits for Enter. It compiles and its plan is unit-tested; it was
  never run against the venue. It does not see or cancel TWAPs.

## Run

```bash
cargo test
```

```bash
cargo run --example data_tester --features examples
```

```bash
cargo run --example exec_tester --features examples
```

```bash
cargo run --bin qfex-trades-probe -- US100-USD-PERP.QFEX 20
```

```bash
cargo run --bin qfex-flatten        # mainnet
cargo run --bin qfex-flatten uat    # UAT
```

```bash
cargo bench --bench signing
```

`data_tester` and `qfex-trades-probe` read public mainnet data and use no key. `exec_tester`
and `qfex-flatten` need the environment variables below. There is no key-revoke binary: QFEX
keys are revoked in the web UI (Developer Settings), not over the API.

| Variable | Use |
|---|---|
| `QFEX_API_KEY` | Public API key (mainnet) |
| `QFEX_API_SECRET` | Secret key (mainnet) |
| `QFEX_UAT_API_KEY`, `QFEX_UAT_API_SECRET` | The same two for UAT (`qfex.io`) |

The process reads the keys at run time only and does not print them. The `Debug` output of the
config and of the credential redacts the secret. The examples and the binaries do not load a
`.env` file.

## Parity with `nautilus-risex`

- **Direct**: the venue has an endpoint or a channel for the same job.
- **Emulated**: the adapter builds the feature from other calls. The limits are in the next
  section.
- **Not on venue**: the venue has no such feature, or the docs do not show one.
- **Not done**: out of scope here.

### Data (public, no key)

| Feature | State | Source |
|---|---|---|
| Instruments | Direct | REST `/refdata` (`PerpetualContract`, id `AAPL-USD-PERP.QFEX`) |
| Order book deltas | Emulated | WS `level2`: a whole 20 + 20 level book every 500 ms |
| Order book depth | Direct | WS `level2` (10 levels of the 20) |
| Order book snapshot request | Direct | REST `/md/orderbook/{symbol}` |
| Quotes | Direct | WS `bbo` |
| Trades | Direct | WS `trade` |
| Trade history request | Not on venue | No public REST trades |
| Bars | Emulated | REST `/candles` poll after each bar close |
| Bar history request | Direct | REST `/candles` |
| Mark price, index price | Direct | WS `mark_price`, `underlier` |
| Funding rates | Direct | WS `funding`; history from REST `/funding` |
| Instrument status | Emulated | REST `/refdata` field `status`, polled every 60 s |

### Orders (trade WebSocket; not run against the venue)

| Feature | State | Source |
|---|---|---|
| Limit (GTC, IOC, FOK), post-only | Direct | `add_order` `LIMIT` / `ALO` |
| Market | Direct | `add_order` `MARKET`, IOC |
| Stop market, market if touched | Direct | `add_order` `STOP_LOSS` / `TAKE_PROFIT` |
| Stop limit, trailing stop, GTD | Not on venue | Denied before sending |
| Reduce only | Direct | `reduce_only` |
| Modify | Direct | `modify_order` / `modify_stop_order`; the venue gives a new order id |
| Cancel, batch cancel | Direct | `cancel_order` / `cancel_stop_order`, one frame per order |
| Cancel all | Direct or emulated | `cancel_all_orders` for the symbol when that hits no other order; else one cancel per order |
| Order list | Emulated | One `add_order` per order |
| Cancel on disconnect | Direct | Config `cancel_on_disconnect` (default off), sent after each login and reconnect |
| Leverage | Direct | REST `POST /user/leverage`: config `leverages` at connect, `set_leverage` on the client at run time |

### Account and reconciliation (not run against the venue)

| Feature | State | Source |
|---|---|---|
| Account state | Direct | WS `balances`, REST `/user/positions` at connect |
| Order events | Direct | WS `order_responses` |
| Fill events | Direct | WS `fills`, one event per trade id |
| Order status reports | Direct | Open: WS `get_user_orders`; closed: REST historic orders; paged |
| Fill reports | Direct | REST `/user/trade`; paged |
| Position reports | Direct | REST `/user/positions` |
| Mass status | Emulated | Trait default over the three reports |
| Catch-up after reconnect | Emulated | Fills, open orders, closed orders, balance |

### Around the adapter

| Feature | State |
|---|---|
| HMAC signing (REST headers, WebSocket auth message) | Done; no vector from QFEX |
| Python binding | Done (see Python) |
| Fuzz package | Done; each target ran 30 s in the CI `fuzz` job on 27 Nov 2026, no crash (see Fuzz) |
| Benches | Done: `book`, `data`, `exec`, `signing` (numbers in `docs/STATUS.md`) |
| `qfex-flatten` | Done; compiles, never run against the venue |
| `qfex-trades-probe` | Done; public market data socket, no key |
| Key-revoke binary | Not on venue: keys are revoked in the web UI |
| CI workflow | Done: `.github/workflows/ci.yml` (the `check` job runs `./checks.sh`, the `fuzz` job 30 s per fuzz target); runs on GitHub on every push and pull request since 12 Nov 2026, green on `main` at `4b46b86` |
| TWAP, subaccounts, transfers, deposits and withdrawals, builder codes | Not done |
| Wallet (web3) authentication, OAuth | Not done |

## Limits of the emulated features

- **Order book**: the venue sends no deltas. Each `level2` message becomes a `Clear` plus one
  `Add` per level, so the book is at most 500 ms old and 20 levels deep per side.
- **Bars** are polled from REST shortly after the bar closes, not pushed. Intervals: 1, 5, 15,
  30 minutes, 1 and 4 hours, 1 day; `LAST` price, external aggregation only. The event time of
  a bar is the **open** time of the candle. The bar volume is `usdVolume / close` when the venue
  sends a zero base volume. The market data socket has a `candle` channel, but the docs
  describe neither its subscribe frame nor its messages, so it is not used (see
  `docs/STATUS.md`).
- **Cancel all** sends the venue's `cancel_all_orders` for the symbol when the command has no
  side filter and the cache shows no other open order on the symbol. The venue's mass cancel
  takes every order of the account on the symbol, so with a side filter, or with an open order
  of another strategy on the symbol, the adapter sends one cancel per order as before. Stop
  orders always get their own cancel. An order on the symbol that the cache does not show as
  open (in flight, or placed outside Nautilus) is cancelled by the mass cancel too.
- **Reports** follow the venue's `offset` paging in pages of 1,000 rows, up to 20 pages
  (20,000 rows) per request; beyond that a warning is logged and older rows are missing. A row
  that arrives between two pages can come twice.
- **Instrument status** is the `status` of `/refdata`, read once a minute: `ACTIVE` is
  `Trading`, `INACTIVE` is `Suspend`, `DELISTED` is `NotAvailableForTrading`. An event is sent
  when the status is first read and when it changes, so a change is seen up to 60 s late. The
  market data socket has no status channel. `market_hours` of `/refdata` are the hours of the
  reference market, not trading hours of QFEX, and are not used.
- **Leverage** from the config is set at connect. The venue refuses a change while the symbol
  has an open order or a position; the refusal is logged and the connect goes on.
  `QfexExecutionClient::set_leverage` returns the refusal to the caller.
- **Market orders** are IOC at the venue. A FOK market order is denied.
- **Prices and sizes** go on the wire as JSON numbers (the docs show numbers). Values with up to
  15 significant digits read back unchanged.

## Python

The `python` feature has `QfexEnvironment`, the two configs and the two factories, registered
with the Nautilus PyO3 registry as in `nautilus-risex`.

The published `nautilus-core` 0.67.0 does not build its `python` feature from crates.io: it
needs `pyo3-stub-gen =0.20.0`, which does not compile with `pyo3` 0.30.1. NautilusTrader patches
that crate in its own workspace (`patches/pyo3-stub-gen`). Get that directory from the
NautilusTrader repository, then give its path to cargo:

```bash
cargo test --features python --test python --config 'patch.crates-io.pyo3-stub-gen.path="/path/to/nautilus_trader/patches/pyo3-stub-gen"'
```

With the patch, the feature compiles, clippy is clean, and the test passes (11 Nov 2026, patch
from commit `d53849b216e20f2c122ca01b4dd0da377638b186`).

Not confirmed: a wheel that a Python `TradingNode` can load. This crate has no `cdylib` target,
and a separate extension module has its own copy of the Nautilus registry. Use the Rust
`LiveNode` (see `examples/`).

## Fuzz

The `fuzz/` crate has six targets: `rest_parse`, `mds_parse`, `trade_parse`, `book_apply`,
`auth`, `wire_format`. It needs a nightly toolchain and `cargo-fuzz`:

```bash
cargo +nightly fuzz run rest_parse
```

The target bodies are in `src/fuzzing.rs`. `cargo test` runs each of them on 400 inputs from a
fixed-seed generator, so they run without `cargo-fuzz` too. The `fuzz` job of the CI workflow
installs nightly and `cargo-fuzz` on the runner and runs each target for 30 seconds.

## License

LGPL-3.0-only.
