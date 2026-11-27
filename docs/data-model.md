# Data model

The adapter keeps no durable state. Its entities are NautilusTrader 0.67.0 types (`nautilus-model`, `nautilus-common`) built from QFEX frames, plus a few in-memory maps the clients own. This file lists them and the invariants that must hold at all times.

## Entities

| entity | type | built from | owner |
| --- | --- | --- | --- |
| Instrument | `PerpetualContract`, id `{symbol}-PERP.QFEX` | REST `/refdata` | `data` |
| Instrument status | `InstrumentStatus` | `/refdata` field `status`, polled every 60 s | `data` |
| Quote | `QuoteTick` | WS `bbo` | `data` |
| Trade | `TradeTick` | WS `trade` | `data` |
| Book | `OrderBookDeltas`, `OrderBookDepth10` | WS `level2` (full 20 + 20 snapshot every 500 ms), REST `/md/orderbook` | `book`, `data` |
| Bar | `Bar` (external, `LAST`) | REST `/candles` | `data` |
| Mark and index price | `MarkPriceUpdate`, `IndexPriceUpdate` | WS `mark_price`, `underlier` | `data` |
| Funding rate | `FundingRateUpdate` | WS `funding`, REST `/funding` | `data` |
| Order | Nautilus order events | WS `order_responses` | `execution` |
| Venue order id map | client order id ↔ venue `order_id` | `ACK`, `MODIFIED` rows | `execution` |
| Fill | `OrderFilled`, `FillReport` | WS `fills`, REST `/user/trade` | `execution` |
| Account state | `AccountState` (margin, `USDC`) | WS `balances`, REST `/user/positions` | `execution` |
| Position report | `PositionStatusReport` | REST `/user/positions` | `execution` |
| Socket state | `SocketStateChanged` (`client_id`, `venue`, `endpoint`, `state`) | the WebSocket client's connect, drop and reconnect | `websocket`, `data`, `execution` |

## Relationships

- Every market data event and every order refers to exactly one instrument id ending in `.QFEX`.
- An order has one client order id for its whole life and one venue order id at a time; a modify replaces the venue order id.
- A fill belongs to one order and has one venue trade id.
- A socket state event belongs to one client id and one endpoint (`market_data` or the trade endpoint).

## Invariants

Each one is a statement a test can check.

1. An instrument id is `{venue symbol}-PERP.QFEX` and its raw symbol equals the venue symbol.
2. A `level2` frame becomes one `Clear` delta followed by at most 20 `Add` deltas per side, and no level has size zero.
3. A `bbo` quote never has bid price > ask price.
4. One venue trade id produces at most one `OrderFilled` event, also across a reconnect and its catch-up.
5. After a `MODIFIED` row, the client order id maps to the new venue order id and the old venue order id maps to nothing.
6. An order type or time in force the venue lacks (stop limit, trailing stop, GTD, FOK market) is denied before any frame is sent.
7. Account total = `available_balance + order_margin + position_margin`, locked = `order_margin + position_margin`, currency `USDC`.
8. When a socket connects, drops and reconnects once, the events for its `(client_id, endpoint)` are `Connected`, `Disconnected`, `Connected` in that order. After `close()` builds a new WebSocket client, that client still publishes through a sink of the same endpoint.
9. Socket state events of the market data socket carry venue `QFEX` and endpoint `market_data`.
10. No secret appears in any `Debug` or `Display` output, log line, or event.
