# Build 2026-11-28-trade-socket-control-shape

## Summary

- New private module `src/execution/trade_socket.rs` holds `StateOnlyTradeSocketControl`: a private `control: Option<SocketControl>`, a `const fn built_on_first_connect()` and a `sink(client_id)` that builds the control on its first call (`SocketControlFactory::new(client_id, Some(*QFEX_VENUE)).control("trade")`) and returns its `SocketStateSink`. Nothing outside the module can reach the control, so nothing in the execution client can call `register` on it
- `QfexExecutionClient` keeps `trade_socket_control: StateOnlyTradeSocketControl`, built in `new` with `StateOnlyTradeSocketControl::built_on_first_connect()`; `connect()` hands `self.trade_socket_control.sink(client_id)` to `QfexWebSocketClient::new_trade(...).with_state_sink(...)`. `mod.rs` no longer imports `SocketControl`, `SocketControlFactory` or `QFEX_VENUE`, and `TRADE_ENDPOINT` moved into the new module
- Behaviour is unchanged: the control is still built on the first `connect()`, outside the node's registry scope, so a reconnect command for `trade` still answers `Unsupported`. No test, doc or count changes
STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `b9d820e` | `cargo test --test exec_client socket_state_follows_the_trade_socket` 10 runs; `grep -nw -e SocketControl -e SocketControlFactory -e register src/execution/mod.rs`; `grep -n -e register -e // src/execution/trade_socket.rs`; `git diff origin/main -- tests docs README.md ARCHITECTURE.md src/websocket/client.rs`; `bin/gate --e2e` (`./checks.sh`) | 10 of 10 green; both greps print nothing (exit 1); the diff is empty; gate PASS, `cargo test` and `cargo test --features examples` 171 passed, as written | `evidence/slice-1.md` (PASS) |

A refactor has no red-first check: the spec names `socket_state_follows_the_trade_socket`, left as it is, as the check that pins the behaviour. Its prototype in box 3 showed it fails as soon as the control is built in `new` inside a registry scope.

## Acceptance criteria

- [x] 1 `grep -n trade_socket_control src/execution/mod.rs` shows the field, the `built_on_first_connect()` call in `new` and the `sink(client_id)` call in `connect()`
- [x] 2 `src/execution/mod.rs` names neither `SocketControl`, `SocketControlFactory` nor `register` as a word (grep empty); `control` has no `pub` in `trade_socket.rs`
- [x] 3 `trade_socket.rs` has no `register` and no `//` (grep empty)
- [x] 4 the diff adds no `//` line to `src/execution/mod.rs`
- [x] 5 `socket_state_follows_the_trade_socket` passes 10 of 10 with `tests/exec_client.rs` unchanged

## Notes for reviewers

- Worth a closer look: `sink` is `&mut self` and uses `get_or_insert_with`, so a second `connect()` takes a new sink from the same `SocketControl`, as before the change (data-model invariant 8 for the `trade` endpoint)
- `clean-comment`: `comment-cleanser` ran on the diff `origin/main...HEAD`: 0 deleted, no `MUST KILL` flag (the diff adds no comment). It ran after the commit rather than before; it found nothing, so the commit is unchanged
- `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/17 (draft)
