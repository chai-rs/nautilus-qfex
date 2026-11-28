REPEATS: 1

## What happened

Short. The record is the table below, not this part.

- review rounds: 1. Code PASS with one `[should]`: the drop retry loop in `tests/exec_client.rs:1010-1015` had no upper bound. No `[must]`. The approver approved after running criteria 1 to 6 and 9 again itself (`socket_state_follows_the_trade_socket` 11 of 11 runs)
- the gate missed: 0 `[must]`. `build.md` has no `## Why the gate missed this` section
- lead time: 0 days. The issue date and `MERGED-AT` are both 2026-11-27 (PR #8 as `ce9d457`, merged by the agent at the person's request in the session). CI on `main` was green for `ce9d457` (run `46031361751`) and for `7e67051` (run `46032555854`)
- scope did not change after `restate` (`RESCOPED: no`)
- the lesson of the last retro held. The `2026-11-27-market-data-socket-state` retro left the reconnect-registry side effect at the `human` rung, with a note in `docs/external/nautilustrader/README.md`. This plan read `node/builder.rs`, built the control in `connect()`, and pinned the outcome with a registry-scope assertion in the new test (criterion 5)
- while measuring the review finding, this retro found a separate race. `socket_state_follows_the_trade_socket` as merged fails 1 run in 100 with `venue refused: ["subscribe before auth"]`. The cause is in the adapter, not in the stand-in (row 2)

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| The stand-ins cut a socket with `Notify::notify_waiters`, which only wakes a task that is already waiting. If `Venue::serve` was busy answering a frame, the drop was lost: 5 of 30 build runs failed. The build worked around it with a retry loop that had no upper bound, so a missing `Disconnected` would hang `checks.sh` instead of failing it. The `Disconnected` event's client id and endpoint were also left unasserted | 1 | test | `tests/exec_client.rs` and `tests/data_client.rs`: `Venue::drop_socket()` uses `notify_one`, which keeps the permit until `serve` polls it. All three drop sites call it (`reconnect_logs_in_again_and_catches_up`, `socket_state_follows_the_trade_socket`, and the data client's reconnect test at `:313`). The retry loop is gone. `Disconnected` now goes through `next_state`, which asserts client id, venue and endpoint and fails after 10 s. Proven both ways: without the drop the test fails in 10.2 s with `no socket state within 10 s`; with it, the two exec tests pass 100 of 100 and `data_client` passes 50 of 50. `./checks.sh` passes (171, as written) |
| If the trade socket drops after the venue answers `auth` but before `read_loop` handles that answer, the late answer can send the private `subscribe` frames on the next socket before that socket's `auth`. Seen as `subscribe before auth` at the stand-in: 1 run in 100 at `ce9d457`, and 2 in 100 once the drop became reliable and fired right after `Connected`. The diagnosis is a hypothesis | 1 | human, until the issue lands | issue `2026-11-27-trade-late-auth-answer` (`KIND: bug`, for `/bloop:debug` and then a fix). `socket_state_follows_the_trade_socket` now waits for the first `subscribe` before the drop, as `Harness::connect` does, so a socket-state test no longer fails on this race. `docs/STATUS.md`, the `websocket` trade client row, records the race as known and not fixed |
| The records of this issue from before the merge (intake to build, `4ee4181` to `bc51081`) were committed on `chore/2026-11-27-retro-status-test-count-check`, another issue's branch, and not on `feat/2026-11-27-trade-socket-state`. The `AGENTS.md` rule cannot be followed for files bloop writes in the main checkout. The records written after the merge used the new `chore/<id>-records` clause (PR #10) with no trouble | 4 | human, until the issue lands | issue `2026-11-27-post-merge-records-route` (waiting on a person's three answers): its `issue.md` now lists this fourth case. It also records that the route for records written after the merge worked, so the open question is only about records written before it |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. The "this issue" column is read from this folder (`review/round-1` only, `decision.md`, `deliver.md`).

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 0 days | 0.0 days median |
| review rounds | 1 | 1.20 mean |
| escalate rate | no | 14% (`escalateRate` 0.1429) |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

`bin/metrics` also says 2 delivered issues have no `retro.md`: this one until now, and `2026-11-27-data-client-reconnect-command`.

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`, `## Dataflow` step 7: already names both sockets, the `trade` endpoint and `Unsupported` for its reconnect command. Slice 2 (`21c6b9e`) did this, so this retro changes nothing there
- `docs/external/nautilustrader/README.md`, "Reconnect registry side effect": now also says that the execution client avoids the side effect by building its control in `connect()`, and which test pins it. Done in this retro
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- Nobody has run a real `LiveNode` end to end. The claim that the trade control sees the system event sender rests on reading `nautilus-live` 0.67.0, not on a run (review "What I did not cover", deliver "Watch after deploy"). A host that builds the node on one thread and connects it on another would get no `trade` events
- After a refused login, the last state an actor sees is `Connected` while the client has stopped. The spec accepted this in Risks, and `docs/STATUS.md` records it
- No reconnect handler is registered for the trade socket. The restate put it out of scope. The pattern now exists (`2026-11-27-data-client-reconnect-command`, `f9b4079`), but no issue was opened: a commanded reconnect would force a new login and a catch-up, and a person has to want that first. Such a handler would need the control built in `new`, inside the registry scope (`docs/STATUS.md`, socket state row)
- The late-`auth` race (row 2) is only opened as an issue and recorded, not fixed
- Watch after deploy: there is no deploy. Nobody has reported a socket state check from mocha against `ce9d457` (`deliver.watchFor`)
- `auto.md` has no `Runs` row for the plan run, only the trust-level answer at 15:10Z. The row was not added, because its times are not recorded anywhere
- Product docs: `bin/docs` reports that `docs/product` does not exist, so nothing was refreshed
