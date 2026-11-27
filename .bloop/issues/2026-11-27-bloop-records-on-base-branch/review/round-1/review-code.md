VERDICT: PASS

## Findings

Severity 0 to 4. 3 and 4 are [must], 1 and 2 are [should], 0 is not written down.

Load assumed: none applies. The diff is one rule line in `AGENTS.md` and a pure rename of one issue folder; no runtime code, so the scale lens has nothing to judge.

Acceptance criteria re-checked against `origin/main` (read-only git, GitHub API read-only):
AC1 `git show origin/main:AGENTS.md | grep -n "issue's own branch"` prints line 20 · AC2 `git merge-base --is-ancestor origin/claude/bloop-init origin/main` exits 0 · AC3/AC4 `git ls-tree -d origin/main .bloop/archive/2026-11-27-market-data-socket-state` prints one tree, the `.bloop/issues/` path prints nothing · AC5 `"baseBranch": "main"` at config line 44 · AC7 `gh api .../pulls/4` and `/pulls/7` both `merged: true`, `merged_by` a `User`. `354922c` is a pure move: 17 files, 0 insertions, 0 deletions, nothing else in the commit, as `.bloop/issues/README.md` requires.

1. [should] AGENTS.md:20 — the new rule contradicts `.bloop/issues/README.md` ("Where issue files live": "Only in the main checkout ... every file of the issue folder is written here") and cannot hold for records written after the merge. An agent following both files is told to write records in the main checkout but commit them on the `bin/work` branch, which lives in `.worktrees/<id>/`; and `deliver.md`, `retro.md` and the archive move are written after that branch's pull request has merged, so they can never "land with the code in the same pull request". Severity 2: hit on every issue, but nothing is lost, the work just routes around the rule.
   Proof: this issue's own slice 3 did not follow the line — `354922c` landed via `chore/2026-11-27-records-after-merges` (PR #7, with `e832f23` "Record the merges of #4, #5 and #6"), and the main checkout is right now on `chore/2026-11-27-retro-status-test-count-check` holding `0b01b80` (retro) and `93dce31` (archive) for another issue, neither on a `bin/work` branch. The spec saw this tension (Risks, third bullet) but the line does not say what to do with post-merge records or how main-checkout files reach the worktree branch. Fix is one more clause (post-merge records go on a short `chore/<id>` branch and PR) or a note in the README; README is not in this issue's scope to rewrite, so say it in AGENTS.md.
2. [should] .bloop/issues/2026-11-27-bloop-records-on-base-branch/build.md:91 — build.md is stale against what landed: all seven acceptance criteria are still `[ ]` though every one is now verifiable and passes (above), the slice 3 row has a blank line before it that splits it out of the table, and it records the branch as `chore/2026-11-27-records-after-merges` while `spec.md` slice 3 names `chore/2026-11-27-archive-market-data-socket-state` and checks `git ls-tree` on that branch, without saying the plan changed. Severity 1: records only.
   Proof: `sed -n 89,101p build.md` shows the empty line at 90 and the unchecked boxes; `git ls-remote origin chore/2026-11-27-archive-market-data-socket-state` is not the branch PR #7 used (`gh api .../pulls/7 --jq .head.ref` prints `chore/2026-11-27-records-after-merges`).

## Checked from last round

Round 1, nothing earlier.

## What I did not cover

- AC6 as written (`git diff --stat 362bcb0 origin/main -- src tests`) prints 3 files changed today, because PRs #5 and #6 merged code since `362bcb0`; those changes are not from these two commits (`0d3a8ff` touches only `AGENTS.md` and evidence, `354922c` only renames). I did not audit PRs #5/#6, which are other issues.
- I did not run `bin/archive` or `bin/job`, and did not check that every record commit on other in-flight issue branches reaches main; that is the behaviour of the plugin and of future issues, not this diff.
- No test suite run, per instructions. No security lens; no secrets appear in the diff (the evidence file is gate output only).
- `companions.review` is absent from `.bloop/config.json`, so no companion skills applied.
