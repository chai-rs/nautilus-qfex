STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `539a475` | `cargo test --lib reconnect_request`, then `./checks.sh` | red first (`no method named reconnect_request`), then green 10 of 10; gate failed once on clippy (`must_use` on a function returning a closure), passed on rerun: 169 passed, 0 failed per run | `evidence/slice-1.md` (FAIL, lint) · `evidence/slice-1-rerun.md` (PASS) |
| 2 | `691abc8` | `cargo test --test data_client reconnect_command`, then `./checks.sh --e2e` | red first (`market_data has a reconnect handle` panicked: lookup found none), then green 10 of 10; gate: 170 passed, 0 failed per run (143 unit, 8, 2, 5 data client, 12 exec client) | `evidence/slice-2.md` (PASS) |

## Acceptance criteria

- [x] 1 inside a registry scope, `market_data` resolves to a handle right after `new` — `evidence/slice-2.md`, test `reconnect_command_reconnects_the_market_data_socket` (`registry.handle(..).expect(..)`)
- [x] 2 `Closed` before `connect()` — same test
- [x] 3 `Accepted`, then `Connected`, `Disconnected`, `Connected`, and the subscribe frames sent again — same test (`next_state`, `wait_for_requests`)
- [x] 4 after `disconnect()` a fresh lookup answers anything but `Accepted` — same test (`assert_ne!(.., Accepted)`)
- [x] 5 after `disconnect()` and a second `connect()`, `Accepted` and `Disconnected`, `Connected` — same test. Mutation run: with `Feed::close` building the socket without `register`, the test panics at the handle lookup (run by hand, not kept as a file)
- [x] 6 the WebSocket client alone: `Closed`, `Accepted` with a second connection and the subscription sent again, `Closed` — `evidence/slice-1-rerun.md`, test `mds_reconnect_request_follows_the_connection`
- [x] 7 the trade socket registers nothing — `grep -n "SocketControl\|\.register(" src/execution/mod.rs` prints nothing (exit 1) at `691abc8`
- [x] 8 the 168 tests of `362bcb0` still pass — `evidence/slice-2.md`: 170 passed, 0 failed
- [x] 9 no test leaves `127.0.0.1` — `grep -n "qfex.com" tests/data_client.rs src/websocket/client.rs` prints nothing (exit 1) at `691abc8`
- [x] 10 `docs/STATUS.md` drops `UnknownEndpoint` and names the new tests and 170 — `grep -c UnknownEndpoint docs/STATUS.md` is 0 at `691abc8`; row 77 and lines 31-32 updated

## Notes for reviewers

- Worth a closer look: `market_data_socket` in `src/data/mod.rs` pairs every `sink()` with a `register` on the same `SocketControl`, because a new sink retires the previous registration. Both `new` and `Feed::close` go through it
- Worth a closer look: the reconnect slot is a `parking_lot::Mutex` read synchronously by the registered function; it is filled after `start_session` succeeds in `connect()` and emptied first thing in `disconnect()`
- Deferred, on purpose: `spec.md` slice 2 also rewrites `ARCHITECTURE.md` dataflow step 7 and the "reconnect registry side effect" bullet in `docs/external/nautilustrader/README.md`. Their current text exists only on `claude/bloop-init` (PR #4); changing them on this branch would conflict with PR #4. They are rewritten on this branch after PR #4 merges, before this one merges
- `clean-comment` ran inline (three doc comments on new items, one "why" line in the test), not through the `comment-cleanser` agent
- `MUST KILL` flags: none. `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/5 (draft)
