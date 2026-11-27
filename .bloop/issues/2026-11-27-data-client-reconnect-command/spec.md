REVIEWER: code
MODEL: default
APPROVED: yes
APPROVED-BY: trustLevel autonomous

# Controller can reconnect the market data socket

## Summary

- `QfexWebSocketClient` keeps the `WebSocketReconnectHandle` of its live transport in a shared slot (set in `connect()`, cleared in `disconnect()`) and hands out a sync request function that answers `Closed` while the slot is empty and the transport's own `ReconnectRequestOutcome` otherwise
- `QfexDataClient` calls `SocketControl::register` with that function right after every `socket.sink()`: once in `new`, and again in `Feed::close` for the rebuilt client, because each `sink()` drops the previous registration
- Under a `LiveNode`, a `ReconnectSocket` command for `QFEX-DATA` / `market_data` then resolves to a handle: `Accepted` while connected (the socket drops, reconnects and resubscribes through the existing `RECONNECTED` path), `Closed` before `connect()` and after `disconnect()`, never `UnknownEndpoint`
- Proven by a unit test on the WebSocket client and an integration test in `tests/data_client.rs` that builds the data client inside a `SocketReconnectRegistry` scope against the local stand-in on `127.0.0.1`
- Two slices: the WebSocket client exposes the request (no behaviour change for its callers), then the data client registers it, with the test and the docs

## Context

Line numbers are those of origin/main at `362bcb0`, where the adapter code as merged lives.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `src/websocket/client.rs` | `QfexWebSocketClient` (line 343) holds `client: Option<Arc<WebSocketClient>>`; `connect()` (438) builds the transport with `.maybe_state_sink(..)` and starts `read_loop`, which on `RECONNECTED` (615) clears the gate and runs `start_session` again (resubscribe); `disconnect()` (481) drops the transport; `with_state_sink` (383) | must change: add a field `reconnect: Arc<parking_lot::Mutex<Option<WebSocketReconnectHandle>>>`, filled with `client.reconnect_handle()` in `connect()` and emptied in `disconnect()`; add a method returning `impl Fn() -> ReconnectRequestOutcome + Send + Sync + 'static` over a clone of that slot. `new_mds`, `new_trade` and `with_state_sink` keep their signatures. The `RECONNECTED` path is reused, do not rewrite: it already resubscribes after any transport reconnect |
| `src/websocket/client.rs` (tests module, `MockVenue`, `mds_state_sink_sees_connect_loss_and_reconnect` at 836) | stand-in venue for both sockets; counts connections, records frames per connection | reuse, do not rewrite: the new unit test uses `MockVenue` to show `Closed` before connect, `Accepted` and a second connection with the subscriptions sent again, `Closed` after disconnect |
| `src/data/mod.rs` | `QfexDataClient::new` (300) builds one `SocketControl` for `market_data` inside whatever registry scope is current (309-310) and keeps it as `Feed::socket: Arc<SocketControl>` (130); `Feed::close` (271-278) replaces the client with `new_mds(..).with_state_sink(self.socket.sink())`; nothing calls `register` | must change: after each `with_state_sink(socket.sink())`, call `socket.register(ws.<request fn>())` on the same `SocketControl` instance (a clone starts at generation 0 and its `register` does nothing). `Feed::run` logging `Reconnected` (175) stays |
| `src/factories/data.rs` | `create` (31) calls `QfexDataClient::new(ClientId::from(name), config)` (45); `LiveNodeBuilder` runs it inside the node's registry scope | no change: the scope is already around it |
| `tests/data_client.rs` | stand-in REST + MDS socket on `127.0.0.1` (`start_venue` 161, `Venue::serve` 63, `wait_for_requests` 113); `next_state` (463) and `socket_state_follows_the_market_data_socket` (479) capture `SystemEvent::SocketState` | reuse, do not rewrite: add one test that builds the client inside `SocketReconnectRegistry::default().scope(..)`, uses `next_state` and `wait_for_requests` |
| nautilus-live 0.67.0 src/socket.rs | `SocketControlFactory::new` (396) takes an owner from the current scope, which marks the client reconnect-capable; `SocketControl::sink_with` (547-571) advances the generation and drops the previous registration; `register` (574-598) only works for the instance's current generation; `SocketReconnectRegistry::scope` (121), `get` (137) | reuse, do not rewrite |
| nautilus-live 0.67.0 src/node/mod.rs | `request_socket_reconnect` (740-762) maps `SocketReconnectLookup` and `ReconnectRequestOutcome` to the dispatch outcome; `EndpointNotFound` becomes `UnknownEndpoint` | the outcome this work changes; read only |
| nautilus-live 0.67.0 src/node/builder.rs | builds every data client factory inside `socket_registry.scope(..)` (646) | why the control is reconnect-capable; read only |
| nautilus-network 0.67.0 src/websocket/client.rs | `WebSocketClient::reconnect_handle()` (3231) returns a cloneable `WebSocketReconnectHandle` (2554) whose `request_reconnect()` (2585) is sync and returns `ReconnectRequestOutcome` (mode.rs line 251), publishing `Disconnected` through the sink before the replacement connects | reuse, do not rewrite |
| `docs/external/nautilustrader/README.md` | lists what we use of `nautilus-live` and `nautilus-network`; "Reconnect registry side effect" names this issue | rows added in planning for `SocketControl::register`, `SocketReconnectRegistry` and `WebSocketReconnectHandle`; build rewrites the side-effect bullet once the handler is registered |
| `docs/STATUS.md` | test count line (31: 168 passed, 142 unit, 4 data client); socket state row (77) records the `UnknownEndpoint` side effect | must change: new counts, the row says what the reconnect command answers and names the new tests |
| `ARCHITECTURE.md` | Dataflow step 7 (52) says no socket publishes `SocketStateChanged` yet | must change: step 7 says the market data socket publishes its state and answers a controller reconnect command; the trade socket does neither |
| `docs/data-model.md` | invariants 8 and 9 on socket state (42-43) | the new test checks 8 also holds around a controller-requested reconnect; no text change needed |

Depends on one outside system: NautilusTrader 0.67.0, recorded in `docs/external/nautilustrader/README.md`. No call to QFEX beyond the local stand-in.

**This change has to reach** callers: `QfexDataClient::new`, `Feed::close`; `QfexExecutionClient` and `bin/trades_probe.rs` (54) use `new_trade` / `new_mds` with unchanged signatures and register nothing · tests: a new unit test in `src/websocket/client.rs`, a new test in `tests/data_client.rs` · fixtures: none · config: none · exports: one new public method on `QfexWebSocketClient`

## Prototype

- Question to answer: can a sync closure registered with `SocketControl::register`, reading a `parking_lot::Mutex<Option<WebSocketReconnectHandle>>` slot, drive a handler-mode QFEX-style `WebSocketClient` through a registry lookup, and what does the lookup answer before connect, after disconnect and after a second `sink()`?
- What was tried: a throwaway integration test in a scratch checkout of `origin/main` (axum socket on `127.0.0.1`, the same `WebSocketConfig` as `QfexWebSocketClient::connect`, `SocketControlFactory::new` inside `SocketReconnectRegistry::scope`, `replace_system_event_sender` to read states)
- Result: lookup is `EndpointNotFound` until the first `sink()` + `register`, then `Handle`; with an empty slot the handle answers `Closed`; while connected `Accepted`, then `Disconnected`, `Connected` on the sink and `RECONNECTED` reaches the message handler; an immediate second request answers `AlreadyReconnecting`; after `disconnect()` `Closed`; a new `sink()` turns the lookup back to `EndpointNotFound` and the old handle to `Closed` until `register` is called again, after which a request on the rebuilt socket is `Accepted` with `Disconnected`, `Connected`; once the control drops the lookup is `EndpointNotFound`. The plan above holds unchanged
- Code thrown away, never merged

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `QfexWebSocketClient` keeps its transport's `WebSocketReconnectHandle` in a shared slot (set in `connect()`, emptied in `disconnect()`) and exposes a request function over it; new unit test `mds_reconnect_request_follows_the_connection`; `docs/STATUS.md` counts updated. No caller uses it yet | `cargo test --lib reconnect_request` passes: `Closed` before connect, `Accepted` then a second `MockVenue` connection that receives the recorded subscriptions again, `Closed` after `disconnect()`; `cargo test` reports 169 passed | done |
| 2 | `QfexDataClient` registers the request function after each `socket.sink()` (in `new` and in `Feed::close`); new test `reconnect_command_reconnects_the_market_data_socket`; `docs/STATUS.md` row 77, `ARCHITECTURE.md` step 7 and the side-effect bullet of `docs/external/nautilustrader/README.md` rewritten | `cargo test --test data_client reconnect_command` passes; `cargo test` reports 170 passed, 0 failed | done |

## Acceptance criteria

1. Built inside a `SocketReconnectRegistry` scope, `registry.get(client_id, "market_data")` resolves to `SocketReconnectLookup::Handle` right after `QfexDataClient::new`, verified by `cargo test --test data_client reconnect_command`
2. Before `connect()` that handle answers `ReconnectRequestOutcome::Closed`, verified by the same test
3. After `connect()`, `request_reconnect()` answers `Accepted`, the system events for `(client_id, market_data)` are `Connected`, `Disconnected`, `Connected` in that order, and the stand-in sees the subscribe frames of the existing subscription a second time, verified by the same test (`next_state` and `wait_for_requests`)
4. After `disconnect()`, a fresh lookup still resolves to a handle and it answers `Closed` (or `Disconnected`), never `Accepted`, verified by the same test
5. After `disconnect()` and a second `connect()`, a fresh lookup's handle answers `Accepted` and `Disconnected`, `Connected` follow, verified by the same test
6. On the WebSocket client alone, the request function answers `Closed` before `connect()`, `Accepted` while connected (and `MockVenue` counts a second connection with the subscriptions sent again), `Closed` after `disconnect()`, verified by `cargo test --lib reconnect_request`
7. The trade socket registers nothing: `src/execution/mod.rs` has no call to `register` and no `SocketControl`, verified by `grep -n "SocketControl\|\.register(" src/execution/mod.rs` showing nothing
8. The 168 tests passing at `362bcb0` still pass, verified by `cargo test` showing 170 passed, 0 failed
9. No test opens a connection outside `127.0.0.1`, verified by `grep -n "qfex.com" tests/data_client.rs src/websocket/client.rs` showing nothing, as at `362bcb0`
10. `docs/STATUS.md` no longer says the reconnect command answers `UnknownEndpoint` and names the new tests and count, verified by `grep -n "UnknownEndpoint\|170 passed" docs/STATUS.md`

## Risks

- Forgetting the second `register` in `Feed::close` would make the command work once and answer `UnknownEndpoint` after the first disconnect and connect of the data client, silently; criteria 4 and 5 guard it
- Calling `register` on a clone of the `SocketControl` (for example by changing `Feed::socket` from `Arc<SocketControl>` to a plain clone) makes it a no-op, because a clone starts at generation 0; the test would catch it as `EndpointNotFound`
- `register` reached before `sink()` is a no-op too; order inside `Feed::close` and `new` matters
- The slot keeps a handle to a dead transport only between a transport `Closed` and the next `disconnect()`; the handle then answers `Closed` itself, so the answer stays honest
- An accepted reconnect publishes `Disconnected` from the thread that runs the command (the node's), not from the transport task; ordering with a concurrent transport loss is handled by `nautilus-network` (`AlreadyReconnecting`), as the prototype saw
- The 2 s gap the prototype needed is the transport's reconnect delay (250 ms initial plus jitter); a slow CI box could make the test flaky if it waited a fixed time, so it waits on events with the 10 s timeout `next_state` already uses

## Assumptions

- The test resolves the handle through `SocketReconnectRegistry::get` rather than sending a `ReconnectSocket` command through a full `LiveNode`, because `LiveNode::request_socket_reconnect` is a direct mapping of that lookup (`node/mod.rs` 740-762) and a node in this test would add a lot for no extra coverage of this crate
- Before `connect()` and after `disconnect()` the answer is `Closed`, because there is no transport to ask and `Closed` is what `nautilus-live` itself answers for a deactivated handle; restate allows `Disconnected` or `Closed`
- The registration stays in place across `disconnect()` (slot emptied, handle answers `Closed`) instead of being removed, because restate asks for an honest answer from the transport, not `UnknownEndpoint`
- `parking_lot::Mutex` for the slot, because the request function is sync, `parking_lot` is already a dependency and the lock is held only to clone or read the handle
- `nautilus-lighter` and `nautilus-risex` sources are not on this machine, so no file was followed from them; the plan follows the `nautilus-live` and `nautilus-network` 0.67.0 APIs read above
- Test count baseline is 168 at `362bcb0`, as `docs/STATUS.md` on `origin/main` says
