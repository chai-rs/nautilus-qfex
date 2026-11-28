DECISION: ACCEPT
TYPE: chore
FR: none
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- The person answered the three questions of the first run in session: records written before the merge (b) are copied into the issue's worktree and ride in the code's pull request · the signal (a) is a CI job on pull requests · `.bloop/issues/README.md` may be edited in this repo
- That settles the contradiction between `AGENTS.md:20` (records go on the `bin/work` branch) and the README's "Where issue files live" (every issue file is written in the main checkout): both stay true, the main checkout holds the live copy and the worktree carries a copy into the pull request
- The route also covers project files a skill writes in the main checkout that the code's acceptance criteria depend on, such as `docs/external/` (the `2026-11-27-trade-late-auth-answer` case), not only `.bloop/issues/`
- Post-merge records keep the route `AGENTS.md:21` already gives them (`chore/<id>-records`, archive pushed as `main:chore/<id>-archive`), which worked for `2026-11-27-trade-socket-state` (PR #10)
- One pull request: `AGENTS.md`, `.bloop/issues/README.md` and a job in `.github/workflows/`. No adapter code or tests. Risk stays medium because every later issue's records follow the new route

## Why

`issue.md` reserved the route and the signal for a person, and the person has now chosen among the options it listed, so nothing is left for the requester and the work fits one pull request. The job belongs in CI, not `checks.sh`, because `checks.sh` must run offline (its header says so) and only a `pull_request` run in `.github/workflows/ci.yml` knows the head branch and what the pull request carries. One thing for box 3 Planning to settle: after the code merges, the main checkout still holds untracked copies of the same record files, which can block `git pull` on `main`, so the route has to say how those copies are dropped or committed.
