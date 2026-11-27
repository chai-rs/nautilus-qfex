CONFIRMED: no

# The gate checks the test count written in STATUS.md

## What we will do

- When `./checks.sh` runs the tests, it adds up how many passed in each run and compares that total with the number written for that run in the Verification table of `docs/STATUS.md`
- Both test runs the gate makes are checked: the run with the `examples` feature against the `cargo test --features examples` row, and the run without it against the `cargo test` row, whose cell reads `The same N`
- If a number differs, the gate stops with a failure that names the row, the number written and the number counted, so the fix is to correct the row
- If the number cannot be found in the row at all, the gate fails too, rather than passing silently
- Proof that it bites: with a wrong number in `docs/STATUS.md` the gate fails, with the right one it passes
- The step needs no network, like the rest of the gate

## What we will not do

- Check any other number in `docs/STATUS.md` or `README.md`, such as the per-binary breakdown in the same cell (unit, `qfex-flatten`, ...): only the total is asked for
- Write the count into `docs/STATUS.md` automatically: the gate checks, a person or agent edits
- Touch `claude/bloop-init`'s copy of `docs/STATUS.md`: the work lands on a branch from `main`, where the count is already right (168)
- Change the CI workflow file: CI already calls `./checks.sh`

## Contracts affected

none
