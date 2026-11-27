REPEATS: 1

## What happened

Short. The record is the table below, not this part.

- review rounds: 1. Code PASS with two `[should]` (the new `AGENTS.md` rule cannot hold for records written in the main checkout or after the merge; `build.md` stale), no `[must]`. Approver APPROVE after rerunning every criterion read-only
- the gate missed: 0 `[must]`. `build.md` has no `## Why the gate missed this` section
- lead time: 0 days, issue date and `MERGED-AT` both 2026-11-27 (PR #4 as `3c0d7ed`, PR #7 as `4b46b86`)
- the work merged before review ran: review and deliver recorded merges already made, so review findings could only be fixed afterwards, here
- scope did not change after `restate` (`RESCOPED: no`), but one line of it did not hold: `restate.md` said no agent merges a pull request, and the agent merged PR #4 through the person's account at the person's request in session
- this retro itself is written on `chore/2026-11-27-retro-status-test-count-check`, a branch of another issue, because bloop writes issue files in the main checkout and `bin/job` reads them there: the first row, live

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| The `AGENTS.md` rule "commit bloop records on the issue's own branch" cannot be followed: `.bloop/issues/README.md` says every issue file is written in the main checkout (where `bin/job` reads it), not in the worktree of the issue branch, and `deliver.md`, `retro.md` and the archive move are written after the code's pull request has merged. Post-merge records landed on ad hoc branches: `chore/2026-11-27-records-after-merges` (PR #7, `354922c`, `e832f23`), `chore/2026-11-27-retro-status-test-count-check` (`0b01b80`, `93dce31`, `dfe46ed`), and this retro | 3 | human, until the issue lands | `AGENTS.md`, "Every time": a clause for records written after the merge (commit them on `chore/<id>-records` cut from `origin/main` and open a pull request; push a `bin/archive` commit with `git push origin main:chore/<id>-archive`, then reset local `main`). The contradiction with `.bloop/issues/README.md` for records written before the merge, and a check that catches records stranded off `main`, are issue `2026-11-27-post-merge-records-route` |
| `build.md` went stale against what landed: the seven acceptance boxes stayed unchecked after the merges, a blank line split the slice 3 row out of the table, and slice 3 landed on `chore/2026-11-27-records-after-merges` instead of the branch `spec.md` names, with no note. The work merged before review, so nobody returned to `build.md` | 1 | human | `build.md` of this issue, in this retro: boxes checked with the commands the approver ran, the table joined, a note on the slice 3 branch (review-code.md finding 2) |
| Acceptance criterion 6 measured "no file under `src/` or `tests/` changed" as `git diff --stat 362bcb0 origin/main`, a diff from a fixed commit, which started printing 3 files as soon as PRs #5 and #6 of other issues merged, though this work touched none | 1 | human | `build.md` criterion 6 now records the command scoped to this work's commits (`git show --name-only 0d3a8ff 354922c -- src tests`, empty) and why the written one no longer measures it |
| `restate.md` said no agent merges a pull request; the agent merged PR #4 through the person's account after "reviewed, can you merge all". Criterion 7 still reads `merged_by` a `User`, so the check cannot tell the two apart | 1 | human | `build.md` criterion 7 says which merge the person made (PR #7) and which the agent made on request (PR #4). A person's request in session is the authority; no check can read it |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. The "this issue" column is read from this folder (`review/round-1` only, `decision.md`, `deliver.md`).

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 0 days | 0.0 days median |
| review rounds | 1 | 1.25 mean |
| escalate rate | no | 17% (`escalateRate` 0.1667) |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

`bin/metrics` also says 2 delivered issues have no `retro.md`: this one until now, and `2026-11-27-data-client-reconnect-command`.

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`: none. It does not describe where bloop records live or how they reach `main`, and this issue changed no code, module or dataflow
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- The half of review finding 1 about records written before the merge: `AGENTS.md` still says they go on the issue branch while `.bloop/issues/README.md` says they are written in the main checkout. `.bloop/issues/README.md` is a file the bloop plugin manages and was out of scope in `restate.md`; now `2026-11-27-post-merge-records-route`
- No check catches record commits stranded off `main` (the `deliver.md` watch signal `git log origin/main..<branch> -- .bloop/issues/`); now `2026-11-27-post-merge-records-route`
- `claude/bloop-init` and `origin/claude/subscribe-gate-a9d5d1` were not deleted: not asked for, and other sessions may still have them checked out (`spec.md`, Assumptions)
- `auto.md` of this issue lists the plan run at 15:00Z before the build run at 14:43Z; the times were not corrected, since nothing reads their order
- Archiving this folder: `bin/archive` refused (`on chore/2026-11-27-retro-status-test-count-check, not main`). It waits until these records are on `main`, then runs there, per the new `AGENTS.md` clause
- Product docs: `bin/docs` reports that `docs/product` does not exist, so nothing was refreshed
