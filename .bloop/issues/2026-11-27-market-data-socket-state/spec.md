REVIEWER: code
MODEL: default
APPROVED: no

# Market data socket publishes its connection state

## Summary

- The data client takes one `SocketControl` for endpoint `market_data` from `SocketControlFactory::new(client_id, Some(QFEX))` and hands its `sink()` to the market data `QfexWebSocketClient`, which passes it to `WebSocketClient::builder().state_sink(..)`
- `nautilus-network` then reports `Connected` on connect and after each reconnect, and `Disconnected` when the transport is lost; a deliberate `disconnect()` stays silent
- `close()` builds a fresh `QfexWebSocketClient` with a fresh sink from the same control, so state keeps flowing after a disconnect and connect of the data client
- Proven by a new test in `tests/data_client.rs` that captures `SystemEvent::SocketState` while the local stand-in drops the socket once
- Two slices: the WebSocket client accepts a sink (no behaviour change), then the data client wires it and the test lands

## Context

| path | what it does today | how this work relates |
| --- | --- | --- |
| `src/websocket/client.rs` | `QfexWebSocketClient::new_mds` / `new_trade` store the URL and session; `connect()` builds `WebSocketClient::builder().config(..).message_handler(..).connect()` | must change: hold an `Option<SocketStateSink>`, set by a new `with_state_sink(sink)`, and pass it to the builder's `state_sink`; `new_mds(url)` and `new_trade` keep their signatures and default to `None` |
| `src/data/mod.rs` | `QfexDataClient::new` builds `Feed` with `QfexWebSocketClient::new_mds(ws_url)` (line 306); `Feed::close` replaces it with another `new_mds` (line 270); `Feed::run` logs `QfexWsMessage::Reconnected` (line 170) | must change: build the `SocketControl` in `new`, keep it in `Feed`, give `new_mds(..).with_state_sink(control.sink())` in both places; the log line stays |
| `tests/data_client.rs` | stand-in REST + MDS socket on `127.0.0.1` (`start_venue`, `Venue::serve`); `Venue::drop_connection` cuts the socket; `replace_data_event_sender` captures data events | reuse, do not rewrite: add one test that also calls `replace_system_event_sender` before `QfexDataClient::new` and uses `drop_connection` |
| `nautilus-live` 0.67.0 `src/socket.rs` | `SocketControlFactory::new(client_id, venue)` reads the thread-local system event sender at construction; `control(endpoint).sink()` returns a `SocketStateSink`; a new `sink()` from the same control retires the previous one | reuse, do not rewrite |
| `nautilus-network` 0.67.0 `src/websocket/client.rs` | `WebSocketClient::builder` takes `state_sink: Option<SocketStateSink>` | reuse, do not rewrite |
| `nautilus-common` 0.67.0 `src/live/runner.rs`, `src/messages/mod.rs` | `replace_system_event_sender` (thread-local); `SystemEvent::SocketState(SocketStateChange)` | reuse in the test |
| `docs/STATUS.md` | Verification table with test counts (says 162; HEAD counts 166) | must change: new test, new count, a line on socket state |
| `docs/data-model.md` | invariants 8 and 9 on socket state | the test checks them |

**This change has to reach** callers: `QfexDataClient::new`, `Feed::close`, `QfexExecutionClient` (calls `new_trade`, unchanged signature) · tests: `tests/data_client.rs` (new test), existing unit tests of `websocket::client` · fixtures: none · config: none · exports: `QfexWebSocketClient::with_state_sink` is new; `new_mds(url)` keeps its signature, so `bin/trades_probe.rs:54` and the five unit tests in `src/websocket/client.rs` that call it need no change

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `QfexWebSocketClient` holds an optional `SocketStateSink`, set by `with_state_sink`, and passes it to `WebSocketClient::builder().state_sink(..)`; without it nothing changes | `cargo test --lib websocket` passes and `cargo build --bins --examples --features examples` builds | todo |
| 2 | `QfexDataClient` builds a `SocketControl` for `market_data` / `QFEX`, passes `sink()` in `new` and in `Feed::close`; new test `socket_state_follows_the_market_data_socket`; `docs/STATUS.md` updated | `cargo test --test data_client socket_state` passes and shows `Connected`, `Disconnected`, `Connected`; `cargo test` reports 167 passed | todo |

## Acceptance criteria

1. After `connect()`, a `SystemEvent::SocketState` with state `Connected`, `client_id` of the data client, venue `QFEX`, endpoint `market_data` is sent, verified by `cargo test --test data_client socket_state`
2. When the stand-in drops the socket, `Disconnected` then `Connected` follow for the same `(client_id, endpoint)`, verified by the same test
3. After `disconnect()` and a second `connect()`, `Connected` is sent again (the sink survives `close()`), verified by the same test
4. A deliberate `disconnect()` sends no `Disconnected`, verified by the same test seeing no further socket event within 500 ms after `disconnect()`
5. The trade socket publishes nothing new: `QfexWebSocketClient::new_trade` takes no sink, verified by reading `src/websocket/client.rs`
6. The 166 tests that pass at `4f4ed16` still pass, verified by `cargo test` showing 167 passed, 0 failed
7. No test opens a connection outside `127.0.0.1`, verified by `grep -n "qfex.com" tests/data_client.rs` showing only what is there today

## Risks

- A sink lost in `Feed::close` would make events stop after the first data client disconnect, silently; criterion 3 guards it
- `SocketControlFactory` reads the system event sender when it is built. If a node built `QfexDataClient` on a thread without that sender, nothing would be published (no crash). The factory runs where `nautilus-lighter` builds its own control, so this matches upstream
- Keeping the old sink alive next to a new one could publish twice; taking every sink from one `SocketControl` retires the old one (generation check in `nautilus-live`)
- The stand-in must keep reading so axum answers the client's pings; `Venue::serve` already loops on `recv()`. If it stopped reading, the 20 s heartbeat would see a dead peer and the test would flake

## Assumptions

- The test checks `SystemEvent::SocketState` from the data client, not `SocketStateChanged` delivered to a real actor, because the node turns one into the other inside NautilusTrader and a full `LiveNode` in this test would add a lot for no extra coverage of this crate; mocha's prototype already showed the actor side
- `nautilus-lighter` 0.67.0's data client is the pattern named in the issue; its source is not vendored here, so the plan follows the `nautilus-live` and `nautilus-network` APIs read above, which that pattern uses
- The test count baseline is 166 (counted at `4f4ed16`), not 162 as the request said, because `docs/STATUS.md` predates `0e8ed08`
- The endpoint label is the string `market_data`, as the request says
