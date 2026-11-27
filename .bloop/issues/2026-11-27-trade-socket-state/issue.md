FR: FR-8
KIND: request
GOAL: none

An actor that calls `subscribe_socket_state` can see the QFEX trade socket connect, drop and reconnect, which it cannot today.

## What is there now (main at 362bcb0)

- The market data socket publishes `SocketStateChanged` (endpoint `market_data`, venue `QFEX`) since `2026-11-27-market-data-socket-state`.
- The trade socket publishes nothing. On reconnect `src/execution/mod.rs` only logs "QFEX trade socket reconnected" and resets its login state.
- `QfexWebSocketClient::with_state_sink` already exists; `new_trade` sets no sink.

## What is asked

- The execution client hands a sink from `SocketControlFactory` (venue `QFEX`, a trade endpoint label to be chosen in planning) to the trade `QfexWebSocketClient`, kept across every place the client is rebuilt.
- Decide in restate whether `Connected` should mean "transport up" or "transport up and logged in": the trade socket is only usable after `auth` succeeds.

## Related

- `2026-11-27-data-client-reconnect-command` covers the reconnect-registry side effect for the data client; the same side effect will appear for the execution client and should be handled the same way.

## Constraints

- No authenticated request and no order to the live QFEX venue. Tests use the local mock in `tests/exec_client.rs`.
