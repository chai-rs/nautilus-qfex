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
- Seen a fifth time, in the retro of `2026-11-27-trade-late-auth-answer`, and this time with a project document, not only bloop records. Box 3 (`external-contract`) wrote the `send_text_on_connection` rows of `docs/external/nautilustrader/README.md` in the main checkout, so they went to `main` with the records branch `chore/2026-11-28-auto-run-records` (PR #18), not with the code branch `fix/2026-11-27-trade-late-auth-answer` (PR #16). Acceptance criterion 10 of the code was therefore false on the base while PR #16 was ready: the round 1 approver escalated it (`review/round-1/decision.md`, point 2), the person chose to merge PR #18 first, and the round 2 decision made "PR #18 merges before PR #16" a merge condition that deliver had to check with a `git grep` against `origin/main` before merging (`decision.md`, `deliver.md` row "merge condition"). So the route has to cover files a skill writes in the main checkout that the code's acceptance criteria depend on, such as `docs/external/`, not only `.bloop/issues/`

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
