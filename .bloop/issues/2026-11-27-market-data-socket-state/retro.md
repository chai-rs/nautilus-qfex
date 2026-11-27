REPEATS: 2

## What happened

Short. The record is the table below, not this part.

- review rounds: 2. Round 1: code PASS with one `[should]`, approver ESCALATE on an unrecorded side effect (reconnect command for the data client answers `UnknownEndpoint`). Round 2: code PASS, approver APPROVE
- the gate missed: 0 `[must]`, 1 `[should]` (the reconnect side effect, escalated by the approver)
- lead time: 0.0 days from the issue date to merge (`362bcb0`, merged by a person on 2026-11-27)
- caught by the machines, nothing to add: `bin/job` flagged crate paths in the spec's Context (`bad-context`, fixed in the next run); the gate failed once on `cargo fmt` import order in slice 1, passed on rerun
- scope did not change after `restate` (`RESCOPED: no`)

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| Building a `SocketControlFactory` in the data client's constructor, inside the node's reconnect registry scope, changed a reconnect command's answer from `Unsupported` to `UnknownEndpoint`; the plan read `SocketControlFactory` but not `LiveNodeBuilder`, and no test builds the client inside a registry scope | 1 (the trade socket will meet it next: `2026-11-27-trade-socket-state` says so) | human, until the test lands | `docs/external/nautilustrader/README.md`, "What we cannot rely on": a "Reconnect registry side effect" entry that tells a plan adding a `SocketControlFactory` to read `node/builder.rs` and state the outcome. The test rung (a registry-scope lookup of `market_data`) is in the "What is asked" of `2026-11-27-data-client-reconnect-command`; it is not written yet |
| A hand-written test count did not match `cargo test`: the request said 162 (`docs/STATUS.md` not updated by `0e8ed08`, real 166), and spec criterion 6 said 167 (real 168) | 2 | test | issue `2026-11-27-status-test-count-check`: a `checks.sh` step comparing the `docs/STATUS.md` count with the `cargo test` totals. Not written here: `docs/STATUS.md` on `claude/bloop-init` still says 162, so the step has to land on a branch cut from `main` |
| The bloop records and `checks.sh` live on `claude/bloop-init` while `loop.baseBranch` is `main`: the issue branch was cut from `claude/bloop-init`, its merge carried the setup commits into `main`, review round 2 found a `docs/STATUS.md` pointer to a folder not on the branch, and `bin/archive` refused to archive this issue | 3 | human | issue `2026-11-27-bloop-records-on-base-branch`: a person chooses between merging `claude/bloop-init` into `main` and changing `loop.baseBranch`. Agents never push to `main`, so no agent can fix it |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. One issue was delivered in the window, so the window figures are this issue's.

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 0.0 days | 0.0 days median |
| review rounds | 2 | 2.00 mean |
| escalate rate | 0 (`bin/metrics` reads the final `decision.md`, APPROVE; round 1 escalated) | 0% |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`, `## Dataflow` step 7: said no socket publishes `SocketStateChanged`. Now says the market data socket publishes it since `362bcb0`, the trade socket does not yet, and a reconnect command for the data client answers `UnknownEndpoint`. Done in this retro
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- Socket state for the trade socket: out of scope in `restate.md`; now `2026-11-27-trade-socket-state`
- Registering a reconnect handler for the market data socket, or answering `Unsupported` again: the person chose to record the side effect, not fix it; now `2026-11-27-data-client-reconnect-command`
- The test of `SocketStateChanged` reaching a real actor in a `LiveNode`: the test checks `SystemEvent::SocketState` from the data client (spec, Assumptions); mocha's prototype showed the actor side
- The Python factory path was not checked end to end for the system event sender (`review/round-1/review-code.md`, "What I did not cover")
- Watch after deploy: there is no deploy; mocha pins `362bcb0`, and nobody has reported its socket state check yet (`deliver.watchFor`)
- Archiving this folder: `bin/archive` refused (`on claude/bloop-init, not main`); waits on `2026-11-27-bloop-records-on-base-branch`
