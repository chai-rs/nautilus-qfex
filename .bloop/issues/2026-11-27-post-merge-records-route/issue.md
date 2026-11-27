FR: none
KIND: request
GOAL: none

Every bloop record of an issue reaches `main` by one route that an agent can follow without breaking a rule, and a check fails when records sit on a branch `main` never receives.

## Where it comes from

- Retro of `2026-11-27-bloop-records-on-base-branch`, row 1, and `review/round-1/review-code.md` finding 1 of that issue.
- `AGENTS.md` says records are committed on the issue's own branch, which `bin/work` cuts into `.worktrees/<id>/`. `.bloop/issues/README.md` says every issue file is written in the main checkout, because `bin/job`, the board and `/bloop:auto` read them there. A file written in the main checkout is not on the worktree branch, so the rule cannot be followed as written, before the merge or after it.
- Records written after the code's pull request merges (`deliver.md`, `retro.md`, the archive move) landed on ad hoc branches three times in one day:
  - `chore/2026-11-27-records-after-merges` (PR #7: `354922c` archive, `e832f23` records of three issues)
  - `chore/2026-11-27-retro-status-test-count-check` (`0b01b80` retro, `93dce31` archive, `dfe46ed` review and deliver of `2026-11-27-bloop-records-on-base-branch`)
  - the retro of `2026-11-27-bloop-records-on-base-branch` itself, on that same branch
- `bin/archive` refuses off `loop.baseBranch`, so an archive is always a local commit on `main` that has to be pushed to another branch name.
- Seen a fourth time, in the retro of `2026-11-27-trade-socket-state`. Its records from before the merge (intake to build) were committed on `chore/2026-11-27-retro-status-test-count-check`, a branch of another issue, not on `feat/2026-11-27-trade-socket-state`. Its `deliver.md` followed the `AGENTS.md` clause: it went on `chore/2026-11-27-trade-socket-state-records` (PR #10, `7e67051`) with no trouble. So that route works for records written after the merge, and the gap is only in records written before it.

## What is asked

- A person decides the route, among for example:
  - keep the main checkout on `main`, commit records there, and publish them through short `chore/<id>-records` branches (what the retro clause in `AGENTS.md` now says for post-merge records)
  - copy pre-merge records into the issue's worktree before its pull request, so they ride with the code
  - ask the bloop plugin to support records on a branch other than the base (outside this repo)
- A check, in `checks.sh` or CI, that fails when a branch other than `main` has carried `.bloop/issues/` commits for longer than its pull request, or another signal the person prefers.
- `AGENTS.md` and `.bloop/issues/README.md` say the same thing afterwards.

## Constraints

- No change to adapter code or tests.
- No agent pushes to `main`. `.bloop/config.json` is a protected path.
