REVIEWER: code
MODEL: default
APPROVED: yes
APPROVED-BY: trustLevel autonomous

# Trade socket publishes its connection state

## Summary

- `QfexExecutionClient` builds one `SocketControl` for endpoint `trade`, venue `QFEX`, on its first `connect()`, keeps it, and hands `control.sink()` to every trade `QfexWebSocketClient` it builds through the existing `with_state_sink`; `nautilus-network` then reports `Connected` on connect and after each reconnect, and `Disconnected` when the transport is lost
- The control is built in `connect()`, outside the `SocketReconnectRegistry` scope in which `LiveNodeBuilder` constructs clients, so a controller reconnect command for the execution client keeps answering `Unsupported`, as it does today; no `UnknownEndpoint` side effect, no follow-up for it
- Login, catch-up, the reconnect log line and order handling are not touched; `Connected` means the transport is up, as the restate confirmed
- Proven by a new test in `tests/exec_client.rs` against the local stand-in: connect, drop, reconnect, deliberate disconnect, connect again, plus a registry lookup that shows the client never marked itself reconnect-capable
- Two slices: the behaviour with its test, its invariant and the test counts; then the prose in `docs/STATUS.md` and `ARCHITECTURE.md`

## Context

Code is read at origin/main (`362bcb0`); line numbers are from there.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `src/execution/mod.rs` | `QfexExecutionClient` (line 490) holds `core`, `config`, `emitter`, `shared`, `cancel`, `tasks`; `connect()` (line 716) builds `QfexWebSocketClient::new_trade(&url, signer, cancel_on_disconnect)` with no sink (line 750) on every connect; `consume` (line 416) logs "QFEX trade socket reconnected" and resets the login state on `QfexWsMessage::Reconnected` (line 444), and closes the socket when cancelled (line 485) | must change: a `TRADE_ENDPOINT` constant (`"trade"`), a field holding the `SocketControl`, created once on the first `connect()` from `SocketControlFactory::new(self.core.client_id, Some(*QFEX_VENUE))`, and `new_trade(..)?.with_state_sink(control.sink())` at line 750. `consume`, `logged_in` and the catch-up stay as they are |
| `src/websocket/client.rs` | `with_state_sink` (line 383) stores the sink; `connect()` passes it to `WebSocketClient::builder().maybe_state_sink(..)` (line 456) for both the market data and the trade client; `new_trade` (line 394) sets `state_sink: None` | reuse, do not rewrite: no change in this file |
| `src/data/mod.rs` | `MARKET_DATA_ENDPOINT` (line 62); `SocketControlFactory::new(client_id, Some(*QFEX_VENUE)).control(..)` in `QfexDataClient::new` (line 310); one control kept across `Feed::close` | the example of the shape to follow (one control per endpoint, a fresh `sink()` for each new socket), except where the control is built: here in `connect()`, not in the constructor |
| `src/common/consts.rs` | `QFEX_VENUE` (line 12) | reuse |
| `tests/exec_client.rs` | stand-in REST + trade socket on `127.0.0.1` (`Venue`, line 85); `Venue::drop_connection` (line 88) cuts the socket; `Harness::start` (line 384) sets the exec event sender, builds `QfexExecutionClient::new` and connects; `reconnect_logs_in_again_and_catches_up` (line 942) drops the socket and waits for the second `auth` | reuse, do not rewrite: the new test sets `replace_system_event_sender` before the client connects and builds the client inside `SocketReconnectRegistry::scope`, as `LiveNodeBuilder` does; the harness grows only what that needs |
| `tests/data_client.rs` | `next_state` and `socket_state_follows_the_market_data_socket` read `SystemEvent::SocketState` from a replaced system event sender | the example of the test to follow |
| `docs/data-model.md` | entity "Socket state" lists `execution` as a source; relationship says "`market_data` or the trade endpoint"; invariant 9 covers the market data socket only | must change: name the endpoint `trade` and extend invariant 9 to the trade socket |
| `docs/STATUS.md` | Verification rows say 168 passed (142 unit, 4 data client, 12 execution client); the socket state row says "The trade socket publishes no socket state" | must change: the counts in slice 1, the socket state row in slice 2 |
| `ARCHITECTURE.md` | on origin/main, step 7 of the data flow says no socket publishes `SocketStateChanged` and the market data socket is the first | must change: step 7 names both sockets |
| `checks.sh` | on origin/main, fmt, clippy and tests; PR #6 adds a step that fails when the counts in `docs/STATUS.md` differ from what `cargo test` ran | the count rows must be right in the slice that adds the test, so the slice passes whichever lands first |
| `docs/external/nautilustrader/README.md` | lists `SocketControlFactory::new(..).control(..)` and `SocketControl::sink()`, and the reconnect registry side effect | the outside contract this work relies on; complete for this work, no change |
| nautilus-live 0.67.0 src/socket.rs | `SocketControlFactory::new` reads the thread-local system event sender and the innermost registry scope at construction (lines 396-399, 438-450); outside a scope the control has no owner and registers nothing; each `sink()` advances the generation, so a sink handed to an older socket stops publishing | reuse, do not rewrite |
| nautilus-live 0.67.0 src/node/builder.rs | sets the node's senders on its thread (line 608), builds execution clients inside socket_registry.scope (line 701), then calls `register_client` | why the control is built in `connect()` rather than in `new` |
| nautilus-live 0.67.0 src/node/mod.rs | `request_socket_reconnect` maps `Unsupported` to `Unsupported`, `EndpointNotFound` to `UnknownEndpoint`, `ClientNotFound` to `UnknownClient` (lines 741-761) | what a controller sees for the execution client |
| nautilus-network 0.67.0 src/websocket/client.rs | the transport publishes `Connected` and `Disconnected` to its sink; a deliberate `disconnect()` publishes nothing | reuse |

This work depends on NautilusTrader, recorded in `docs/external/nautilustrader/README.md`. It adds no contact with QFEX: the test uses the stand-in on `127.0.0.1`.

Nearby work not yet merged: PR #5 (`2026-11-27-data-client-reconnect-command`) changes `src/websocket/client.rs` (a `reconnect` field and `reconnect_request`), `src/data/mod.rs` and the socket state row of `docs/STATUS.md`, and takes the count to 170. PR #6 (`2026-11-27-status-test-count-check`) makes `checks.sh` compare the counts. This plan does not touch the lines PR #5 changes in code; whichever lands second rebases the two count rows and the socket state row of `docs/STATUS.md`.

**This change has to reach** callers: `QfexExecutionClient::connect` only; `QfexExecClientFactory` and the Python factory call `QfexExecutionClient::new`, whose signature does not change · tests: `tests/exec_client.rs` (one new test, harness extended), the 12 existing execution client tests unchanged · fixtures: none · config: none, `QfexExecClientConfig` is unchanged · exports: none new; `QfexWebSocketClient::new_trade` and `with_state_sink` keep their signatures

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `QfexExecutionClient` builds one `SocketControl` (endpoint `trade`, venue `QFEX`) on its first `connect()`, keeps it, and passes `control.sink()` to every `new_trade(..)` through `with_state_sink`; new test `socket_state_follows_the_trade_socket` in `tests/exec_client.rs` (connect, drop, reconnect, deliberate disconnect, connect again, registry lookup); `docs/data-model.md` names endpoint `trade` in its relationship line and invariant 9; the two count rows of `docs/STATUS.md` say the new total | `cargo test --test exec_client socket_state` passes and shows `Connected`, `Disconnected`, `Connected`, then `Connected` after the second connect; `cargo test` reports 169 passed, 0 failed (171 on top of PR #5), the number both count rows of `docs/STATUS.md` say | todo |
| 2 | Prose follows the behaviour: the socket state row of `docs/STATUS.md` covers the trade socket (the new test, `Unsupported` for a reconnect command, the silent close after a refused login, why a later reconnect handler needs the control in `new`), the trade client row and the dated header line; `ARCHITECTURE.md` step 7 names both sockets | `git grep -n "publishes no socket state\|publishes nothing yet" -- docs/STATUS.md ARCHITECTURE.md` prints nothing and `git grep -n socket_state_follows_the_trade_socket -- docs/STATUS.md` prints the socket state row | todo |

## Acceptance criteria

1. After `connect()`, the execution client sends a `SystemEvent::SocketState` with state `Connected`, its own `client_id`, venue `QFEX` and endpoint `trade`, verified by `cargo test --test exec_client socket_state`
2. When the stand-in drops the trade socket, `Disconnected` then `Connected` follow for the same `(client_id, endpoint)`, and the client logs in again as before (a second `auth` frame reaches the stand-in), verified by the same test
3. After `disconnect()` and a second `connect()`, `Connected` is sent again for the same `(client_id, endpoint)`, verified by the same test
4. A deliberate `disconnect()` sends no `Disconnected`, verified by the same test seeing no socket event within 500 ms after `disconnect()`
5. Built inside a `SocketReconnectRegistry::scope`, the client does not mark itself reconnect-capable: `registry.get(client_id, "trade")` and `registry.get(client_id, "market_data")` both answer `ClientNotFound` after `connect()`, which a `LiveNode` (that also calls `register_client`) turns into `Unsupported`, verified by the same test
6. The 168 tests that pass at `362bcb0` still pass, verified by `cargo test` reporting 169 passed, 0 failed (171 if PR #5 has landed first), and the two count rows of `docs/STATUS.md` saying that number
7. `docs/data-model.md` names the endpoint `trade`, and its invariant 9 covers the trade socket, verified by reading it
8. `docs/STATUS.md` no longer says the trade socket publishes no socket state; its socket state row names the new test and says the reconnect command for the execution client answers `Unsupported`; `ARCHITECTURE.md` step 7 names both sockets, verified by `git grep -n "publishes no socket state\|publishes nothing yet" -- docs/STATUS.md ARCHITECTURE.md` printing nothing
9. No test opens a connection outside `127.0.0.1` and no key leaves the machine, verified by `git grep -n "qfex.com" -- tests/exec_client.rs` printing nothing, as at `362bcb0`

## Risks

- `SocketControlFactory::new` in `connect()` reads the system event sender of the thread that runs `connect()`. Under a `LiveNode` that is the node's own thread, where the builder set the sender (`builder.rs:608`); a caller that connects the client on a thread without that sender gets no event, and no crash. Criterion 1 runs `connect()` on the test thread with the sender set
- A refused login, at connect or after a reconnect, closes the socket on purpose, and a deliberate close publishes nothing: the last event an actor sees is then `Connected` while the client is stopped. This follows from `Connected` meaning the transport, which the restate confirmed; the refusal stays visible as the error that `connect()` returns or as the error log of `consume`. It is written into the socket state row of `docs/STATUS.md`
- A control built on every connect would leave an older sink live if a socket outlived its client's `disconnect()` (the consume task is aborted after 5 s); one control kept across connects retires the old sink through its generation. Criterion 3 runs through a second connect
- A later issue that registers a reconnect handler for the trade socket has to build the control inside the registry scope, that is in `new`; if it keeps it in `connect()`, `register` reaches no registry. Written into the socket state row of `docs/STATUS.md` so that issue reads it
- PR #5 and this work both edit the socket state row and the count rows of `docs/STATUS.md`; the second to land has a textual conflict there, which `deliver` resolves by keeping both facts and recounting

## Assumptions

- The endpoint label is `trade`, because the restate left it to planning, the QFEX docs and this crate call it the trade socket (`qfex_trade_ws_url`), and it differs from `market_data`
- The control is built on the first `connect()`, not in `QfexExecutionClient::new`, because the restate left the place to planning and this keeps the reconnect command answer at `Unsupported` instead of `UnknownEndpoint`, with no follow-up to open; the trade reconnect handler, which needs the constructor, is out of scope by the restate
- The test checks `SystemEvent::SocketState` from the client, not `SocketStateChanged` at a real actor, as `socket_state_follows_the_market_data_socket` does, because the node turns one into the other inside NautilusTrader
- The registry check asserts `ClientNotFound`, not `Unsupported`, because `SocketReconnectRegistry::register_client` is `pub(crate)` in `nautilus-live` 0.67.0; `ClientNotFound` here proves the client is not in the registry's `supported` set, which is what decides between `Unsupported` and `UnknownEndpoint` under a node
- `docs/data-model.md` is changed in the slice that makes its invariant true, because this project has no `openapi.yaml` (`contract.openapi` is null) and the event's shape belongs to NautilusTrader; only the endpoint value is ours
- `README.md` needs no change, because it states no test count and says nothing about socket state
- Port source: `src/data/mod.rs` of this crate, itself following the `nautilus-live` API; `nautilus-lighter` 0.67.0 is not vendored here
