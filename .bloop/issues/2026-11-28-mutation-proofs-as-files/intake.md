DECISION: ACCEPT
TYPE: test
FR: none
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- Accepted: a mutation cited as proof that a check bites is saved as a patch plus its captured red output under the issue's `evidence/mutations/`, and one script in this repo reruns every saved mutation of an issue on a throwaway copy and fails when one goes green
- The need is real and repeated: four issues proved checks by mutation and kept them only as prose; two people rebuilt the same mutations by hand, and one rebuild was thrown away because it ran another copy's binary
- Every copy builds with a target dir of its own, so the script adds no new way into the stale-binary cause of `2026-11-28-gate-tests-its-own-tree`
- Writing patches after the fact for the four earlier issues is left out; the plan picks the saved mutation that proves the script
- `RISK` medium: a new script that builds and runs the test suite, and may be wired into `./checks.sh`, the gate every pull request goes through

## Why

The records bear the request out. `.bloop/issues/2026-11-27-trade-late-auth-answer/build.md` (rows 1, 2 and r2) describes mutations A, B, M1 and M2 only in prose, and `evidence/` there holds only `slice-*.md` and `round-2*.md`. `review/round-2/review-code.md` names the gap and rebuilt M1 by hand from a `git archive` copy, and `retro.md` row 4 sends the fix to this issue. Neither `evidence/` of `2026-11-27-data-client-reconnect-command` nor of `2026-11-28-trade-socket-control-shape` holds a mutation file, and there is no `scripts/` directory yet. `checks.sh` today runs fmt, clippy, the tests and the test count, nothing about mutations. Three target dirs (`target`, `target-late-auth`, `target-control-shape`) under the main checkout show the cost of a target dir per tree, which the plan has to weigh when it sets the gate time. One script, a file shape and three docs rows (`README.md`, `docs/STATUS.md`, the `gate` row of `ARCHITECTURE.md`) land as one pull request.
