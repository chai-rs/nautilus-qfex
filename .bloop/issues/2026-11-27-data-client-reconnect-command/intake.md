DECISION: ACCEPT
TYPE: feat
FR: FR-7
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- Under a `LiveNode`, a controller reconnect command for the QFEX data client's `market_data` endpoint answers `UnknownEndpoint` today: the client builds a `SocketControl` inside the node's registry scope but never calls `SocketControl::register`
- `issue.md` leaves the choice to restate: register a reconnect handler, or go back to answering `Unsupported`. We take the first reading: register the handler, so the command actually reconnects the socket
- NautilusTrader 0.67.0 already has the parts: `SocketControl::register` in `nautilus-live` and `WebSocketClient::request_reconnect` returning a `ReconnectRequestOutcome` in `nautilus-network`
- One pull request: the data client and its market data socket only, plus a test built inside a registry scope and the `docs/STATUS.md` row
- The trade socket stays out. It has its own issue, `2026-11-27-trade-socket-state`

## Why

Registering serves FR-7 (reconnect and resubscribe) and fits the endpoint the client already publishes `SocketStateChanged` for. Going back to `Unsupported` would mean building the factory outside the registry scope that `LiveNodeBuilder` wraps around every data client factory (`nautilus-live-0.67.0/src/node/builder.rs:646`). That gives up a capability the framework offers and only papers over the wrong answer. On `origin/main` (`362bcb0`), `src/data/mod.rs:309-316` creates one shared `Arc<SocketControl>`, and `Feed::close` (`:271-275`) takes a fresh `sink()` for every rebuilt `QfexWebSocketClient`. In `nautilus-live-0.67.0/src/socket.rs:547-598`, each `sink()` drops the previous registration, and `register` only works for the current generation. So the handler has to be registered again after every `close()`, and it has to reach the live transport inside `QfexWebSocketClient` (`src/websocket/client.rs:343-349`) from a sync closure. That ordering is why risk is medium rather than low. It does not reach high: no order, money or venue state is involved, and the market data socket can be tested against the local mock server.
