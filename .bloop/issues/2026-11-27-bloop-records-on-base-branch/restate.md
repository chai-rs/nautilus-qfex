CONFIRMED: yes
CONFIRMED-BY: the person, in session, answering "1"

# Bloop records live on main

## What we will do

- Open a pull request from `claude/bloop-init` into `main` carrying every record and tooling commit `main` does not have yet. The person reviews and merges it; no agent pushes to `main`.
- After it merges, `main` holds the issue records, `checks.sh` and the bloop config, and `loop.baseBranch` stays `main`.
- From then on, issue branches are cut from `origin/main` by `bin/work`, and an issue's records are committed on its own issue branch so they land on `main` with the code in the same pull request.
- Once on `main`, `bin/archive 2026-11-27-market-data-socket-state --apply` moves the closed issue into the archive, in a pull request the person merges.
- `AGENTS.md` gets one line saying where records are committed, so later sessions do not split them again.

## What we will not do

- Change `loop.baseBranch` or any bloop plugin script.
- Touch adapter code or tests.
- Push to `main` or merge any pull request: a person merges.

## Contracts affected

none
