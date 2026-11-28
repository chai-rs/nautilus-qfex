ESCALATE
APPROVED-BY: none

## Reviewers

- code: PASS (`review/round-1/review-code.md`)
- security: PASS, with one `[should]` finding left to the order path issue (`review/round-1/review-security.md`, finding 1)
- ux: not run, no ui surface
- ui: not run, no ui surface

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 1 and 3 together, the ones that would hurt most if wrong. 1: the moved check is green on the branch and fails again with the `Authenticated` arm put back to `send_text`. 3: after a stale reconnect notice the live socket gets exactly `["auth", "subscribe"]`, and the check fails with `start_session` put back to `send_text`. I also re-ran the whole suite for criterion 9, because neither lens re-ran it
- command: in worktree `9cea260`, `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target`, all `--offline`:
  - `cargo test --lib -- trade_late_auth_answer trade_stale_reconnect`, 20 runs
  - `cargo test`, once
  - the mutations ran in a throwaway `git archive 9cea260` copy in the scratchpad, deleted afterwards. The branch was not edited
  - mutation A: `send_session_frames` with its epoch check removed and `send_text(frame, None)` put back. Then `cargo test --lib -- trade_late_auth_answer`, 5 runs
  - mutation B: `start_session`'s trade branch with its epoch check removed and `send_text(auth, None)` put back. Then `cargo test --lib -- trade_stale_reconnect`, 5 runs
- result:
  - branch: 20 of 20 green. `cargo test` gives 145 unit + 8 + 2 + 5 + 13 = 173 passed, 0 failed, which matches both count rows
  - mutation A: 5 of 5 red, `left: ["subscribe", "auth", "subscribe"]` at `src/websocket/client.rs:1222`
  - mutation B: 5 of 5 red, `left: ["auth", "auth", "subscribe", "subscribe"]` at `src/websocket/client.rs:1167`
  - every run used the `MockVenue` on 127.0.0.1 only
  - spot check PASS

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- The trade socket logs in but never subscribes to its private channels after a reconnect. Look for `QFEX WebSocket: session frame not sent on its connection` or `login answered on a closed connection, its session frames dropped` with no later `Authenticated` and private `subscribe` on a newer connection. On the venue this shows as fills and order updates stopping after a reconnect while `SocketState` reads `Connected`
- The trade socket never logs in after a drop. Look for `connection replaced before auth, the next one logs in` or `reconnect notice of a replaced connection, no auth sent` with no `auth` frame on the next connection. The execution client shows `logged_in` running into `ANSWER_TIMEOUT`, or `consume` staying logged out after `Reconnected`
- Orders sent during a reconnect are refused before `auth`. This is the known gap the restate left open. Watch for venue errors on order requests right after a `Reconnected`

## Why escalated

The reviewers and the spot check found nothing wrong with the code. I escalated on two rules: `RISK` is `high` and a lens reported a material gap it could not cover, and one acceptance criterion is not yet true where this branch merges. A person needs to look at three things.

1. **A new login branch that no test reaches.** `src/websocket/client.rs:611-614`: on `SendError::ConnectionChanged`, `start_session` returns `Ok(())` without sending `auth`. `connect()` relies on that at `:483` for the first `auth`. The code lens says no test reaches it (`review-code.md`, "What I did not cover", first bullet), and `grep ConnectionChanged src tests` finds only that line. Both lenses judged it correct by reading nautilus-network 0.67.0, which emits `RECONNECTED` only for an `Active` replacement (`review-security.md`, "Fail-open check"). Neither proved it by running it. On the socket that carries orders, a wrong `Ok(())` here would leave a live connection without `auth`, or without cancel on disconnect. Decide one of two things:
   - accept the reading-only proof, or
   - send it back to `build` for a check that forces a drop between the epoch check at `:599` and the writer
2. **Criterion 10 is not met on the base.** `docs/external/nautilustrader/README.md` lists `send_text_on_connection` only on the main checkout's unmerged branch `chore/2026-11-28-auto-run-records` (lines 16 and 28). `git grep` finds it neither at `origin/main` `92dca56` nor at the code branch `9cea260`. `build.md` says it is "on main with the records PR of this run", which is not true yet. Merge that records PR first or together with this one, or the external contract will not describe the API the code now relies on.
3. **Agree where the deferred work goes.** The order path is out of scope by the restate. `build.md` ("Notes for reviewers") says no issue is open for it yet. Security finding 1 adds to it: a stale `Authenticated` still lets `logged_in` (`src/execution/mod.rs:405`) report a login before the live connection has logged in. The security lens also asks that the later fix be reviewed for duplicate orders if it gates the replay buffer. Confirm a box 2 issue gets opened for both before this merges, so the only record is not `docs/STATUS.md`.
