STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `3b5f35b` | `cargo test --test exec_client socket_state`, then `./checks.sh` | red first (`no socket state within 10 s`). Then 5 of 30 runs red at the reconnect: the stand-in's `notify_waiters` drop is lost while it answers a frame; after retrying the drop until `Disconnected` arrives, 30 of 30 green. Gate: 171 passed, 0 failed, `171 passed, as written` | `evidence/slice-1.md` (PASS) |
| 2 | `21c6b9e` | `git grep -n "publishes no socket state\|publishes nothing yet" -- docs/STATUS.md ARCHITECTURE.md`; `git grep -n socket_state_follows_the_trade_socket -- docs/STATUS.md`; `./checks.sh --e2e` | red first (two lines found); then none found (exit 1) and the socket state row names the test; gate exit 0, 171 passed | `evidence/slice-2.md` (PASS) |

## Acceptance criteria

- [x] 1 `Connected` after `connect()` with the client's id, venue `QFEX`, endpoint `trade` — `next_state` asserts all three; `evidence/slice-1.md`
- [x] 2 `Disconnected` then `Connected` after the drop, and a second `auth` reaches the stand-in — same test (the `Disconnected` is read in the retry loop, its client id and endpoint are not asserted there; the `Connected` after it is)
- [x] 3 `Connected` again after `disconnect()` and a second `connect()` — same test
- [x] 4 no event within 500 ms after a deliberate `disconnect()` — same test
- [x] 5 built inside a `SocketReconnectRegistry` scope, `get(client_id, "trade")` and `get(client_id, "market_data")` answer `ClientNotFound` — same test
- [x] 6 the 170 tests of `4b46b86` still pass — 171 passed, 0 failed; both count rows say 171 (`checks.sh` step `test count`)
- [x] 7 `docs/data-model.md` names `trade` in its relationship line and invariant 9 — commit `3b5f35b`
- [x] 8 no "publishes no socket state" / "publishes nothing yet" left; the socket state row names the new test and `Unsupported`; `ARCHITECTURE.md` step 7 names both sockets — slice 2 check
- [x] 9 no test leaves `127.0.0.1` — `git grep -n "qfex.com" -- tests/exec_client.rs` prints nothing at `21c6b9e`

## Notes for reviewers

- Worth a closer look: the retry loop around `drop_connection.notify_waiters()` in the new test. It works around a race in the stand-in (`tokio::sync::Notify::notify_waiters` wakes only tasks already waiting), not in the adapter. The existing `reconnect_logs_in_again_and_catches_up` uses the same drop without a retry and may carry the same race
- Worth a closer look: the control is built in `connect()` on purpose (spec Context): built in `new`, inside the node's registry scope, it would mark the client reconnect-capable with no handler and turn `Unsupported` into `UnknownEndpoint`
- Records: this issue's folder is committed on `chore/2026-11-27-retro-status-test-count-check` with the other records of this run, not on the issue branch: bloop writes issue files in the main checkout and `bin/job` reads them there, which is the conflict `2026-11-27-post-merge-records-route` is open for
- `clean-comment` ran inline (two doc comments, two "why" lines in the test)
- `MUST KILL` flags: none. `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/8 (draft)
