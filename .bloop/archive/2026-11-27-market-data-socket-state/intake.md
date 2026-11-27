DECISION: ACCEPT
TYPE: feat
FR: FR-8
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- Accepted as one pull request: the market data socket publishes `SocketStateChanged` for endpoint `market_data`, venue `QFEX`
- Small change (prototype: about 20 lines in two files), with a proven reference in `nautilus-lighter` 0.67.0
- Risk is medium, not low: `QfexWebSocketClient` is shared by the market data and trade sockets, and `close()` rebuilds the client, so a sink lost there would silently stop events after the first disconnect
- The trade socket is not part of this issue
- Test count baseline is 166, not the 162 in the request

## Why

`src/data/mod.rs:170` only logs `QfexWsMessage::Reconnected`, and nothing in the crate uses `SocketControlFactory`, so an actor on `subscribe_socket_state` sees nothing today. The data client builds its `QfexWebSocketClient::new_mds` in `new` (`src/data/mod.rs:306`) and again in `close()` (`src/data/mod.rs:270`), which matches the two places the request names. `nautilus-live` 0.67.0 provides `SocketControlFactory::control(endpoint).sink()`, so no new dependency is needed.
