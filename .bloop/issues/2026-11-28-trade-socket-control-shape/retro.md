REPEATS: 2

## What happened

Short. The record is the table below, not this part.

- review rounds: 1. Code PASS with no finding. Approver APPROVE after rerunning criteria 1 to 7 itself, with a mutation (the control built in `new`) that made `socket_state_follows_the_trade_socket` fail at `tests/exec_client.rs:1031`
- the gate missed: 0 `[must]`. `build.md` has no `## Why the gate missed this` section
- lead time: 0 days. The issue date and `MERGED-AT` are both 2026-11-28 (PR #17 as `d779309`, merged by the agent through the REST API after the user's "merge ให้หน่อย")
- scope did not change after `restate` (`RESCOPED: no`). The restate chose the reading that keeps `Unsupported` and left the trade socket's reconnect handler out
- the lesson of `2026-11-27-market-data-socket-state` held a second time: the plan read `node/builder.rs`, and a prototype proved that the existing test fails when the control moves into the constructor, so the refactor needed no new test
- CI on `d779309` (run `46118159407`): the `check` job, which runs `./checks.sh`, passed; the `fuzz` job was still running when this retro was written (read on GitHub, read only)

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| A `CARGO_TARGET_DIR` shared between checkouts let `./checks.sh` run test binaries another tree had built, so a gate result belonged to a tree it did not test: PR #16's worktree binaries counted 173 in the main checkout against rows of 171 (`spec.md` Risk 4), and the approver's first `./checks.sh` failed on a mutant copy's test binary (`decision.md`). Earlier in `2026-11-27-trade-late-auth-answer` (`debug.md`). Cleared by hand with `cargo clean -p nautilus-qfex` each time | 3 | human, until the issue lands (the issue asks for the test rung: `checks.sh` builds this crate from its own tree) | issue `2026-11-28-gate-tests-its-own-tree`, opened through `/bloop:backlog` with `FR: none`. `checks.sh` is code and goes through the loop on its own branch, not through this retro |
| The evidence of a slice names the parent commit "with uncommitted changes", because the gate runs before the commit, so review cannot match the tested tree with the reviewed commit and relied on timestamps (`review/round-1/review-code.md`, "What I did not cover"). Every `evidence/slice-*.md` of the 7 issues with evidence has the same line; this is the first review to name it | 7 | human, until the issue lands (the issue asks `checks.sh` to print the git tree hash it tested) | issue `2026-11-28-gate-tests-its-own-tree`, "What is asked", second bullet. `bin/gate` and `evidence-pack` belong to the bloop plugin, so the fix asked for lives in `checks.sh` |
| `bin/ship --merge` could not merge: `gh pr merge` got HTTP 403 because GitHub GraphQL is closed to this session. The merge went through `gh api -X PUT .../pulls/17/merge` with `sha=b9d820e`, pinned to the gated head | 1 | human | `deliver.md`, rows "merge" and "merge, REST", record the refusal and the exact REST call, with the `sha` pin that stops it merging a head the gate did not see. `bin/ship` belongs to the plugin and the session's GitHub access to the environment; nothing in this repo can change either |
| `clean-comment` ran on the diff after the slice was committed, not before as `build` asks | 1 | human | `build.md`, "Notes for reviewers", records the order and that `comment-cleanser` deleted nothing, so the commit did not change. No check can see the order of two steps in one session |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. The "this issue" column is read from this folder (`review/round-1` only, `decision.md`, `deliver.md`).

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 0 days | 0.0 days median |
| review rounds | 1 | 1.17 mean |
| escalate rate | no | 22% (`escalateRate` 0.2222) |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

`bin/metrics` also says 2 delivered issues have no `retro.md`: this one until now, and `2026-11-27-data-client-reconnect-command`.

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`, `## Dataflow` step 7: still true, but named no holder for the trade control. Now says the control is held in the private `execution::trade_socket::StateOnlyTradeSocketControl`, which registers no reconnect handler (since `b9d820e`). Done in this retro
- `docs/external/nautilustrader/README.md`, "Reconnect registry side effect": now names `StateOnlyTradeSocketControl` (`src/execution/trade_socket.rs`), `built_on_first_connect()` in `new` and `sink(client_id)` in `connect()`, so a plan that wants a trade reconnect handler finds where the timing lives. Done in this retro
- `ARCHITECTURE.md`, `## Codebase shape`: no change, it lists modules, not files
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- A reconnect handler for the trade socket (forced re-login and catch-up on command): out of scope in `restate.md`. No issue was opened: it changes the order socket in the login path that PR #16 is still changing, and it would make the replay that PR #16 records reachable on demand. It waits until PR #16 and `2026-11-27-uat-order-path` are settled, and a person has to want it
- `spec.md` Risk 2: no test separates one control kept across reconnects from a new control per `connect()`. Review read the `sink` body; nothing reruns that reading
- Merge order with PR #16 (still an open draft at `9cea260`): whichever lands second re-runs `./checks.sh` after a rebase, with its own target dir or a cleaned crate. Not observed yet
- Watch after deploy: there is no deploy. The `fuzz` job of CI run `46118159407` had not finished; nobody has reported mocha's socket state check against `d779309` (`deliver.watchFor`)
- Nobody has run a real `LiveNode` and sent a reconnect command for the execution client; the `Unsupported` answer rests on `nautilus-live` 0.67.0 sources and the registry-scope test
- Product docs: `bin/docs` reports that `docs/product` does not exist, so nothing was refreshed
- Archiving this folder: `bin/archive` refused (`on chore/2026-11-28-auto-run-records, not main`). On `main` the folder holds only `issue.md` until the records branch merges, so it waits until these records are on `main`, then runs there, per `AGENTS.md`
