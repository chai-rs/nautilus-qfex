CONFIRMED: no

# One route for issue records to main

## What we will do

- Records an issue gets before its code merges (intake to decision) are still written in the main checkout, where `bin/job`, the board and `/bloop:auto` read them. Before each push of the issue branch, they are copied into the issue's worktree and committed there, so they reach `main` in the same pull request as the code.
- The same goes for project files a skill writes in the main checkout that the code's acceptance criteria rely on, such as rows in `docs/external/`: they travel in the code's pull request, so the base never lacks them while the code is ready to merge.
- Records written after the merge (deliver, retro, the archive move) keep the route they already have: a short `chore/<id>-records` branch, or `main:chore/<id>-archive`, through a pull request a person merges.
- The main checkout ends up clean after the merge: the steps say what to do with its leftover copies so pulling `main` does not trip on them.
- A CI job runs on pull requests to `main`. A pull request from an issue branch fails it when the branch does not carry that issue's records written so far.
- `AGENTS.md` and `.bloop/issues/README.md` state the same route in the same words, and no rule in either can be followed only by breaking the other.

## What we will not do

- Change the bloop plugin or its scripts, or `.bloop/config.json` (a protected path): the route works with `bin/work`, `bin/job` and `bin/archive` as they are.
- Add the check to `checks.sh`: it must run offline and cannot see pull requests.
- A scheduled scan of remote branches for stranded records: the person chose the pull request job instead.
- Touch adapter code or tests, or push to `main`.

## Contracts affected

none
