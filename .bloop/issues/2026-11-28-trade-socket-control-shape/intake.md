DECISION: ACCEPT
TYPE: refactor
FR: none
RISK: low
SURFACES: none
RESCOPED: no

## Summary

- `issue.md` leaves restate a choice: register a reconnect handler for the trade socket, or keep the `Unsupported` answer and give the control a shape that says so. We take the second reading
- Today `QfexExecutionClient` keeps a plain `socket: Option<SocketControl>` and fills it with `get_or_insert_with` in `connect()`. That timing alone keeps the control out of the node's registry scope. Nothing in a name or a type says so since `7a7d27d` deleted the comments that did
- Behaviour stays the same: under a `LiveNode` the reconnect command for the execution client answers `Unsupported`, and the trade socket publishes `SocketStateChanged` exactly as it does now
- Left out: a reconnect handler for the trade socket. It would change the order socket and the trade login path, which unmerged PR #16 is still changing, and its review waits on a person
- One pull request: the execution client only, plus doc lines that name the control, if a name they cite changes

## Why

`issue.md` sets the goal: the code says by itself why the trade socket's reconnect command answers `Unsupported`. That goal assumes the answer stays. In `nautilus-live` 0.67.0, `LiveNodeBuilder` builds every execution client factory inside `socket_registry.scope` (`node/builder.rs:701`). `SocketControlFactory::new` takes the registrar from that scope (`socket.rs:396-399`), and `SocketReconnectRegistrar::owner` marks the client as supported (`socket.rs:191-198`). A control built in `QfexExecutionClient::new` without a registered handler would therefore answer `EndpointNotFound`. It answers `Unsupported` only because `src/execution/mod.rs:760-762` builds it in `connect()`, outside the scope. So the fact sits in where the call happens, not in a name, and restructuring that is a contained refactor. `socket_state_follows_the_trade_socket` (`tests/exec_client.rs:1004-1037`) already pins it: it builds the client inside a registry scope and expects `ClientNotFound` for `trade` and `market_data`.

The other reading, a reconnect handler that forces a re-login and catch-up on command, follows the precedent of `2026-11-27-data-client-reconnect-command`, and `WebSocketClient::reconnect_request` (`src/websocket/client.rs:393-403`) already exists for it. But it changes behaviour on the order socket, and it lands in the login path that PR #16 (`origin/fix/2026-11-27-trade-late-auth-answer`) is rewriting. That PR is escalated to a person. Its `docs/STATUS.md` row says orders sent with `send` during a reconnect can still be replayed on the next socket before its `auth`, and its `decision.md` asks for a separate issue on that order path. A command that triggers a reconnect would make that replay reachable on demand. It is worth its own issue, with risk high, once PR #16 and the order-path issue are settled. The refactor touches neither `src/websocket/client.rs` nor the login, so it does not depend on PR #16. That keeps risk low.
