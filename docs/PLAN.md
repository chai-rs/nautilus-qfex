# nautilus-qfex plan

NautilusTrader 0.67.0 Rust adapter for the QFEX perpetuals exchange. Same structure as
`nautilus-risex`. Written 11 Nov 2026, before the code; decisions 13 to 21 and the last open
questions were added while the execution client was written.

## Scope

In: `common`, `http` (public + HMAC REST), `websocket` (MDS client, trade client with auth,
heartbeat, reconnect, resubscribe), `book` (pulsed book), `signing` (HMAC headers + WebSocket auth
payload), `config`, `data` client, `execution` client, `factories`, two examples behind the
`examples` feature, Python binding (`python` / `extension-module` features), `fuzz/` package,
unit + integration tests on fixtures, `README.md`, this file, `docs/STATUS.md`.

Out (follow-ups in STATUS.md): benches, CI workflow, flatten bins, TWAP, subaccounts, transfers,
deposits/withdrawals, builder codes, wallet (web3) authentication, OAuth.

## Parity with nautilus-risex

| Feature | risex | qfex |
|---|---|---|
| Instruments | REST markets | REST `/refdata` (`PerpetualContract`) |
| Order book | WS deltas + checksum + REST recovery | WS `level2`: full 20-level snapshot every 500 ms; no deltas, no checksum, nothing to recover |
| Quotes | from book | WS `bbo` channel |
| Trades | WS + REST history | WS `trade`; no public REST trades, so `request_trades` is not supported |
| Bars | REST poll at bar close + history | REST `/candles` poll at bar close + history (no WS candle channel is documented) |
| Mark / index price | WS | WS `mark_price` / `underlier` |
| Funding | WS + REST history | WS `funding` + REST `/funding` history |
| Signing | EIP-712 session key | HMAC-SHA256 of `nonce:unix_ts` |
| Order entry | REST | trade WebSocket only |
| Submit / modify / cancel / cancel-all | yes | yes (modify returns a new venue order id) |
| Stop orders | TP/SL endpoint | `add_order` with `TAKE_PROFIT` / `STOP_LOSS`; cancel with `cancel_stop_order` |
| Cancel on disconnect | no | option, re-sent after each reconnect |
| Reports | REST | orders: WS `get_user_orders`; fills: REST `/user/trade`; positions + balance: REST `/user/positions` |
| Python binding, fuzz package | yes | yes |
| Benches, CI, flatten bin | yes | out of scope |

## Venue facts

Checked live on 11 Nov 2026 (public, no key):

- `GET https://api.qfex.com/refdata?api_version=2026-10-29`: 333 symbols (237 `ACTIVE`), categories
  `EQUITY` 304, `INDEX` 18, `COMMODITY` 7, `FX` 4; margin asset `USDC` for all; quote `USD` for 320,
  13 others in `KRW`, `JPY`, `SEK`, `HKD`, `CNY`, `INR`; every symbol lists order types
  `LIMIT, MARKET, ALO` and time in force `GTC, IOC, FOK`. Tick sizes run from `0.00001` to `100`,
  lot sizes from `0.000001` to `1000`. `min_price` / `max_price` are a moving price band.
- The response header `qfex-api-version` echoes the version. An unknown version gives HTTP 400
  `application/problem+json` (`{"title","status","detail"}`).
- `GET /md/orderbook/{symbol}` returns levels with size `"0"`; these are dropped.
- `GET /candles/{symbol}` returns newest first; `baseTokenVolume` was `"0"` while `usdVolume` was
  not, so bar volume is `usdVolume / close` when the base volume is zero.
- `GET /funding/{symbol}` returns `rate` as a JSON number.
- `wss://mds.qfex.com`: subscribe ack is `{"type":"subscribed","channels":[..],"symbols":[..]}`;
  `sequence` counts per connection across all channels; `level2` arrived every 500 ms with exactly
  20 + 20 levels and no zero sizes, so each message is a full snapshot; trade `side` is upper case
  (`BUY` / `SELL`; the docs example shows lower case); `funding` also carries
  `annualised_funding_rate`.
- No rate-limit or `retry-after` header was seen on public REST.

From the docs only (not checked, needs a key):

- REST `https://api.qfex.com`, trade socket `wss://trade.qfex.com?api_key=PUBLIC_KEY`, auth message
  within one minute. A UAT environment exists on `qfex.io` (`api.qfex.io`, `trade.qfex.io`).
- HMAC: lower-case hex HMAC-SHA256 of the string `{nonce}:{unix_ts}` keyed by the secret key.
  REST headers `x-qfex-public-key`, `x-qfex-hmac-signature`, `x-qfex-nonce`, `x-qfex-timestamp`.
  WebSocket: `{"type":"auth","params":{"hmac":{"public_key","nonce","unix_ts","signature"}}}`.
  Nonce: at most 100 characters, unique for 15 minutes. Timestamp: Unix seconds, within 5 minutes.
- Order entry is WebSocket only. Order types `LIMIT`, `MARKET`, `ALO` (post only), `TAKE_PROFIT`,
  `STOP_LOSS`. Time in force `GTC`, `IOC`, `FOK`. `modify_order` answers `MODIFIED` with a new
  `order_id`. `client_order_id` is not guaranteed unique.
- Rate limit: 12,000 weight units per 60 s on the trade socket. REST limits are not documented.

## Decisions

1. Instrument id `{symbol}-PERP.QFEX` (`AAPL-USD-PERP.QFEX`); raw symbol is the venue symbol.
2. Instrument type `PerpetualContract` with the asset class from `product_category`. The quote
   currency is `USDC` for `USD` symbols so that PnL and the USDC margin account share one currency;
   other quote currencies keep their code with `USDC` settlement.
3. `level2` is applied as a snapshot: `Clear` + one `Add` per level. Quotes come from `bbo`.
4. Bars: REST poll shortly after each bar close, like risex. The bar event time is the open
   time of the candle, the only time the venue sends.
5. `api_version` is pinned to `2026-10-29` and can be changed in the config.
6. HMAC through `nautilus_cryptography::signing::hmac_signature` (already in the tree). The nonce
   is a UUID4 without dashes; nonce and clock are injected so tests are deterministic.
7. One WebSocket transport for both sockets (`nautilus-network`, reconnect with backoff, 20 s ping
   frames). After connect and after each reconnect the trade client sends auth, waits for success,
   subscribes `order_responses`, `fills`, `positions`, `balances`, re-sends `cancel_on_disconnect`.
8. Orders are tracked by `client_order_id` (the Nautilus id is sent as is) and by venue `order_id`.
   Fills come only from the `fills` channel and are de-duplicated by trade id.
9. Account state: total = `available_balance + order_margin + position_margin`, locked =
   `order_margin + position_margin`, currency `USDC`.
10. `reduce_only` is sent as a JSON boolean and left out when false.
11. Mass status uses the trait default built from the three report methods.
12. Fuzz target bodies live in `src/fuzzing.rs` so unit tests can run them with a fixed-seed
    generator; `cargo-fuzz` is not installed here.
13. One HTTP client struct (no raw / domain split) and one WebSocket client type for both
    sockets: the venue is small enough.
14. Market and stop orders go out as IOC, post-only (`ALO`) as GTC. A FOK market order, a stop
    limit and every other type the venue lacks are denied before sending.
15. Cancel-all sends one cancel per open order of the strategy. The venue's `cancel_all_orders`
    would also cancel the orders of other strategies. Order lists go out order by order.
16. Reports read one page of 1,000 rows (REST) and one page of 1,000 open orders (socket).
17. After a reconnect a catch-up runs in its own task: REST fills, open orders over the socket,
    REST closed orders, REST balance, from the last complete catch-up minus 60 s.
18. `get_user_orders` answers have no request id, so any answer serves the oldest waiter.
19. Position frames are ignored; position reports come from REST. No flat report is made up.
20. A failed send of `add_order` is logged and the order stays submitted (the venue may have
    it). A failed send of cancel or modify gives the matching rejected event.
21. The REST client limits itself to 10 requests per second (no documented limit).

## Open questions

- No HMAC test vector in the docs. Our vector comes from `openssl` only.
- `reduce_only` on `add_order`: `0` in one JSON example, `true` in the code samples.
- Three error frame shapes are shown (`{"err":{..}}`, `{"type":"error",..}`, `{"type":"Err",..}`).
- Are stop orders listed under `orders` or `stop_orders` in `all_orders_response`?
- The changelog names `2026-10-30` while the version table lists only `2026-10-29`.
- Funding stream scaling: the text gives two different divisors. The rate is passed on unchanged.
- Is trade `side` the taker side? Assumed yes.
- `client_order_id` format limits (an `INVALID_CLIENT_ORDER_ID` status exists).
- Balance total formula (decision 9) and how PnL of non-USD symbols is converted to USDC.
- `wss://mds.qfex.io` (UAT market data) is not documented; assumed by analogy.
- WebSocket `user_trades` rows have no own-side field, so fill reports use REST `/user/trade`.
- Stop orders: the docs describe `TAKE_PROFIT` / `STOP_LOSS`, live `/refdata` lists only
  `LIMIT, MARKET, ALO` for every symbol.
- Does an ACK row echo `client_order_id`? The docs example has an empty id.
- After `MODIFIED`, does a `CANCELLED` row arrive for the old id? Is the modify quantity the
  total or the open rest? Assumed total.
- REST rate limit and the cap on candles per response are not documented.
- Does `terminal_status` of historic orders use the order-status values?
- Event time of a funding history row: taken as the end of the window (`window_start` plus
  the interval). The docs do not say.
- The fill example in the docs has redacted ids; the docs symbol `US500-USD` is not in live
  `/refdata` (the examples use `US100-USD`).
- No status is documented for an ALO order that would cross on add.
- The error-code list has no order-specific code beyond `InvalidOrder` / `InvalidOrderId`.
- What does a refused login look like on the trade socket?

## Sources

- https://docs.qfex.com/ `llms-full.txt`, downloaded 11 Nov 2026.
- https://api.qfex.com/openapi.yaml, fetched 11 Nov 2026.
- Live public captures (REST + `wss://mds.qfex.com`), 11 Nov 2026 (10 Nov 18:53 UTC).
- `nautilus-risex` (template) and the NautilusTrader 0.67.0 crates.
