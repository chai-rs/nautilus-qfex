STATUS: GATED
MERGE: none
MERGED-AT: none
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

## The run

| step | what ran | result |
| --- | --- | --- |
| sync | `bin/ship 2026-11-27-status-test-count-check` | already up to date with `origin/main` (`362bcb0`) |
| re-gate | `bin/gate --e2e` inside `bin/ship` | exit 0 |
| merge | not run | waits for a person: `deliver.autoMerge.medium` is false and `bin/trust --for autonomous` is not met |
| deploy | not applicable | CI and tooling only |

## Watch after deploy

- Merge order with PR #5 (170 tests): whichever merges second must carry the right count in both rows. Signal: the `check` job on main or on the second PR red at `==> test count` with "says 168 ... counted 170" (or the reverse)
- First CI run of `run: ./checks.sh` on `ubuntu-latest` was not observed by anyone (review-code "What I did not cover"). Signal: the `check` job on PR #6 and on main fails at a step it never ran before (`test count` or `shortcut marker`), or fails to start `./checks.sh`
- Test output is buffered until each `cargo test` ends (review-code finding 1). Signal: a CI job cancelled at its time limit with no test output in the log; fix then is the `tee` plus exit-code file pattern the lens gave
- Temp files left in `/tmp` on Ctrl-C or CI cancel (review-code finding 2), cosmetic. Signal: `tmp.*` files accumulating on dev machines
- A reworded Verification row in `docs/STATUS.md` fails the gate with "no test count found", by design. Signal: that message after a docs-only edit

## What a rollback would take

- command: revert the merge commit on `main`; CI goes back to its five cargo steps
- what it cannot undo: nothing persistent
