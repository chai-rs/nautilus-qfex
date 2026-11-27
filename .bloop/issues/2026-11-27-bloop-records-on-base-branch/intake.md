DECISION: ACCEPT
TYPE: chore
FR: none
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- The bloop records and `checks.sh` live on `claude/bloop-init`, while `loop.baseBranch` in `.bloop/config.json` is `main`
- `origin/main` stops at `362bcb0`; five record commits (`018594b` through `413bdd1`) are only on `claude/bloop-init`
- The work is worth doing: the split cost three detours in `2026-11-27-market-data-socket-state` and blocks its archive
- The person chose (a) on 2026-11-27: merge `claude/bloop-init` into `main` through a pull request they merge, and keep `loop.baseBranch: main`
- Once chosen, the work is one small change plus `bin/archive` for `2026-11-27-market-data-socket-state`, with no adapter code touched

## Why

`issue.md` says in so many words that a person decides how the records reach the base, and `AGENTS.md` plus the issue forbid agents from pushing to `main`, so neither the code nor an agent can settle it. The two readings lead to different work: merging `claude/bloop-init` into `main` needs a person to merge a pull request and then a rule for how later records reach `main`, while setting `loop.baseBranch` to `claude/bloop-init` is a one-line change to `.bloop/config.json` that makes `bin/work`, `bin/ship` and `bin/archive` (all of which read `loop.baseBranch`) cut, land and archive against `claude/bloop-init`. Risk is medium because the choice decides where every later issue branch is cut from and merged into, and a merge into `main` is hard to undo.
