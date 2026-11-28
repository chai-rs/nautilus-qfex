APPROVE
APPROVED-BY: none

Round 2. Whole change `92dca56...575c901` on `fix/2026-11-27-trade-late-auth-answer`, PR #16 (draft). The round 1 decision (ESCALATE) is at `review/round-1/decision.md`. A person answered its three points, recorded in `auto.md` under "Person's decision on the round 1 escalation".

**Merge condition: PR #18 (`chore/2026-11-28-auto-run-records`) merges before PR #16.** This is the person's answer to round 1 point 2, and acceptance criterion 10 depends on it. At `origin/main` `d779309`, `git grep -n send_text_on_connection origin/main -- docs/external/nautilustrader/README.md` prints nothing (exit 1). On `origin/chore/2026-11-28-auto-run-records` it prints lines 16 and 28. Before #16 merges, `deliver` reruns that grep against `origin/main`. If it prints nothing, `deliver` stops and does not merge. `origin/main` + #18 + #16 merges clean (`git merge-tree`).

## Reviewers

- code: PASS (`review/round-2/review-code.md`). No findings. Reran mutation M1 itself, 6 of 6 red
- security: PASS (`review/round-2/review-security.md`). Round 1 `[should]` finding 1 (stale `Authenticated` seen by `logged_in`) moved to `2026-11-28-trade-orders-before-live-login`, as the person decided. Severity 1, does not block
- ux: not run, no ui surface
- ui: not run, no ui surface

## Round 1 escalation points

1. `ConnectionChanged` branch untested. **Closed.** `trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in` (`src/websocket/client.rs:1194`) reaches it. The code lens and I each ran M1 and got red
2. Criterion 10 only on the records branch. **Still true. It is handled by the merge condition above, which the person chose.** PR #18 is open and not merged (checked through the GitHub REST API at 07:16Z)
3. Deferred order path and stale `Authenticated`. **Closed.** The issue `2026-11-28-trade-orders-before-live-login` is opened on the #18 branch, and the trade client row of `docs/STATUS.md` names it

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: **6 on the new path.** On the trade socket that carries orders, the worst outcome is a live connection after a reconnect with no `auth`, or with `auth` but no cancel on disconnect. The new `ConnectionChanged` arm is the one place that returns `Ok(())` without sending. The security lens listed as not covered that no run showed cancel on disconnect turned back on after this arm, because the new test runs with `new_trade(.., false)`. I also checked **9**, the gate, on the tree that actually lands, `origin/main` `d779309` merged with `575c901`. No lens ran it there
- command: throwaway copies in the scratchpad, deleted afterwards. The branch was not edited. Every run was `--offline` against `MockVenue` on 127.0.0.1, and nothing contacted the QFEX venue
  - merged tree: `git merge-tree --write-tree origin/main 575c901` (`cbda334`) extracted with `git archive`. Then `sh ./checks.sh` with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target-late-auth-approver/merged`, and `cargo test --lib trade_` 20 times
  - cod variant: `git archive 575c901`, with the new test changed to `new_trade(&url, fixed_signer(), true)`, waits of `requests(1).len() == 3` and `requests(2).len() >= 3`, and the expected socket 2 frames `["auth", "subscribe", "cancel_on_disconnect"]`. Then `cargo test --lib trade_auth_for_a_connection_replaced` 20 times with `CARGO_TARGET_DIR=.../target-late-auth-approver/cod`
  - the same variant with M1 (the `Err(SendError::ConnectionChanged)` arm deleted), 5 runs, and with M2 (`start_session` back to `send_text(auth, None)`), 5 runs
- result:
  - merged gate: exit 0. `cargo test --features examples` and `cargo test` both gave 174 passed (146 unit + 8 + 2 + 5 + 13), "as written" for both count rows. Format, clippy (both feature sets) and the shortcut marker were clean. `cargo test --lib trade_`: 20 of 20 green, 7 tests each
  - cod variant: 20 of 20 green. After `ConnectionChanged`, socket 2 receives exactly `auth`, `subscribe`, `cancel_on_disconnect`
  - M1 on the variant: 5 of 5 red at `src/websocket/client.rs:1216:9`, the `is_ok` assertion, so the run really goes through the arm
  - M2 on the variant: 5 of 5 red at `:1228:9`, `left: ["auth", "auth", "subscribe", "cancel_on_disconnect", "subscribe", "cancel_on_disconnect"]`. No lens had rerun M2
  - A first attempt shared one target dir between the two copies. That reused a stale cod/M2 binary for the merged runs, and the backtrace showed the cod source path. I threw those results away and reran everything above in a separate target dir for each copy. `Compiling nautilus-qfex v0.1.0 (.../scratchpad/cod)` confirms the variant build
  - CI on `575c901`: `check` success, `fuzz` success (both runs)
  - spot check PASS

## Watch after deploy

What could still go wrong once this is live, and what would show it. The trade client stays UNVERIFIED on the venue (`docs/STATUS.md:77`). Every check here is against the mock, so the first live reconnects are the real test.

- The trade socket logs in after a reconnect but never subscribes, or never turns on cancel on disconnect. Signal: `QFEX WebSocket: session frame not sent on its connection` or `login answered on a closed connection, its session frames dropped`, with no later `Authenticated` on a newer connection. On the venue, fills and order updates stop after a reconnect while `SocketStateChanged` for endpoint `trade` reads `Connected`. Resting orders survive a disconnect they should not have survived
- The trade socket never logs in after a drop. Signal: `connection replaced before auth, the next one logs in` or `reconnect notice of a replaced connection, no auth sent`, with no `Authenticated` after the next `Reconnected`. The execution client's `logged_in` runs into `ANSWER_TIMEOUT`, or `consume` stays logged out after `Reconnected`
- `read_loop` stalls on a stale send. `send_text_on_connection` waits in `wait_for_active` for up to the connect timeout, 10 s by default (spec Risks, first bullet). Signal: market or trade frames arriving in a burst several seconds after a `Reconnected`
- The known gap stays open (`2026-11-28-trade-orders-before-live-login`). Orders sent with `send` during a reconnect can be replayed before `auth`, and `logged_in` can see a stale `Authenticated`. Signal: venue errors on order requests right after a `Reconnected`

## Why escalated

Not escalated. Every lens is at PASS. My spot check passed on the merged tree and closed the one gap at this `RISK` that a lens could not cover (cancel on disconnect after `ConnectionChanged`). `spec.md` `## Assumptions` has no starred product assumption. Round 2 adds only a test, a test-only `MockVenue` gate and `docs/STATUS.md` rows, all of them asked for by the person's round 1 answer. The one open item, criterion 10, is the merge condition at the top of this file, set by the person. `APPROVED-BY` stays `none` until a person signs.
