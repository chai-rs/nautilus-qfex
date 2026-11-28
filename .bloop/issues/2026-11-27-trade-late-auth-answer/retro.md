REPEATS: 3

## What happened

Short. The record is the table below, not this part.

- review rounds: 2. Round 1: code PASS (no finding), security PASS with one `[should]` (a stale `Authenticated` still counts as a login), approver ESCALATE on three points. A person answered all three with the recommended option (`auto.md`, "Person's decision on the round 1 escalation"). Round 2: code PASS, security PASS, approver APPROVE on the condition that PR #18 merges before PR #16
- the gate missed: 0 `[must]`. `build.md` `## Why the gate missed this` records one gap the approver escalated, not a lens `[must]`: the `ConnectionChanged` login branch had no test (row 1)
- lead time: 1 day, issue date 2026-11-27 and `MERGED-AT` 2026-11-28 (PR #16 as `4134e4c`, after PR #18 as `4a32ba5`), the two fields `bin/metrics` reads
- scope changed once after intake (`RESCOPED: yes`): orders sent with `send` during a reconnect share the root cause but were left out as an order-state decision. The person had that opened as `2026-11-28-trade-orders-before-live-login`, together with the security `[should]`
- deliver: the rebase onto `4a32ba5` had 0 conflicts, the gate passed twice in a fresh target dir, and the push waited on the person's "force-push แล้ว merge" (force-push, then merge). The merge went straight through the REST merge with `expectedHeadSha` pinned, because `gh pr merge` had already been refused with 403 earlier in the same session, as the `2026-11-28-trade-socket-control-shape` retro recorded. That route worked, so it is not a row here. CI on `4134e4c` passed (run `46123378992`)
- caught by a person's rule, nothing to add: the merge condition held. `deliver` ran the `git grep` against `origin/main` before merging and it printed both lines
- this retro ran `./checks.sh` on `chore/2026-11-28-pr16-records` with its edits uncommitted, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target-retro16`, a fresh dir: exit 0, 171 passed, as written. That branch was cut at `4a32ba5`, before PR #16, so it tested the tree without #16's code; the records add no code, and deliver gated #16's code on `5721890`. The first run was lost to a full disk (six target dirs, about 55 GB); the dirs of PR #16's finished approver and deliver runs, `target-late-auth-approver` and `target-late-auth-deliver`, were deleted to make room (row 3)

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| The fix added a login branch that returns `Ok(())` without sending `auth` (`start_session`, `Err(SendError::ConnectionChanged)`, `src/websocket/client.rs:611-614`), and no test reached it. Both lenses judged it correct by reading nautilus-network 0.67.0, and only the approver escalated it, because on the order socket a wrong `Ok(())` would leave a live connection without `auth` or without cancel on disconnect | 1 | test | `src/websocket/client.rs`, merged in `4134e4c`: the unit check `trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in` and the test-only `MockVenue::next_upgrade_gate`, which holds the next connection's upgrade so the epoch-0 `auth` waits in `wait_for_active` until the writer answers `ConnectionChanged`. Red under M1 (the arm removed) and M2 (`send_text` back). The code lens and the approver each reran M1, and the approver reran M2 with cancel on disconnect turned on |
| A file the code's acceptance criteria depend on was written in the main checkout, so it did not ride with the code. Box 3 (`external-contract`) wrote the `send_text_on_connection` rows of `docs/external/nautilustrader/README.md` there, so they went to `main` on the records branch (PR #18) and not with PR #16. Criterion 10 was false on the base while #16 was ready, the round 1 approver escalated it, and the merge order became a condition deliver had to check by hand | 5 | human, until the issue lands | issue `2026-11-27-post-merge-records-route`: its `issue.md` now lists this fifth case (the first four are listed there too) under "Where it comes from" and says the route must also cover files such as `docs/external/` that a skill writes in the main checkout, not only `.bloop/issues/`. The merge condition in `decision.md`, and the `deliver.md` row "merge condition" that checked it, are what held this time |
| The approver's spot check built two throwaway copies (the merged tree and a mutant variant) into one target dir, so the merged tree's `./checks.sh` and `cargo test --lib trade_` runs ran the mutant's stale binary. The backtrace showed it, and the results were thrown away and rerun with a target dir per copy | 4 | human, until the issue lands (the issue asks for the test rung: `checks.sh` builds this crate from its own tree) | issue `2026-11-28-gate-tests-its-own-tree`: its `issue.md` now lists this fourth case after the three already there, and notes that part of these runs were bare `cargo test` loops in `git archive` copies, which a fix inside `checks.sh` alone does not cover, for restate to decide |
| Every mutation that proved a check bites (A and B in round 1, M1 and M2 in round 2, the approver's variant with cancel on disconnect) lives only as prose in `build.md` and `decision.md`, with no patch and no captured output under `evidence/`. The round 2 code lens named it, and the code lens and the approver each rebuilt the mutations by hand from the prose. One of those rebuilds is the stale-binary case in row 3 | 4 | human, until the issue lands (the issue asks for the test rung: saved patches and a script that reruns them and fails on a green one) | issue `2026-11-28-mutation-proofs-as-files`, opened through `/bloop:backlog` with `FR: none`; its `issue.md` names the four issues (this one, `2026-11-27-market-data-socket-state`, `2026-11-27-data-client-reconnect-command`, `2026-11-28-trade-socket-control-shape`). A script is code and goes through the loop on its own branch, not through this retro |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. The "this issue" column is read from this folder (`review/round-1` and `review/round-2`, `decision.md`, `deliver.md`).

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 1 day | 0.0 days median |
| review rounds | 2 | 1.29 mean |
| escalate rate | no (`bin/metrics` reads the final `decision.md`, APPROVE; round 1 escalated) | 9% (`escalateRate` 0.0909) |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

`bin/metrics` also says 2 delivered issues have no `retro.md`: this one until now, and `2026-11-27-data-client-reconnect-command`.

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`, `## Dataflow` step 5: said only that the trade WebSocket sends the order to QFEX. Now says each trade connection sends its own `auth` first, the private `subscribe` and cancel-on-disconnect frames go out only on the connection whose `auth` succeeded (`send_text_on_connection`, since `4134e4c`), and orders still use the ordinary send (`2026-11-28-trade-orders-before-live-login`). Done in this retro
- `docs/external/nautilustrader/README.md`: already lists `epoch_builder`, `send_text_on_connection` and `connection_epoch()`, and the "`send_text` is not bound to a connection" entry (PR #18). No change
- `docs/STATUS.md`, the `websocket` trade client row: already names the three unit checks and the open order-path issue (`4134e4c`). No change
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- Orders sent with `send` during a reconnect can still be replayed on the next connection before its `auth`, and `logged_in` can still take a stale `Authenticated` (security `[should]`, both rounds): out of scope in `restate.md`; now `2026-11-28-trade-orders-before-live-login`
- The market data socket's `start_session` and subscribe pump still use the unbound send. With no `auth` there, a replayed `subscribe` is only a repeat (`restate.md`, "What we will not do")
- What QFEX itself does with a frame before `auth`, and whether a refused one is answered: needs the live venue, which this work never contacts. The trade client stays UNVERIFIED on the venue (`docs/STATUS.md`)
- `read_loop` can wait up to the connect timeout (10 s by default) on a stale bound send while the transport reconnects (`spec.md` Risks, first bullet). Accepted, not measured
- The round 2 check uses a 200 ms hold to show the `auth` is still waiting. Nobody judged how it behaves on a slow CI runner beyond the margin against the 10 s connect timeout (`review/round-2/review-code.md`). CI passed on `575c901` and `4134e4c`
- The round 2 check runs with cancel on disconnect off. Only the approver's throwaway variant showed cancel on disconnect turned back on after `ConnectionChanged`; nothing in the gate reruns it
- The `api_key` query parameter in the trade socket URL predates this change; whether anything logs that URL was not checked (security lens, both rounds)
- `nautilus-risex` and `nautilus-lighter` are not on this machine, so the epoch handling was not compared with them
- Watch after deploy: there is no deploy. The four signals in `deliver.md` "Watch after deploy" need a live run, and nobody has reported mocha's socket state check against `4134e4c` (`deliver.watchFor`)
- Product docs: `bin/docs` reports that `docs/product` does not exist, so nothing was refreshed
