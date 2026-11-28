# Status

Date: 11 Nov 2026, updated 12 Nov 2026 (section "Parity with `nautilus-lighter`"), 13 Nov
2026 (section "Tooling around the adapter"), 27 Nov 2026 (socket state of both sockets,
reconnect command of the data client, test counts) and 28 Nov 2026 (trade session frames bound to
their connection). Plan: `docs/PLAN.md`.

## Summary

The crate builds and all offline tests pass. **No authenticated request went to QFEX, on UAT or
on mainnet, and no order was sent.** The only venue contact was public, read-only data: the
public fixtures in `tests/fixtures/`, one run of `data_tester`, and on 12 Nov 2026 one
connection to the public market data socket with one subscribe frame (see "Instrument status").
On 13 Nov 2026, three more keyless connections to public mainnet data: one run of
`data_tester` (45 s), one run of `qfex-trades-probe` (3 trades) and one `candle` subscribe
(see "Candle channel").

Public market data is verified against the live venue. **Authentication and execution are
UNVERIFIED against the venue, by design**: the task did not allow a key. They follow the
documentation (downloaded 11 Nov 2026) and are tested against a local mock server whose frames
are written by hand from the examples in the docs.

## Verification

Run on 13 Nov 2026, Rust 1.99.1, macOS. The two `python` rows are from 12 Nov and were not run
again.

| Check | Result |
|---|---|
| `cargo fmt --check` | Clean |
| `cargo build --bins --examples --features examples` | Passes: `qfex-flatten`, `qfex-trades-probe`, `data_tester`, `exec_tester` |
| `cargo test --features examples` | 193 passed, 0 failed: 151 unit, 8 `qfex-flatten`, 2 `qfex-trades-probe`, 13 `exec_tester`, 5 data client, 14 execution client (28 Nov 2026, Linux, with the UAT steps of `exec_tester`; 180 with the recording only; 174 before it; 171 at `d779309` and `92dca56`, 170 at `4b46b86`, 168 at `362bcb0`, 166 at `4f4ed16`, 162 before `0e8ed08`, 152 before 13 Nov, 145 before 12 Nov) |
| `cargo test` (no feature, as in CI) | 180 passed: the same less the 13 `exec_tester` tests, which need the `examples` feature |
| `cargo clippy --all-targets --features examples -- -D warnings` | Clean |
| `cargo clippy --all-targets -- -D warnings` (no feature, as in CI) | Clean |
| `cargo bench --no-run` | Builds the four benches |
| `cargo clippy --features python -- -D warnings` (with the patch, see Python) | Clean (12 Nov) |
| `cargo test --features python --test python` (with the patch) | 1 passed (12 Nov) |
| `cargo check --manifest-path fuzz/Cargo.toml` | Passes (type check only) |
| `cargo fmt --check --manifest-path fuzz/Cargo.toml` | Clean |

The two `cargo test` counts above are checked by `./checks.sh` (step `test count`): a change in the number of tests fails the gate until these rows say the new total.

The tests use dummy key strings and a mock server on `127.0.0.1`. They open no connection to
the venue.

`data_tester` ran once without a key against public mainnet data on 11 Nov 2026 (10 Nov 20:04
UTC), for 25 s: 3,192 log lines, 0 `ERROR`, 0 `WARN`. 330 instruments loaded. For `AAPL-USD` and
`US100-USD`: 45 + 45 quotes, 45 + 45 book depth updates, 24 + 23 mark prices, 15 + 22 index
prices, 2 + 2 funding rates, 20 trades on `US100-USD` (none on `AAPL-USD` in that window), and 1
one-minute bar on `AAPL-USD`. The venue acknowledged every subscription.

It ran once more, without a key, on 13 Nov 2026 (12 Nov 17:29 UTC) for 45 s, now with the
instrument status subscription: 6,033 log lines, 0 `ERROR`, 0 `WARN`. 331 instruments loaded.
For `AAPL-USD` and `US100-USD`: 85 + 85 quotes, 85 + 85 book depth updates (10 bids and 10
asks each), 43 + 43 mark prices, 28 + 43 index prices, 2 + 3 funding rates, 2 + 90 trades and
1 + 1 instrument status (`Trading`, from the first read of `/refdata`). No bar: the run did
not reach the end of a minute. This run only checks that the example starts and shows book
depth and instrument status; a status change was not seen.

`exec_tester` compiles. It was never run. `qfex-flatten` compiles; never run against the
venue.

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
| `websocket` trade client (auth, session start, reconnect, socket state) | UNVERIFIED on the venue. Mock. The `auth` frame and the private `subscribe` and cancel-on-disconnect frames go out only on the connection they belong to (`send_text_on_connection`), so a socket that drops between the venue's `auth` answer and its handling, or a late reconnect notice, puts nothing on the next socket before its own `auth`: unit checks `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth` and `trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket` (fixed 28 Nov 2026, was 10 runs of 90 in the whole `exec_client` binary). An `auth` still waiting when its connection is replaced is refused by the writer and the next connection logs in from its own reconnect notice: unit check `trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in`. Orders sent with `send` during a reconnect still use the ordinary send and can be replayed on the next socket before its `auth` (`2026-11-28-trade-orders-before-live-login`) |
| Recording (`record_path` of `QfexExecClientConfig`) | Mock: `recording_holds_the_frames_and_no_key` (connect, submit, cancel: the file holds the `/user/positions` answer, the login answer, `add_order`, `ACK` and `CANCELLED`, and no key, secret, nonce, signature or `api_key=`) and the unit tests of `common::recording` (an `auth` frame sent is written as `{"type":"auth"}`; an error that echoes an `auth` request loses its `params`). One JSON line per REST answer (method, endpoint, query, status, raw body; no header) and per trade-socket text frame sent or received; appended, flushed line by line. Never run against the venue |
| `book` (pulsed book) | Live: recorded `level2` frames; one book per 500 ms seen in the run |
| `data` client | Live: the `data_tester` run. Mock: `tests/data_client.rs` on recorded frames |
| Socket state (`SocketStateChanged`, venue `QFEX`, endpoints `market_data` and `trade`) | Mock: `socket_state_follows_the_market_data_socket` (connect, drop, reconnect, deliberate disconnect, connect again) and the unit test `mds_state_sink_sees_connect_loss_and_reconnect`. Trade socket: Mock: `socket_state_follows_the_trade_socket` (connect, drop, reconnect with a second login, deliberate disconnect, connect again). `Connected` means the transport is up, not logged in; if the venue refuses the login, the client closes the socket deliberately, so an actor sees `Connected` and no `Disconnected`. Under a `LiveNode`, a controller reconnect command for the data client, endpoint `market_data`, reaches the socket: `Accepted` while connected (the socket drops, reconnects and resubscribes), `Closed` before `connect()` and after `disconnect()`. Mock: `reconnect_command_reconnects_the_market_data_socket` (inside a `SocketReconnectRegistry` scope) and the unit test `mds_reconnect_request_follows_the_connection`. The trade socket registers no reconnect handler: its control is built on the first `connect()`, outside the node's registry scope, so a reconnect command for the execution client answers `Unsupported`; a later handler for it would need the control built in `new` |
| Book depth (`subscribe_book_depth`) | Live: 45 + 45 depth updates in the `data_tester` run. Mock: 10 levels a side from the recorded 20-level `level2` frame |
| Instrument status (`subscribe_instrument_status`) | Mock on the recorded `/refdata`, which has all three status values. Live: the first status (`Trading`) of two symbols in the `data_tester` run of 13 Nov; a status change was never seen |
| `execution` client (submit, modify, cancel, cancel all, stop orders, events) | UNVERIFIED on the venue. Mock: `tests/exec_client.rs` |
| Mass cancel, cancel on disconnect, leverage | UNVERIFIED on the venue. Mock, hand-made frames |
| Reconciliation (reports, paging, catch-up after reconnect) | UNVERIFIED on the venue. Mock |
| `config`, `factories` | Unit tests; the data factory runs in `data_tester` |
| `examples/data_tester.rs` | Run twice, keyless (11 and 13 Nov) |
| `examples/exec_tester.rs` | Never run against the venue. Unit tests of the step mapping: UAT for every step, one recording file a step, no step or an unknown step refused |
| `bin/trades_probe.rs` (`qfex-trades-probe`) | 2 unit tests; run once, keyless, against the mainnet market data socket |
| `bin/flatten.rs` (`qfex-flatten`) | UNVERIFIED on the venue: compiles; never run against the venue. 8 unit tests of its plan and of how it reads the answers |
| Benches | Build; one run (see "Benches") |
| CI workflow | The `check` job runs `./checks.sh` (since 27 Nov 2026). The workflow runs on GitHub on every push and pull request since 12 Nov 2026: the first run on `main` failed, every run after it passed. Last checked: run `46026490835` on `main` at `4b46b86` (27 Nov 2026), both jobs green |
| Python binding | Compiles and 1 test with the patch. Loadable wheel: UNVERIFIED |
| Fuzz package | Each of the six targets ran 30 s in the CI `fuzz` job (run `46023650252`, 27 Nov 2026), no crash. Not run on this machine |

## Parity with `nautilus-lighter` (12 Nov 2026)

What `nautilus-lighter` 0.67.0 has and the venue can support. "Mock" means the local stand-in
of `tests/`, with frames written by hand from the docs.

| Feature | State | Verified how |
|---|---|---|
| Mass cancel | Added. `CancelAllOrders` without a side filter sends one `cancel_all_orders` with the symbol, if the cache shows no other open order on the symbol; else one cancel per order as before. Stop orders always get their own cancel. The `CANCELLED` rows come back as order events; a refused mass cancel gives a cancel rejection per order | Mock: `cancel_all_without_a_side_uses_the_mass_cancel_when_it_hits_nothing_else`, `cancel_all_cancels_the_orders_of_the_strategy_one_by_one` (now with a side filter). Unit: refusal in `test_refusals_reach_the_order_they_are_about` |
| Dead man's switch | Was there before: config `cancel_on_disconnect` (default off), sent after each login, so also after a reconnect. No code change | Unit: `trade_socket_authenticates_then_starts_the_session_again_after_reconnect`. Mock: `connect_signs_rest_logs_in_and_subscribes` |
| Leverage | Added. Config `leverages` (instrument to leverage) is set at connect over REST `POST /user/leverage`; `QfexExecutionClient::set_leverage` does it at run time. A refusal at connect is logged and does not stop the connect | Mock: `leverage_is_set_at_connect_and_on_request`. Python: the `leverages` argument in `tests/python.rs` |
| Report paging | Added. Fill reports, closed-order reports (REST `offset`) and open orders (`get_user_orders` `offset`) follow pages of 1,000 rows until a short page, at most 20 pages | Unit: `test_all_pages_stops_at_a_short_page_and_at_the_cap`. Mock: `reports_follow_the_pages` (1,001 open orders, 2,003 trades) |
| Instrument status | Added, as a poll. See below | Mock: `instrument_status_comes_from_refdata` on the recorded `/refdata` |
| Book depth | Was there before. Test added | Mock: `book_depth_is_ten_levels_of_the_pulsed_book` |

Not on the venue, so not done and still refused: real book deltas, stop-limit and
limit-if-touched orders, GTD, public trade history, batch / OCO / bracket messages.

### Instrument status

`/refdata` has a `status` per symbol: `ACTIVE`, `INACTIVE` ("temporarily suspended" in the
docs), `DELISTED` ("permanently suspended"). All three were in the live response of 11 Nov
2026 (237 / 93 / 3 of 333 symbols). The adapter maps them to `Trading`, `Suspend` and
`NotAvailableForTrading`.

There is no push for it. The docs have a "Reference Data" page under the market data socket,
but the socket refuses `refdata` as a channel: one subscribe on 12 Nov 2026 got
`unknown variant refdata`, with the list of channels the socket has (`level2`, `trade`,
`underlier`, `candle`, `funding`, `mark_price`, `open_interest`, `minmax_price`, `bbo`,
`market_stats`). None of them carries a status. So the adapter reads `/refdata` once a minute
while a status subscription is open.

`market_hours` of `/refdata` are the sessions of the reference market ("reference-price
sessions" in the docs), not trading hours: QFEX trades overnight and on weekends. They are
not turned into a status.

## Tooling around the adapter (13 Nov 2026)

What `nautilus-risex` has around its adapter: benches, binaries, CI.

| Item | State | Verified how |
|---|---|---|
| Benches | Added: `benches/book.rs`, `data.rs`, `exec.rs`, `signing.rs`, criterion 0.8.2 (dev-dependency, the only new one) | `cargo bench --no-run` builds; one run, see "Benches" |
| `qfex-trades-probe` | Added: `bin/trades_probe.rs`. Market data socket only, one `trade` subscription, no REST request, no key. Prints the venue's own fields | Unit: `test_arguments_give_the_venue_symbol_and_the_limit`, `test_trade_line_of_a_recorded_frame`. Live: one run on 13 Nov 2026 (12 Nov 17:28 UTC), 3 trades of `US100-USD` in 3 s, exit code 0 |
| `qfex-flatten` | Added: `bin/flatten.rs`. **Compiles; never run against the venue** | Unit, offline: `test_plan_cancels_first_then_closes_every_position`, `test_plan_sends_only_what_is_needed`, `test_outcome_tells_a_refusal_from_progress` (6 cases) |
| Key-revoke binary | Not built: the venue has no API call for it | See "Key revocation" |
| CI | Added: `.github/workflows/ci.yml` | The `check` job runs `./checks.sh` (since 27 Nov 2026). The workflow runs on GitHub since 12 Nov 2026, green on `main` at `4b46b86`. The `fuzz` job runs on the runner only: nightly and `cargo-fuzz` are not installed on this machine |
| `data_tester` | Subscribes to instrument status as well (book depth was already there) | The run of 13 Nov, see "Verification" |
| WebSocket candles | Not done: the wire format is not in the docs | See "Candle channel" |

The binaries need no feature, as in `nautilus-risex`. Unlike there, their `[[bin]]` entries do
not set `test = false`, because each file holds the unit tests of its own logic.

### `qfex-flatten`

It reads the positions (`GET /user/positions`), logs in on the trade socket and reads the open
orders (`get_user_orders`, pages of 1,000). These three need the key and change nothing. Then
it prints the frames it will send and waits for Enter. The frames, in this order:

1. One `cancel_all_orders` with empty `params`, if a plain order is open.
2. One `cancel_stop_order` for each stop order (a row of `stop_orders`, or a row of `orders`
   with the type `TAKE_PROFIT` or `STOP_LOSS`).
3. One `add_order` for each position that is not zero: `MARKET`, `IOC`, `reduce_only: true`,
   the opposite side, the absolute size.

`uat` as the first argument selects UAT; the default is mainnet. The keys come from
`QFEX_API_KEY` / `QFEX_API_SECRET` (or `QFEX_UAT_API_KEY` / `QFEX_UAT_API_SECRET`) at run time;
no `.env` file is read.

Limits:

- It does not see or cancel TWAPs (`twaps` of the `get_user_orders` answer is not read). A
  running TWAP keeps sending orders after the flatten.
- The venue also has a `close_position` request (docs line 9905: a reduce-only `LIMIT` `IOC`
  near the mark price, sized by the engine). It is not used: the adapter has no such frame.
- With 20,000 open orders or more it stops before anything is sent.

### CI

The `check` job runs `./checks.sh`, the same gate as on a dev machine: `fmt --check`, clippy
with `-D warnings` and tests, each with and without `--features examples`, then the test count
and shortcut marker checks (toolchain 1.99.1). The `fuzz` job
installs nightly and `cargo-fuzz` on the runner and runs each file of `fuzz/fuzz_targets` for
30 seconds (`-max_total_time=30`). Each command of that job carries `+nightly`, because
`rust-toolchain.toml` pins the crate to 1.99.1.

## Benches

One run of `cargo bench` on an Apple M3 Max, 13 Nov 2026, criterion defaults. The numbers are
indicative only.

| Bench | Time |
|---|---|
| `signing/hmac_auth` (HMAC-SHA256 of `{nonce}:{unix_ts}`) | 475 ns |
| `signing/rest_headers` (the four REST headers) | 322 ns |
| `signing/ws_auth_frame` (the `auth` frame) | 922 ns |
| `decode/level2_20` (0.9 KB frame, 20 + 20 levels) | 5.71 µs |
| `decode/bbo` | 841 ns |
| `decode/trade` | 861 ns |
| `book/deltas_20` (one pulse to snapshot deltas) | 1.52 µs |
| `book/apply_pulse_20` (the deltas into an `OrderBook`) | 4.90 µs |
| `book/depth10` (the 10-level cut) | 1.02 µs |
| `exec/add_order` (a limit order to the request) | 62 ns |
| `exec/add_order_frame` (the request to JSON) | 274 ns |

The frames are the recorded ones of `tests/fixtures/ws/mds.jsonl`. The signing benches use
the dummy key of the unit tests. `book/apply_pulse_20` applies the same pulse again and again:
each pulse clears the book and fills it.

## Key revocation

No key-revoke binary (`nautilus-risex` has `risex-signer-revoke`): the docs describe no API
call that revokes or deletes an API key. Line numbers are those of the docs export
`llms-full.txt`, downloaded 11 Nov 2026.

- Line 63 (page "API Keys"): "Revoke the key from Developer Settings", which is the web UI
  (lines 10 to 18).
- Lines 81 to 83 and 217: a key is created over the API with
  `POST /rest/v1/rpc/generate_api_key_v3` on `verify.qfex.com`, with a user session and recent
  MFA, "not an existing API key". It is the only `rpc` call in the docs; there is none to
  delete a key.
- Lines 79 and 898 to 900 (builder keys): a new key for the same user and builder label
  replaces the old one, and "There is no automatic revocation". This needs the key of a
  builder and a wallet signature of the user; it is not a revocation of one's own key.
- The only `DELETE` in the docs is `/bank-account/{id}` (line 1605).

## Candle channel

Live bars stay on the REST poll. The docs do not describe the `candle` channel of the market
data socket, so it is not used. What the docs have (same export, same line numbers):

- Lines 1123 to 1136 (Enums, `CandlesInterval`): "Use the wire value when subscribing to the
  WebSocket candle channel", with the values `1MIN`, `5MINS`, `15MINS`, `30MINS`, `1HOUR`,
  `4HOURS`, `1DAY`.
- Line 5103 (Market Stats): "`market_stats` ignores `intervals` and `sig_figs`", the only
  place that names an `intervals` field of a subscription.
- Line 7441 (MDS Playground): "Single channel for orderbook, trades, candles", no schema in the
  export.
- The channel pages of the market data socket (lines 4427 to 5969) are BBO, funding rate, mark
  price, market stats, minmax price, open interest, order book, reference data, trades and
  underlier. There is no candle page. The changelog (lines 4151 to 4426) and the CLI page
  (lines 917 to 1023) do not mention the channel.

Missing in the docs: the subscribe frame (where `intervals` goes), the message, whether a
frame says that a candle is closed, and whether a minute without a trade sends a frame.

One subscribe on 13 Nov 2026 (12 Nov 17:22 UTC; one attempt, no second one), with the frame
put together from the two hints above. UNVERIFIED beyond this one observation, and not in the code:

```
sent     {"type":"subscribe","channels":["candle"],"symbols":["US100-USD"],"intervals":["1MIN"]}
answer   {"type":"subscribed","channels":["candle"],"symbols":["US100-USD"],"intervals":["1MIN"]}
message  {"sequence":1,"type":"candle","symbol":"US100-USD","start":"2026-11-12T17:22:00Z","open":"28338","high":"28343","low":"28325","close":"28334","usdVolume":"32650.3707","resolution":"1MIN"}
```

Seven such messages came within half a second, all for the candle that was still open
(`start` 17:22:00, read during that minute), with a growing `usdVolume`. The probe stopped
after eight frames, so the change to the next candle was not seen. There is no base volume and
no "closed" field. To switch the bars to this channel, a bar would be emitted when `start`
moves on; that rule and the frames above are observed once, not documented, so the switch was
not made.

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
9. `qfex-flatten uat` with one resting order, one stop order and one small position. Check
   that the plan lists all three, and that the account is flat after Enter.

### Commands

Each command reads `QFEX_UAT_API_KEY` and `QFEX_UAT_API_SECRET` from the environment; set
them in your shell, never in a file of the repository. `exec_tester` runs on UAT only and
writes `recordings/uat-step-<step>.jsonl`, which git ignores. Stop it with Ctrl-C. The tester
trades `AAPL-USD-PERP.QFEX`, which trades in its market hours only.

| Step | Command | What it does |
|---|---|---|
| 1 and 2 | `cargo run --example exec_tester --features examples -- 2` | Connects: `GET /user/positions`, then the trade socket login. Dry run: no order |
| 3 | `cargo run --example exec_tester --features examples -- 3` | One post-only limit buy 100 ticks under the best bid, cancelled on Ctrl-C |
| 4 | `cargo run --example exec_tester --features examples -- 4` | As 3, then one modify of the resting order once it is accepted |
| 5 | `cargo run --example exec_tester --features examples -- 5` | One market buy of 0.010 on the first quote, closed on Ctrl-C |
| 6 | `cargo run --example exec_tester --features examples -- 6` | One stop-market buy 100 ticks over the market, cancelled on Ctrl-C |
| 7 | `cargo run --example exec_tester --features examples -- 7` | One resting post-only limit buy with cancel on disconnect. Drop the network by hand for longer than 20 s (the heartbeat), then bring it back |
| 8 | none | The agent cuts fixtures from the files of `recordings/` |
| 9 | `cargo run --bin qfex-flatten uat` | Prints its plan, waits for Enter. Records nothing |

The UAT market data host `wss://mds.qfex.io` is assumed (see below). If the tester gets no
quote, steps 3 to 7 place no order; the recording of steps 1 and 2 still holds.

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
- UNVERIFIED: `cancel_all_orders` with a `symbol`, and one `CANCELLED` row for each cancelled
  order. Whether it reaches stop orders is not in the docs: the adapter sends each stop order
  its own cancel as well. A refusal is read from the error frame that sends the request back
  (the `RateLimited` example of the docs).
- UNVERIFIED: `cancel_on_disconnect`. The frame is from the docs; whether the venue answers it
  is not known and no answer is waited for.
- UNVERIFIED: `POST /user/leverage` with `{"symbol", "leverage"}`, the leverage as a JSON
  number; only the HTTP status is read. The docs describe leverage on the trade socket
  (`set_user_leverage`) and say a change is refused while the symbol has an open order or a
  position. What the venue answers when the value does not change is not known.
- UNVERIFIED: paging. `limit` and `offset` are documented; that a page with fewer rows than
  `limit` is the last one is assumed, for REST and for `get_user_orders`. Whether the `limit`
  of `get_user_orders` counts orders and stop orders together or each list alone is not known;
  the adapter is right in both cases.
- UNVERIFIED: that `status` in `/refdata` changes at the moment the venue stops taking orders
  for a symbol. The rejection `REJECTED_MARKET_CLOSED` may happen while the status is
  `ACTIVE`. The values themselves are seen live.
- UNVERIFIED (`qfex-flatten`, docs only): `cancel_all_orders` with empty `params` cancels
  every active order of the account, on every symbol. Whether it reaches stop orders is not
  in the docs: each stop order gets its own `cancel_stop_order`.
- UNVERIFIED (`qfex-flatten`, docs only): a `MARKET` `IOC` `add_order` with `reduce_only: true`
  and an empty `client_order_id` ("If empty, one is generated", Add Order page). The example
  on that page shows `"reduce_only": 0`, the code samples show `true`; the adapter sends
  `true`.
- UNVERIFIED (`qfex-flatten`): the answers to its frames carry no request id. It prints every
  order row, fill and error frame for 10 s after sending and counts a refusal status or an
  error frame as a failure.

## Not done

Follow-ups, out of scope here: TWAP, subaccounts, transfers, deposits and withdrawals, builder
codes, wallet (web3) authentication, OAuth.

Also not done: `request_trades` (the venue has no public REST trades), a key-revoke binary
(see "Key revocation") and WebSocket candles (see "Candle channel"). Bars stay on the REST
poll.

## Shortcuts in the code

| Place | Limit |
|---|---|
| `src/http/client.rs:152` | REST rate limit of 10 requests per second is a guess |
| `src/http/client.rs:338` | One request per bar history call; no cap on candles is documented |
| `src/execution/mod.rs:127` | A report stops after 20 pages of 1,000 rows; a row that arrives between two pages can come twice |
| `src/execution/mod.rs:857` | The mass cancel also takes orders on the symbol that the cache does not show as open |
| `src/data/mod.rs:62` | Instrument status is a poll of `/refdata` every 60 s |
| `src/execution/dispatch.rs:122` | Closed orders stay tracked until the map is full |
| `src/execution/dispatch.rs:252` | A `MODIFIED` row without client order id goes to the oldest modify in flight |
| `bin/flatten.rs:260` | One IOC order per position: a thin book leaves a rest, so run it again until it prints "Nothing to do" |

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
`auth`, `wire_format`. They run only on the CI runner: `cargo-fuzz` and a nightly toolchain are
not installed here. There is no seed corpus. The `fuzz` job of `.github/workflows/ci.yml` runs
each target for 30 seconds; in run `46023650252` (27 Nov 2026, `main` at `fda6185`) each of the
six finished its 30 seconds (between 0.5 and 2.6 million runs) with no crash.

The target bodies are in `src/fuzzing.rs`. Six unit tests run them on 400 inputs each from a
fixed-seed generator (fixtures with numeric values replaced by extreme ones, cut or damaged
fixtures, random text). These tests found one panic, now fixed: the balance sum overflowed the
decimal type (`parse_balance`, REST `/user/positions` and the socket balance frame). This is a
small substitute, not a fuzz campaign.

The `auth` target takes the timestamp as a `u64`, the type of the signer, not as a string.
