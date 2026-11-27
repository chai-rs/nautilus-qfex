# QFEX

## Owner

QFEX (https://qfex.com), the perpetuals exchange. Docs: https://docs.qfex.com/ (`llms-full.txt` downloaded 11 Nov 2026); spec: https://api.qfex.com/openapi.yaml (fetched 11 Nov 2026). We have no copy of either in this folder yet; `docs/PLAN.md` records what the code follows.

## What we use

| kind | endpoint or channel | used for |
| --- | --- | --- |
| REST, public | `GET /refdata?api_version=2026-10-29` | instruments, instrument status |
| REST, public | `GET /md/orderbook/{symbol}` | book snapshot |
| REST, public | `GET /candles/{symbol}` | bars, bar history |
| REST, public | `GET /funding/{symbol}` | funding history |
| REST, HMAC | `/user/positions`, `/user/trade`, historic orders, `POST /user/leverage` | reports, account state, leverage |
| WS, public | `wss://mds.qfex.com` channels `level2`, `bbo`, `trade`, `mark_price`, `underlier`, `funding` | market data |
| WS, HMAC | `wss://trade.qfex.com?api_key=...` `auth`, `add_order`, `modify_order`, `cancel_order`, `cancel_stop_order`, `cancel_all_orders`, `get_user_orders`; channels `order_responses`, `fills`, `positions`, `balances` | execution |

UAT lives on `qfex.io` (`api.qfex.io`, `trade.qfex.io`; `mds.qfex.io` is assumed, not documented).

## What we cannot rely on

- **HMAC**: no test vector from QFEX. Our vector comes from `openssl`. No authenticated request has reached the venue.
- **Liveness**: the market data socket expects WebSocket pings to be answered both ways; a peer (or a local stand-in) that does not answer pings makes the adapter see a dead connection about every 14 s.
- **Order book**: no deltas, no checksum, no sequence per symbol; `sequence` counts per connection across all channels.
- **Frames that differ from the docs**: trade `side` is upper case live; `reduce_only` is shown as `0` and as `true`; three error frame shapes exist; an `ACK` may have an empty client order id.
- **Undocumented**: the `candle` WS channel's frames, REST rate limits, the cap on candles per response, what a refused login looks like.
- **Stop orders**: the docs describe `TAKE_PROFIT` / `STOP_LOSS`; live `/refdata` lists only `LIMIT, MARKET, ALO`.
- The full list of open questions is in `docs/PLAN.md`, section "Open questions".

## What we do when it is down

- Both sockets reconnect with backoff through `nautilus-network`, then resubscribe; the trade socket authenticates again and re-sends `cancel_on_disconnect`.
- After a trade socket reconnect, a catch-up task reads fills, open orders, closed orders and balance from 60 s before the last complete catch-up.
- A failed `add_order` send leaves the order submitted (the venue may have it); a failed cancel or modify send gives the matching rejected event.
- Actors learn about the outage from `SocketStateChanged` (market data socket first; see `ARCHITECTURE.md` FR-8).
- Tests and CI never contact QFEX: no authenticated request and no order goes to the live venue.
