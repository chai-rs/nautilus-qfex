DECISION: NEEDS-INFO
TYPE: chore
FR: none
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- The work fits the project and is one pull request: a rule in `AGENTS.md`, a matching `.bloop/issues/README.md`, and one check. No adapter code or test changes
- `issue.md` reserves two choices for a person: the route records take to `main`, and the signal that catches stranded records. The code cannot make either choice, so the work waits on answers
- Post-merge records already have a route in `AGENTS.md` line 21 (`chore/<id>-records`, archive pushed as `main:chore/<id>-archive`). The open part is the records written before the merge: `AGENTS.md` line 20 still sends them to the issue branch, but they are written in the main checkout
- `.bloop/issues/README.md` is byte-identical to the plugin's `templates/issues-README.md`, so editing it here makes it different from the plugin copy. Whether that is allowed is also a person's call
- Risk is medium: the change affects how every later issue's records reach `main`, and the check has to see remote branches and pull requests, which `checks.sh` cannot do offline

## Why

`AGENTS.md:20` says records go on the `bin/work` branch, and `.bloop/issues/README.md` ("Where issue files live") says every issue file is written in the main checkout, where `bin/job` reads it. Both cannot hold together. `issue.md` lists three routes and says "A person decides the route". Its check line ends "or another signal the person prefers". `checks.sh` must run offline (its header says so), so a check on branch age against pull request state means a CI or scheduled workflow under `.github/workflows/`, a different kind of guardrail. Choosing for the person here would bypass the decision `issue.md` reserves.

## Questions

1. Which route do records written before the merge (`intake.md` to `decision.md`) take to `main`? (a) Committed in the main checkout and published through a short `chore/<id>-records` pull request, as post-merge records already are. (b) Copied into the issue's worktree so they ride in the code's pull request. (c) Left for a bloop plugin change outside this repo, with only the contradiction documented here
2. Which signal catches records stranded off `main`? (a) A CI job on pull requests to `main`. (b) A scheduled workflow that lists remote branches holding `.bloop/issues/` commits that are not on `origin/main`. (c) A step `/bloop:retro` or `/bloop:deliver` runs by hand, with no automated check
3. May this repo edit `.bloop/issues/README.md`, which is a verbatim copy of the plugin's `templates/issues-README.md`? If not, `AGENTS.md` alone states the route and the README is left as the plugin ships it
