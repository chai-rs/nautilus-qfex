DECISION: ACCEPT
TYPE: feat
FR: FR-8
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- Accepted as one pull request: the trade socket publishes `SocketStateChanged` (venue `QFEX`, a trade endpoint label chosen in planning), the way the market data socket already does
- The open question in `issue.md` is answered from the framework, not by a guess: `Connected` means the transport is up, not "up and logged in". NautilusTrader 0.67.0 documents `SocketState::Connected` as "The transport is available", and the transport itself emits it
- The reconnect-registry side effect is handled the way `2026-11-27-market-data-socket-state` handled it: recorded in `docs/STATUS.md` with a follow-up, not fixed here
- Risk is medium: it touches the execution client's connect and reconnect path, which also drives the login and the catch-up, but it changes no order, fill or login behaviour
- Test count baseline on `origin/main` is 168

## Why

On `origin/main` (`362bcb0`), `src/execution/mod.rs:444-450` only logs "QFEX trade socket reconnected" and resets the login state, and `QfexExecutionClient::connect` (`:750`) builds `QfexWebSocketClient::new_trade` with no sink, while `with_state_sink` (`src/websocket/client.rs:383`) already exists and is used by the data client (`src/data/mod.rs:309-316`). `nautilus-network-0.67.0/src/sink.rs:163-170` has only `Connected` ("The transport is available") and `Disconnected`, and the transport publishes them from inside `WebSocketClient`; holding `Connected` back until `auth` succeeds would mean bending a framework state into a session state the market data socket does not use. Login after a reconnect stays visible as it is today, through the log and the client stopping on a refused login. `LiveNodeBuilder` builds execution clients inside the socket registry scope too (`nautilus-live-0.67.0/src/node/builder.rs:701`), so the same `UnknownEndpoint` side effect appears if the control is built in `QfexExecutionClient::new` (inside that scope) rather than in `connect` (outside it, `nautilus-live-0.67.0/src/socket.rs:137-157`); planning picks the place and `docs/STATUS.md` records the outcome. Registering a reconnect handler for the trade socket means a re-login and a catch-up on command, which deserves its own issue once `2026-11-27-data-client-reconnect-command` settles the pattern. It does not reach high: no order, money or venue state changes, and the mock in `tests/exec_client.rs` covers it offline.
