STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `2fa1809` | `cargo test --lib trade_late_auth_answer`; mutation with `send_text` put back; `repro/loop-a.txt` on the whole `exec_client` binary, 90 runs; `./checks.sh` | the moved check red first (socket 2 got `["subscribe", "auth", "subscribe"]`), then 20 of 20 green; 3 of 3 red with `send_text` back; loop-a 0 of 90 red on this commit against 10 of 90 on `main` `92dca56` (all `subscribe before auth`); gate 172 passed, as written | `evidence/slice-1.md` (PASS) · `evidence/slice-1-after-clean-comment.md` (PASS) |
| 2 | `0e057ce` | `cargo test --lib trade_stale_reconnect`; mutation with `start_session` back on `send_text`; `cargo test --lib trade_` 20 runs; `./checks.sh` | red first (socket 3 got `["auth", "auth", "subscribe", "subscribe"]`), then green; 3 of 3 red with `send_text` back; all 6 `trade_` unit tests 20 of 20; gate 173 passed, as written | `evidence/slice-2.md` (PASS) |
| 3 | `9cea260` | `git grep -n "Known race, not fixed" -- docs/STATUS.md`; `git grep -n trade_stale_reconnect -- docs/STATUS.md`; `git grep -n 'wait_frames("subscribe", 1)' -- tests/exec_client.rs`; `./checks.sh --e2e` | red first (the row was found); then not found (exit 1), the row names both checks, the wait is still at `tests/exec_client.rs:1012`; gate e2e exit 0 | `evidence/slice-3.md` (PASS) |

## Round 2

Sent back by the user on the approver's escalation point 1 (`review/round-1/decision.md`): prove the `SendError::ConnectionChanged` branch of `start_session` by running it, not by reading `nautilus-network`.

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| r2 | `575c901` | `cargo test --lib trade_auth_for_a_connection_replaced` 20 runs; mutation M1 (the `ConnectionChanged` arm removed, so the error propagates); mutation M2 (`send_text` back in `start_session`); `bin/gate`; `bin/gate --e2e` on the commit | 20 of 20 green; M1 red at the `is_ok` assertion ("a replaced connection is handed to the next one"); M2 red, socket 2 got `["auth", "auth", "subscribe", "subscribe"]`; gate PASS, 174 passed, as written (146 unit) | `evidence/round-2.md` (PASS) · `evidence/round-2-e2e.md` (PASS) |

How the check reaches the branch: `MockVenue` gains `next_upgrade_gate`, a lock a new connection's upgrade waits on. The test holds it, drops the trade socket, waits until the client is not active with `connection_epoch() == 0`, then spawns `start_session(.., 0)`. The epoch check at `client.rs:599` passes; `send_text_on_connection` waits in `wait_for_active`; the test asserts the call is still pending after 200 ms, then releases the gate. The writer moves to epoch 1 and answers `ConnectionChanged` for the epoch-0 frame (`nautilus-network` 0.67.0 `websocket/client.rs:2107`, `:2125-2128`, `:2177-2184`). The read loop then logs in on epoch 1 from its own `RECONNECTED`.

## Why the gate missed this

- (escalated by the approver, not a lens `[must]`) the `ConnectionChanged` login branch had no test: round 1 added checks for a late `auth` answer and a stale reconnect notice, both of which leave the epoch check at `:599` to catch the replaced connection, so no test ever got past that check with a connection about to be replaced. Step taken: **a test**, `trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in`, red under both mutations above. A machine can now catch a change that drops the arm or goes back to `send_text`

## Acceptance criteria

- [x] 1 the saved check is red at `main` (debug.md, 20 of 20) and green on the branch (20 of 20), red again with `send_text` back (3 of 3) — slice 1 row
- [x] 2 the moved check asserts nothing reached the stream while the gate's lock was held (`rx.try_recv().is_err()` with message "the read loop was not held at the subscribed frame") — `src/websocket/client.rs`, slice 1
- [x] 3 after a stale reconnect notice the live socket receives exactly `["auth", "subscribe"]` — `trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket`, red with `start_session` back on `send_text`
- [x] 4 the reported shape no longer shows: 0 of 90 runs print `subscribe before auth` (10 of 90 on unfixed `main`) — slice 1 row; run by hand in a throwaway worktree, not kept as a file
- [x] 5 `socket_state_follows_the_trade_socket` still waits for the first `subscribe` — `tests/exec_client.rs:1012`
- [x] 6 login, subscribe and cancel on disconnect on the first connection and after a reconnect — `trade_socket_authenticates_then_starts_the_session_again_after_reconnect` and `reconnect_logs_in_again_and_catches_up` pass unchanged (gate)
- [x] 7 the market data socket is unchanged — the `mds_` unit tests and `tests/data_client.rs` pass unchanged (gate); `start_session`'s market data branch still uses `send_json`
- [x] 8 the trade client row no longer lists the race and says orders still use the ordinary send — slice 3
- [x] 9 both count rows say 173 passed, 145 unit — `checks.sh` step `test count`, `evidence/slice-3.md`; 174 passed, 146 unit after round 2 — `evidence/round-2-e2e.md`
- [x] 10 `docs/external/nautilustrader/README.md` lists the connection-bound API — written in box 3 (`external-contract`), on `main` with the records PR of this run
- [x] 11 no test leaves `127.0.0.1` — `git grep -n "qfex.com" -- src/websocket/client.rs tests/exec_client.rs` prints nothing at `9cea260`

## Notes for reviewers

- Worth a closer look: `start_session` now returns `Ok(())` in three cases (auth sent, connection already replaced before sending, `SendError::ConnectionChanged` while sending); the last two log a warning. `comment-cleanser` flagged that the code did not say the hand-off is on purpose; the log line now says it. A return type that names the cases was the alternative, judged larger than needed
- Worth a closer look: the first `auth` in `connect` treats `ConnectionChanged` as handed to the next connection, which then logs in from its own `RECONNECTED`; `logged_in` in the execution client keeps waiting for the first `Authenticated`, as before
- Left out on purpose (restate): orders sent with `send` during a reconnect still use `send_text` and can be replayed before the next `auth`. Opened as `2026-11-28-trade-orders-before-live-login` on the user's decision, with the stale `Authenticated` from security finding 1
- Deviation from `spec.md` slice 3: the comment above `wait_frames("subscribe", 1)` was deleted instead of rewritten to name the unit checks; it survived clean-comment only as a link to this open issue (exception 5), and a comment naming tests matches no exception
- `clean-comment`: `comment-cleanser` ran on slice 1 (3 deleted, 1 kept under exception 2, 2 flags cleared by renaming `late_auth` to `defer_first_auth_then_drop` and `held` to `read_loop_held_at_subscribe_gate`) and on slice 2 (2 deleted, 1 flag cleared by a log line)
- `MUST KILL` flags left: none. `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/16 (draft)
- Round 2: `comment-cleanser` ran on the round 2 diff: 1 deleted (a doc comment on the mock's new field), 1 `MUST KILL` flag cleared by renaming `upgrades_held` to `next_upgrade_gate`. The 20 runs were before that rename; both gates ran after it
- Round 2, worth a closer look: the test reaches `start_session` and the session's private fields directly from the test module, as the round 1 tests do with `session.gate`; it relies on `wait_for_active` holding the frame until the gate is released, which the `is_finished()` assertion checks
