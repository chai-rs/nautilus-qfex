# nautilus-qfex

QFEX perpetuals exchange adapter for NautilusTrader 0.67.0 (Rust). The parity target is
`nautilus-risex`.

**Status: not run against the venue with a key.** All tests run offline against fixtures and a
local mock server. Public market data ran once against mainnet without a key. No authenticated
request and no order went to QFEX; the HMAC signature has no test vector from QFEX. Read
`docs/STATUS.md` before you connect an account.

## Warnings

- `examples/exec_tester.rs` has `DRY_RUN = true`. With `false` it submits orders on **mainnet**
  with real funds. Start on UAT (`QfexEnvironment::Uat` in both configs).
- The execution path follows the documentation alone. Do the UAT steps of `docs/STATUS.md`
  before any mainnet order.

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

`data_tester` reads public mainnet data and uses no key. `exec_tester` needs the environment
variables below.

| Variable | Use |
|---|---|
| `QFEX_API_KEY` | Public API key (mainnet) |
| `QFEX_API_SECRET` | Secret key (mainnet) |
| `QFEX_UAT_API_KEY`, `QFEX_UAT_API_SECRET` | The same two for UAT (`qfex.io`) |

The process reads the keys at run time only and does not print them. The `Debug` output of the
config and of the credential redacts the secret. The examples do not load a `.env` file.

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
| Cancel all | Emulated | One cancel per open order of the strategy |
| Order list | Emulated | One `add_order` per order |
| Cancel on disconnect | Direct | Config option, sent again after each reconnect |

### Account and reconciliation (not run against the venue)

| Feature | State | Source |
|---|---|---|
| Account state | Direct | WS `balances`, REST `/user/positions` at connect |
| Order events | Direct | WS `order_responses` |
| Fill events | Direct | WS `fills`, one event per trade id |
| Order status reports | Direct | Open: WS `get_user_orders`; closed: REST historic orders |
| Fill reports | Direct | REST `/user/trade` |
| Position reports | Direct | REST `/user/positions` |
| Mass status | Emulated | Trait default over the three reports |
| Catch-up after reconnect | Emulated | Fills, open orders, closed orders, balance |

### Around the adapter

| Feature | State |
|---|---|
| HMAC signing (REST headers, WebSocket auth message) | Done; no vector from QFEX |
| Python binding | Done (see Python) |
| Fuzz package | Done; no target ran (see Fuzz) |
| Benches, CI workflow, flatten binary | Not done |
| TWAP, subaccounts, transfers, deposits and withdrawals, builder codes | Not done |
| Wallet (web3) authentication, OAuth | Not done |

## Limits of the emulated features

- **Order book**: the venue sends no deltas. Each `level2` message becomes a `Clear` plus one
  `Add` per level, so the book is at most 500 ms old and 20 levels deep per side.
- **Bars** are polled from REST shortly after the bar closes, not pushed. Intervals: 1, 5, 15,
  30 minutes, 1 and 4 hours, 1 day; `LAST` price, external aggregation only. The event time of
  a bar is the **open** time of the candle. The bar volume is `usdVolume / close` when the venue
  sends a zero base volume.
- **Cancel all** reads the open orders of the strategy from the cache and cancels them one by
  one. The venue's own mass cancel is not used: it would also cancel orders of other strategies.
- **Reports** read one page of 1,000 rows per request. Older rows are not fetched.
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
fixed-seed generator, so they run without `cargo-fuzz` too.

## License

LGPL-3.0-only.
