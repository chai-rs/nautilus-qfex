CONFIRMED: no

# Mutation proofs are saved as files and can be rerun

## What we will do

- When build, a review lens or the approver proves a check bites by breaking the code, that broken change is kept under the issue's `evidence/mutations/`: the change itself as a patch, the commit it was made against, the command that was run, and the red output that command gave. The plan fixes the exact file names and layout
- One command in this repo, given an issue id, takes every saved mutation of that issue and, for each one, makes a fresh copy of the tree outside the working tree, applies the change there, builds it with a target dir used by that copy alone, and runs the command saved with it, offline
- The command fails when any mutation now passes, and names it: a check that used to catch the change has stopped catching it
- A change that no longer applies is reported by name as not applying, never counted as caught; the run does not end green while one is left in that state
- The branch being checked is never edited, no copy shares a target dir with the branch or with another copy, and the copies are removed afterwards
- At least one real saved mutation proves the command end to end: red when the check catches it, a named failure when it goes green, a named report when it does not apply
- The plan decides whether `./checks.sh` runs the command, for which issues, and records what it adds to gate time on this machine
- `README.md`, `docs/STATUS.md` and the `gate` row of `ARCHITECTURE.md` say what exists and how to run it, and the test count stays true

## What we will not do

- Turning the prose mutations of `2026-11-27-trade-late-auth-answer`, `2026-11-27-market-data-socket-state`, `2026-11-27-data-client-reconnect-command` and `2026-11-28-trade-socket-control-shape` into saved patches: they ran against code that has since moved, and the issue asks for the shape and the rerun, not a backfill. The plan may reuse one of them as the mutation that proves the command
- Changes to `bin/gate` or the `evidence-pack` skill: they belong to the bloop plugin
- Generating mutations automatically (a mutation-testing tool): only mutations a person or agent chose and cited are saved and rerun
- A shared or cached target dir across copies to save build time: it is the stale-binary cause of `2026-11-28-gate-tests-its-own-tree`
- Any contact with the live QFEX venue: every saved command runs against the local stand-ins on `127.0.0.1`

## Contracts affected

none
