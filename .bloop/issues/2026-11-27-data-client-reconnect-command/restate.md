CONFIRMED: yes

# Controller can reconnect the market data socket

## What we will do

- In a `LiveNode`, a controller's reconnect command for the QFEX data client, endpoint `market_data`, reaches the live market data socket. The node reports `Accepted` and the socket drops and comes back on its own, no longer `UnknownEndpoint`.
- After that reconnect, the data client resubscribes to the channels it had, and an actor subscribed to socket state sees `Disconnected`, then `Connected`, for `market_data`.
- When the socket is not up (before `connect()`, or after `disconnect()`), the command gets an honest answer from the transport, such as `Disconnected` or `Closed`, never a reconnect of a dead client.
- This still holds after the data client disconnects and connects again: the socket it rebuilds answers the command too.
- A test builds the data client inside a `SocketReconnectRegistry` scope. It checks that looking up `market_data` resolves to a handle, and that a request through it reconnects the socket against a local stand-in server on `127.0.0.1`.
- All existing tests still pass, and `./checks.sh` passes. `docs/STATUS.md` drops the `UnknownEndpoint` side effect from the socket state row and records the new behaviour and the new test count. Step 7 of the data flow in `ARCHITECTURE.md` is corrected the same way.

## What we will not do

- Reconnect commands for the trade socket or the execution client: the trade socket publishes no state yet and has its own issue, `2026-11-27-trade-socket-state`.
- Going back to answering `Unsupported`: that would mean building the control outside the node's registry scope and giving up a reconnect the framework supports.
- Catching up on data missed during the gap (the rest of FR-7): market data has no replay here, and this issue is only about the command.
- Any contact with the live QFEX venue: tests use a local server only.

## Contracts affected

- `ReconnectSocket` system command (NautilusTrader): the outcome for client `QFEX-DATA`, endpoint `market_data`, changes from `UnknownEndpoint` to the transport's answer
- `SocketStateChanged` for endpoint `market_data`: also emitted around a controller-requested reconnect
