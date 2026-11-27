FR: none
KIND: request
GOAL: none

`./checks.sh` fails when the test count written in `docs/STATUS.md` differs from what `cargo test` reports, so nobody plans against a stale count again.

## Where it comes from

- Retro of `2026-11-27-market-data-socket-state`. A hand-written test count was wrong twice in that one issue:
  - the request said 162, read from `docs/STATUS.md`, which `0e8ed08` did not update when it added 4 tests (166 at `4f4ed16`)
  - the spec's acceptance criterion 6 said 167; slice 1 added a second test, so the real count was 168
- `AGENTS.md` already asks that a change in the test count updates `docs/STATUS.md` in the same branch. Nothing checks it.

## What is asked

- A step in `checks.sh` that sums the `N passed` of every `test result:` line of `cargo test` and compares it with the count in the `cargo test` row of the Verification table of `docs/STATUS.md`. A mismatch fails the gate with both numbers.
- Show it bites: a wrong number in `docs/STATUS.md` fails `./checks.sh`, the right one passes.

## Constraints

- Cut the branch from `main` (at `362bcb0` or later). `docs/STATUS.md` on `claude/bloop-init` still says 162, so the check would fail there until that branch carries `main`'s code.
- The check runs offline, like the rest of `checks.sh`.
