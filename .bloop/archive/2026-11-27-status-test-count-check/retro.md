REPEATS: 1

## What happened

Short. The record is the table below, not this part.

- review rounds: 1. Code PASS with two `[should]` (test output buffered until each `cargo test` ends; temp files left on a signal), no `[must]`. Approver APPROVE after rerunning criteria 1 to 3 itself
- the gate missed: 0 `[must]`. `build.md` has no `## Why the gate missed this` section
- lead time: 0 days, issue date and `MERGED-AT` both 2026-11-27 (PR #6, merge `fda6185`, merged by the agent at the person's request in session)
- scope changed once after intake (`RESCOPED: yes`): the restate claimed CI ran `./checks.sh`; it did not, and the person chose to make the `check` job of `ci.yml` call it (answer "1", `auto.md` 14:34Z)
- watch after deploy, read on GitHub (read only): the `check` job ran `./checks.sh` on `main` for `fda6185` (run `46023650252`, `test count` 168 as written) and for `f9b4079` after PR #5 (run `46023835346`, 170 as written). The merge-order risk with PR #5 did not bite: #5 already carried 170 in both rows

## Lessons into guardrails

One row per problem. **A row with no `what was added` is not a lesson, it is a complaint.**

The ladder, in order of value: fix the architecture > add a lint rule or a test > add a skill > leave it to a human.

| problem | times seen | fixed at which rung | what was added |
| --- | --- | --- | --- |
| A hand-written test count in `docs/STATUS.md` or a spec did not match what `cargo test` ran (162 written, 166 real; 167 written, 168 real, both in `2026-11-27-market-data-socket-state`) | 2 | test | issue `2026-11-27-status-test-count-check` (this one, merged as `fda6185`): `checks.sh` step `test count` sums the `N passed` of each `cargo test` run and fails when the `cargo test --features examples` row or the `cargo test` row of `docs/STATUS.md` says another number, or no number. It ran in CI for both 168 and 170 |
| CI and the gate were two separate lists of steps: `ci.yml` repeated five cargo commands instead of calling `./checks.sh`, so a new gate step would not have run in CI, and the restate assumed it would | 1 | architecture | `.github/workflows/ci.yml`: the `check` job has one `run:` step, `./checks.sh` (`80b1590`); the header of `checks.sh` says so. In this retro, the `gate` row of `ARCHITECTURE.md` now names `./checks.sh` and its steps instead of "the same steps as `ci.yml`" |
| The docs said the CI workflow "never ran on GitHub" and no fuzz target ever ran, while `ci.yml` had run on every push since 12 Nov 2026 and the `fuzz` job had run all six targets; review and deliver then listed the first CI run of `./checks.sh` as unobserved, and nobody ran `gh run list` | 1 | human | In this retro: `README.md` (rows `CI workflow`, `Fuzz package`), `docs/STATUS.md` (rows `CI workflow`, `Fuzz package`, `CI`, section `## Fuzz`) and `ARCHITECTURE.md` (G3 measure source) now state a dated observation with its run id (`46026490835`, `46023650252`) instead of "never". No offline check can read GitHub, so keeping it true stays with whoever changes CI |

A problem seen twice or more gets an issue of its own through `/bloop:backlog`, and the row says its id. `REPEATS` counts those rows, and box 6 does not close while one of them has no issue.

## Numbers

From `bin/metrics`, not counted by hand. The "this issue" column is read from this folder (`review/round-1` only, `decision.md`, `deliver.md`).

| metric | this issue | last 30 days |
| --- | --- | --- |
| lead time | 0 days | 0.0 days median |
| review rounds | 1 | 1.33 mean |
| escalate rate | no | 17% (`escalateRate` 0.1667) |
| change failure rate | no | 0% |
| gate misses (`[must]`) | 0 | 0 |

`bin/metrics` also says 2 delivered issues have no `retro.md`: this one until now, and `2026-11-27-data-client-reconnect-command`.

## Diagrams to update

The picture is wrong the moment the code lands and nobody redraws it.

- `ARCHITECTURE.md`, `## Tech stack` row `gate`: said the gate was the cargo steps, "the same steps as `ci.yml`". Now names `./checks.sh`, its steps including `test count` and `shortcut:` markers, and that the `check` job runs it. Done in this retro
- `ARCHITECTURE.md`, goal G3 source: said `ci.yml` "has not run on GitHub yet". Now says the `check` job runs `./checks.sh` since `fda6185`. Done in this retro
- Handdraw board: none, `handdraw.board` is not set in `.bloop/config.json`

## What was left undone

Anything this issue chose not to do, so the next person does not read the silence as completeness.

- Review finding 1 `[should]`: each `cargo test` in `checks.sh` writes to a temp file and prints only when it ends, so a hung test in CI shows no output before the job times out. Approved as is; the fix the lens gave (`tee` plus an exit-code file) is not written. Not seen in any CI run so far
- Review finding 2 `[should]`: `trap ... EXIT` does not run on INT or TERM under dash, so a cancelled gate leaves two files in `/tmp`. Cosmetic, approved as is, not fixed
- The proof that the `test count` step fails (167, 169, no number) was run by hand and lives only in session output and in `decision.md`; nothing reruns it. A later change to the two `sed -nE` patterns that made them match the wrong number would not be caught by the gate itself
- The `cargo test` row must read `The same <N>`; a test that exists only under `--features examples` would make the two counts differ and need a new row wording (review "What I did not cover"). No such test exists today
- Other numbers in `docs/STATUS.md` and `README.md`, such as the per-binary breakdown of the count, are not checked: out of scope in `restate.md`
- Product docs: `bin/docs` reports that `docs/product` does not exist, so nothing was refreshed
