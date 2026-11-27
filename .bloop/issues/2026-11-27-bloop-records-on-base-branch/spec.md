REVIEWER: code
MODEL: default
APPROVED: yes
APPROVED-BY: trustLevel autonomous

# Bloop records live on main

## Summary

- One line in `AGENTS.md` says where bloop records are committed: on the issue's own branch, cut from `origin/main` by `bin/work`, so they reach `main` in the same pull request as the code
- A pull request from `claude/bloop-init` into `main` carries that line and every record commit `main` lacks (21 commits today, merge base `41f3fc9`, no conflict); a person reviews and merges it, no agent pushes to `main`
- After the merge, the main checkout moves from `claude/bloop-init` to `main`, the merged worktree of `2026-11-27-market-data-socket-state` is removed, and `bin/archive 2026-11-27-market-data-socket-state --apply` moves that issue into `.bloop/archive/`, pushed to its own branch and merged by a person
- No adapter code, test, `.bloop/config.json` or plugin script changes; `loop.baseBranch` stays `main`
- Proven by `git merge-tree` and the GitHub pulls API before the merge, then by `git merge-base --is-ancestor` and `bin/job` after it

## Context

Branch names and plugin scripts are written without backticks below: only repo paths are in backticks, so each one opens.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `AGENTS.md` | Agent rules: "Every time", "Never", dependency direction. Says nothing about where bloop records are committed | must change: one line under "Every time" saying records are committed on the issue's own branch, never on a long-lived side branch |
| `.bloop/config.json` | `loop.baseBranch` is main, `git.mergeStyle` is merge, `deliver.autoMerge` all false; no `review` section | read only. It is a protected path (`.claude/hooks/protect-paths.sh`) and the restate keeps the base branch as is |
| `.bloop/issues/README.md` | Says every file of an issue folder is written in the main checkout, and that a closed issue moves once into the archive folder through the plugin's bin/archive, only on the base branch | reuse, do not rewrite: the archive step follows it as written |
| `.bloop/issues/2026-11-27-market-data-socket-state` | Closed issue, bin/job state done. On origin/main only up to its spec, review round 2 and evidence; its deliver.md, retro.md and round-2 decision.md exist only on claude/bloop-init | must move to .bloop/archive/2026-11-27-market-data-socket-state once all its files are on main |
| `.worktrees/2026-11-27-market-data-socket-state` | Worktree on branch feat/2026-11-27-market-data-socket-state at 733509e, clean, already an ancestor of origin/main | must be removed before archiving, which refuses while a worktree for the issue exists |
| plugin script bin/archive | Refuses unless run in the main checkout, on the base branch, with state done, no worktree for the issue, nothing else staged; then git mv and a commit "chore(bloop): archive <id>" | reuse, do not rewrite: run it as is, dry run first, then with --apply |
| plugin script bin/work | Cuts the issue branch from origin of the base branch into the .worktrees folder | unchanged; after the merge it cuts from a main that holds the records, which is the point of the work |
| plugin script bin/ship | Syncs an issue branch onto the base branch and merges | unchanged |
| `ARCHITECTURE.md` and `docs/external/nautilustrader/README.md` | Differ from origin/main only by record commits on claude/bloop-init (socket state line 7, reconnect registry note) | ride along in the pull request; no edit in this issue |
| local branch main | At 4f4ed16, behind origin/main (362bcb0) | must be fast-forwarded to origin/main before the main checkout switches to it |

No outside system is called by the product. The pull request is opened through the GitHub REST API (gh api, repos chai-rs/nautilus-qfex pulls), which is tooling, not a contract the adapter depends on.

**This change has to reach** callers: none · tests: none · fixtures: none · config: none (the base branch stays main) · exports: none

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `AGENTS.md` gains one line under "Every time": bloop records are committed on the issue's own branch, cut from `origin/main` by `bin/work`, and land with its code; committed on `claude/bloop-init` | `grep -n "issue's own branch" AGENTS.md` prints the line, and `git merge-tree --write-tree origin/main claude/bloop-init` exits 0 | todo |
| 2 | Push `claude/bloop-init` and open a pull request from it into `main`, body listing what it carries and asking for a merge commit; a person merges | `gh api 'repos/chai-rs/nautilus-qfex/pulls?head=chai-rs:claude/bloop-init&base=main&state=all' --jq '.[0].base.ref'` prints `main`; after the person merges, `git fetch origin && git merge-base --is-ancestor origin/claude/bloop-init origin/main` exits 0 | todo |
| 3 | Remove the merged worktree, fast-forward local `main` to `origin/main`, switch the main checkout to `main`, run `bin/archive 2026-11-27-market-data-socket-state` then `--apply`, push the commit to `chore/2026-11-27-archive-market-data-socket-state` and open a pull request into `main`; a person merges | `"$CLAUDE_PLUGIN_ROOT/bin/job" 2026-11-27-market-data-socket-state` reports `"archived": true`, and `git ls-tree -d origin/chore/2026-11-27-archive-market-data-socket-state .bloop/archive/2026-11-27-market-data-socket-state` prints one line | todo |

## Acceptance criteria

1. `AGENTS.md` on `main` says bloop records are committed on the issue's own branch, verified by `git show origin/main:AGENTS.md | grep -n "issue's own branch"` after the merge
2. Every commit of `claude/bloop-init` is on `main`, verified by `git merge-base --is-ancestor origin/claude/bloop-init origin/main` exiting 0
3. `origin/main` holds `deliver.md` and `retro.md` of `2026-11-27-market-data-socket-state` (in the issue or archive folder), verified by `git ls-tree -r --name-only origin/main .bloop | grep 2026-11-27-market-data-socket-state/retro.md`
4. `2026-11-27-market-data-socket-state` is archived, verified by `git ls-tree -d origin/main .bloop/archive/2026-11-27-market-data-socket-state` printing one line after the archive pull request merges, and `.bloop/issues/2026-11-27-market-data-socket-state` no longer listed by `git ls-tree -d origin/main .bloop/issues/`
5. `.bloop/config.json` still says `"baseBranch": "main"`, verified by `git show origin/main:.bloop/config.json | grep '"baseBranch": "main"'`
6. No file under `src/` or `tests/` changed, verified by `git diff --stat 362bcb0 origin/main -- src tests` printing nothing
7. No agent pushed to `main`: both merges are pull request merges by a person, verified by `gh api repos/chai-rs/nautilus-qfex/pulls/<n> --jq '.merged_by.login'` naming a person for each

## Risks

- A squash or rebase merge of the pull request would leave `claude/bloop-init` diverged from `main`, and the next merge would replay the same records; costs another cleanup pull request. The body asks for a merge commit, matching `git.mergeStyle: merge`
- Records committed on `claude/bloop-init` after the pull request opens join it automatically; harmless, but if the person merges while another session is writing, a record written after the merge stays behind on `claude/bloop-init` and has to follow in a later pull request
- Bloop writes every issue file in the main checkout (`.bloop/issues/README.md`). Once that checkout is on `main`, a skill that commits records there makes local commits no agent may push; the `AGENTS.md` line has to steer records onto the issue branch, or they pile up unpushed. Costs a confused session, not data
- Switching the main checkout from `claude/bloop-init` to `main` changes the branch every in-flight issue (`2026-11-27-trade-socket-state`, `2026-11-27-data-client-reconnect-command`, `2026-11-27-status-test-count-check`, `2026-11-27-uat-order-path`) writes its records on. If done before the merge, their records would be missing from the checkout; slice 3 only runs after slice 2's merge is confirmed
- The merge is hard to undo once other branches are cut from the new `main`

## Assumptions

- Box 4 Build for this issue commits on `claude/bloop-init`, not on a `bin/work` branch, because the restate names `claude/bloop-init` as the pull request head and a branch cut from `origin/main` has none of the records; this bootstrap is the one exception the new `AGENTS.md` line closes
- The `AGENTS.md` line goes under "Every time" rather than "Never", because it says what to do on each issue
- The archive commit goes through its own branch and pull request, pushed with `git push origin main:chore/2026-11-27-archive-market-data-socket-state`, because `bin/archive` commits on the local base branch and no agent pushes to `main`
- Removing the `2026-11-27-market-data-socket-state` worktree is safe because it is clean and `feat/2026-11-27-market-data-socket-state` (`733509e`) is already an ancestor of `origin/main`; the local branch is kept, the remote branch is left to the person
- The remote branch `origin/claude/subscribe-gate-a9d5d1` is out of scope, because the issue names only `claude/bloop-init`
- `claude/bloop-init` stays after the merge, because deleting a branch other sessions may still have checked out is not asked for
