STATUS: MERGED
MERGE: 3c0d7ed
MERGED-AT: 2026-11-27
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

## The run

| step | what ran | result |
| --- | --- | --- |
| sync | none | the work had already merged before review: PR #4 as `3c0d7ed` (slices 1, 2) and PR #7 as `4b46b86` (slice 3, archive) |
| re-gate | `./checks.sh` on PR #7's branch before it merged | exit 0, 170 passed, as written |
| merge | PR #4 merged by the agent on the person's request in session ("reviewed, can you merge all"); PR #7 merged by the person | `3c0d7ed`, `4b46b86` |
| deploy | not applicable | records and docs only |

## Watch after deploy

- The new rule at `AGENTS.md:20` conflicts with `.bloop/issues/README.md` ("every file of the issue folder is written" in the main checkout) and says nothing about records written after the code PR merges (`deliver.md`, `retro.md`, archive). Signal: the next issues land those records on ad hoc `chore/` branches again, as `354922c` did through PR #7 and as the current checkout branch `chore/2026-11-27-retro-status-test-count-check` does. Follow-up: one clause in `AGENTS.md` for post-merge records (review-code.md finding 1)
- Record commits stranded off `main` again. Signal: `git log origin/main..origin/claude/bloop-init` (or any other long-lived branch) printing commits that touch `.bloop/issues/`
- `build.md` of this issue is stale: acceptance boxes unchecked, a blank line splitting the slices table at line 90, and slice 3 branch name differs from `spec.md` without a note (review-code.md finding 2). Records only; fix with the retro

## What a rollback would take

- command: none needed for records; reverting `3c0d7ed` would drop the records from `main`
- what it cannot undo: nothing persistent
