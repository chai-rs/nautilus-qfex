CONFIRMED: no

# Market data socket publishes its connection state

## What we will do

- An actor subscribed to socket state receives `Connected` when the QFEX data client's market data socket comes up, `Disconnected` when that socket is lost, and `Connected` again once the adapter has reconnected.
- Every one of those events names the data client's `client_id`, venue `QFEX` and endpoint `market_data`.
- This keeps working after the data client disconnects and connects again: the fresh socket it builds on `close()` still reports its state.
- A test in `tests/data_client.rs` whose name contains `socket_state` proves the sequence against a local stand-in server that answers WebSocket pings and drops the connection once. `cargo test --test data_client socket_state` runs it.
- All 166 existing tests still pass, and `./checks.sh` passes.
- `docs/STATUS.md` records the new behaviour and the new test count.

## What we will not do

- Socket state for the trade socket: not asked for, and its reconnect path (login, catch-up) needs its own look.
- Replacing the reconnect log line: it stays, the event is added next to it.
- Any contact with the live QFEX venue: the test uses a server on `127.0.0.1` only.

## Contracts affected

- `SocketStateChanged` (NautilusTrader system event) gains a publisher: endpoint `market_data`, venue `QFEX`
- `docs/data-model.md` invariants 8 and 9 become tested
