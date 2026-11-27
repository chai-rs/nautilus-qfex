FR: FR-8
KIND: request
GOAL: G1

An actor that calls `subscribe_socket_state` can see the QFEX market data socket connect, drop and reconnect, which it cannot today.

## What is there now (HEAD 4f4ed16)

- The adapter publishes no `SocketStateChanged` for the market data socket. On reconnect it only logs "QFEX market data socket reconnected" (`src/data/mod.rs:170`), so a subscribed actor sees nothing.

## What is asked

- Hand a sink from `SocketControlFactory` (endpoint `market_data`, venue `QFEX`) to the market data `WebSocketClient`, the way `nautilus-lighter` 0.67.0 does in its data client (`DATA_STREAMS_ENDPOINT`).
- Keep the sink both in `new` and in `close()`, which builds a new client.

## From the prototype (bidsize-ai/mocha, issue 2026-11-27-venue-websocket-connect)

- Done on a copy of this repo: about 20 lines added and 3 removed in `src/websocket/client.rs` and `src/data/mod.rs`.
- Result: the actor got `Connected` → `Disconnected` → `Connected`, with `client_id` and `venue` `QFEX`, when a local stand-in cut the socket and the adapter reconnected.
- The stand-in has to answer WebSocket pings, or the adapter sees the connection as dead about every 14 s.

## Check agreed with mocha

- `cargo test --test data_client socket_state` sees `Connected`, `Disconnected`, `Connected`.
- All existing tests still pass. mocha wrote 162; at HEAD 4f4ed16 the count is 166 (141 unit, 8 `qfex-flatten`, 2 `qfex-trades-probe`, 3 data client, 12 execution client).

## Constraints

- No authenticated request and no order to the live QFEX venue.
- When it merges, report the commit sha: mocha pins that rev.
