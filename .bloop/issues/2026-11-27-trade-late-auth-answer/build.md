STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `2fa1809` | `cargo test --lib trade_late_auth_answer`; mutation with `send_text` put back; `repro/loop-a.txt` on the whole `exec_client` binary, 90 runs; `./checks.sh` | the moved check red first (socket 2 got `["subscribe", "auth", "subscribe"]`), then 20 of 20 green; 3 of 3 red with `send_text` back; loop-a 0 of 90 red on this commit against 10 of 90 on `main` `92dca56` (all `subscribe before auth`); gate 172 passed, as written | `evidence/slice-1.md` (PASS) · `evidence/slice-1-after-clean-comment.md` (PASS) |
| 2 | `0e057ce` | `cargo test --lib trade_stale_reconnect`; mutation with `start_session` back on `send_text`; `cargo test --lib trade_` 20 runs; `./checks.sh` | red first (socket 3 got `["auth", "auth", "subscribe", "subscribe"]`), then green; 3 of 3 red with `send_text` back; all 6 `trade_` unit tests 20 of 20; gate 173 passed, as written | `evidence/slice-2.md` (PASS) |
| 3 | `9cea260` | `git grep -n "Known race, not fixed" -- docs/STATUS.md`; `git grep -n trade_stale_reconnect -- docs/STATUS.md`; `git grep -n 'wait_frames("subscribe", 1)' -- tests/exec_client.rs`; `./checks.sh --e2e` | red first (the row was found); then not found (exit 1), the row names both checks, the wait is still at `tests/exec_client.rs:1012`; gate e2e exit 0 | `evidence/slice-3.md` (PASS) |

## Acceptance criteria

- [x] 1 the saved check is red at `main` (debug.md, 20 of 20) and green on the branch (20 of 20), red again with `send_text` back (3 of 3) — slice 1 row
- [x] 2 the moved check asserts nothing reached the stream while the gate's lock was held (`rx.try_recv().is_err()` with message "the read loop was not held at the subscribed frame") — `src/websocket/client.rs`, slice 1
- [x] 3 after a stale reconnect notice the live socket receives exactly `["auth", "subscribe"]` — `trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket`, red with `start_session` back on `send_text`
- [x] 4 the reported shape no longer shows: 0 of 90 runs print `subscribe before auth` (10 of 90 on unfixed `main`) — slice 1 row; run by hand in a throwaway worktree, not kept as a file
- [x] 5 `socket_state_follows_the_trade_socket` still waits for the first `subscribe` — `tests/exec_client.rs:1012`
- [x] 6 login, subscribe and cancel on disconnect on the first connection and after a reconnect — `trade_socket_authenticates_then_starts_the_session_again_after_reconnect` and `reconnect_logs_in_again_and_catches_up` pass unchanged (gate)
- [x] 7 the market data socket is unchanged — the `mds_` unit tests and `tests/data_client.rs` pass unchanged (gate); `start_session`'s market data branch still uses `send_json`
- [x] 8 the trade client row no longer lists the race and says orders still use the ordinary send — slice 3
- [x] 9 both count rows say 173 passed, 145 unit — `checks.sh` step `test count`, `evidence/slice-3.md`
- [x] 10 `docs/external/nautilustrader/README.md` lists the connection-bound API — written in box 3 (`external-contract`), on `main` with the records PR of this run
- [x] 11 no test leaves `127.0.0.1` — `git grep -n "qfex.com" -- src/websocket/client.rs tests/exec_client.rs` prints nothing at `9cea260`

## Notes for reviewers

- Worth a closer look: `start_session` now returns `Ok(())` in three cases (auth sent, connection already replaced before sending, `SendError::ConnectionChanged` while sending); the last two log a warning. `comment-cleanser` flagged that the code did not say the hand-off is on purpose; the log line now says it. A return type that names the cases was the alternative, judged larger than needed
- Worth a closer look: the first `auth` in `connect` treats `ConnectionChanged` as handed to the next connection, which then logs in from its own `RECONNECTED`; `logged_in` in the execution client keeps waiting for the first `Authenticated`, as before
- Left out on purpose (restate): orders sent with `send` during a reconnect still use `send_text` and can be replayed before the next `auth`. Not opened as an issue yet
- Deviation from `spec.md` slice 3: the comment above `wait_frames("subscribe", 1)` was deleted instead of rewritten to name the unit checks; it survived clean-comment only as a link to this open issue (exception 5), and a comment naming tests matches no exception
- `clean-comment`: `comment-cleanser` ran on slice 1 (3 deleted, 1 kept under exception 2, 2 flags cleared by renaming `late_auth` to `defer_first_auth_then_drop` and `held` to `read_loop_held_at_subscribe_gate`) and on slice 2 (2 deleted, 1 flag cleared by a log line)
- `MUST KILL` flags left: none. `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/16 (draft)
