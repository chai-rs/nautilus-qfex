FR: none
KIND: request
GOAL: none

A mutation run that build, a review lens or the approver cites as proof that a check bites is kept as a file under the issue's `evidence/`, with the change it made and the output it gave, and one command reruns every saved mutation of an issue on a throwaway copy and fails when one of them goes green.

## Where it comes from

- Retro of `2026-11-27-trade-late-auth-answer`. Its checks were proven by five mutations (A and B in round 1, M1 and M2 in round 2, and the approver's variant with cancel on disconnect turned on), and every one of them lives only as prose in `build.md` and `decision.md`. The round 2 code lens wrote it down (`review/round-2/review-code.md`, "What I did not cover", second bullet: "The mutation results ... are recorded only as prose in `build.md`, not as captured output under `evidence/`. Round 1 used the same convention"). The code lens then rebuilt M1 by hand from that prose (6 runs), and the approver rebuilt M1 and M2 by hand again. The approver's first rebuild shared one target dir between two copies, so its results came from another copy's binary and were thrown away (`decision.md`, "Spot check")
- The same shape in three more issues, each with no mutation file under `evidence/`:
  - `2026-11-27-market-data-socket-state` (archived), `build.md`, acceptance criteria: "Mutation run: with the sink removed from `Feed::close`, the test fails"
  - `2026-11-27-data-client-reconnect-command`, `build.md`, criterion 5: "Mutation run: with `Feed::close` building the socket without `register`, ..."
  - `2026-11-28-trade-socket-control-shape`, `decision.md`, "Spot check": the approver's mutation of `QfexExecutionClient::new`, described in a sentence

## What is asked

- A place and a shape for a saved mutation, for example `evidence/mutations/<name>.patch` (a diff against the commit it was run on) and `evidence/mutations/<name>.txt` (the command and its captured output, red). Decide in restate and plan
- A script in this repo, for example `scripts/mutants.sh <issue-id>`, that applies each saved patch to a `git archive` copy in a temporary directory, builds it with a target dir of its own, runs the command saved with it, and fails when any of them passes. A patch that no longer applies is reported as such, not as a pass
- Whether `./checks.sh` runs it, for which issues, and what that costs in gate time. Decide in plan
- `README.md`, `docs/STATUS.md` and `ARCHITECTURE.md` (the `gate` row) stay true

## Constraints

- No contact with the live QFEX venue. Every mutation runs against the local stand-ins on `127.0.0.1`, offline
- `bin/gate` and the `evidence-pack` skill belong to the bloop plugin and are out of reach here; the change lives in this repo
- A copy never shares a target dir with another tree. That is the root of `2026-11-28-gate-tests-its-own-tree`; this issue must not add a new way to hit it
- The source tree of the branch is never edited to run a mutation
