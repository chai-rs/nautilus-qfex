FR: none
KIND: request
GOAL: none

The bloop issue records and tooling live on the branch that `loop.baseBranch` names, so an issue branch, its records and its archive all agree on one base.

## Where it comes from

- Retro of `2026-11-27-market-data-socket-state`. `loop.baseBranch` is `main`, but `checks.sh` and the issue records were committed on `claude/bloop-init`. The same cause showed up three times in that one issue:
  - build had to cut the issue branch from `claude/bloop-init`, not `main`, and merging it into `main` also landed the bloop setup commits (`build.md` notes, `deliver.md`)
  - review round 2 found `docs/STATUS.md` pointing at `.bloop/issues/2026-11-27-data-client-reconnect-command`, which was not on the issue branch (`review/round-2/review-code.md` finding 1); fixed by a rebase onto `claude/bloop-init`
  - the retro could not archive the issue folder: `bin/archive` runs only in the main checkout on `loop.baseBranch`, and the records are not on `main`
- `362bcb0` brought `claude/bloop-init` up to `41f3fc9` into `main`; the later records (`3213378` and after) are still only on `claude/bloop-init`.

## What is asked

- A person decides how the records reach the base: merge `claude/bloop-init` into `main` and keep committing records there, or set `loop.baseBranch` to the branch that holds them. Agents never push to `main`.
- Once decided, archive `2026-11-27-market-data-socket-state` with `bin/archive`.

## Constraints

- No change to the adapter's code or tests.
