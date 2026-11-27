STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `55d4a84` | `./checks.sh` with the rows as written, as 167, as 169, with no number; a throwaway failing test | red first: with 167 written the old gate exited 0 and had no `test count` step. After: as written exit 0 (`168 passed, as written` twice); 167 → exit 1 naming the row, 167, 168; 169 → exit 1 naming the row, 169, 168; no number → exit 1 `no test count found`; a failing test stopped the gate at `unit + integration` (exit 1) before counting, test removed | `evidence/slice-1.md` (PASS; the four cases and the failing test ran by hand, output in the session) |
| 2 | `80b1590` | the `python3` yaml check of `spec.md`; `git diff origin/main -- .github/workflows/ci.yml`; `./checks.sh --e2e` | red first: the check job listed five cargo `run:` steps. After: `['./checks.sh']` and `5` for fuzz; the diff touches only the check job; gate exit 0, 168 passed | `evidence/slice-2.md` (PASS) |

## Acceptance criteria

- [x] 1 the gate prints `test count` naming both rows with 168 — `evidence/slice-1.md`
- [x] 2 examples row at 167 fails, naming row, 167, 168 — slice 1 run by hand (output quoted in the row above and in the PR)
- [x] 3 plain row at 169 fails, naming row, 169, 168 — same
- [x] 4 number removed fails with "no test count found" — same
- [x] 5 a failing `cargo test` stops the gate at the test step, before counting — throwaway test run, exit 1 at `==> unit + integration`; `checks.sh` uses `|| { cat ..; exit 1; }`, no pipe into `tee`
- [x] 6 the count step uses only `awk`, `sed`, `mktemp` and local files — read in `checks.sh` at `80b1590`
- [x] 7 the check job has one `run:` step, `./checks.sh`, and keeps checkout, toolchain 1.99.1 with `rustfmt, clippy`, rust-cache — yaml check and diff at `80b1590`
- [x] 8 the fuzz job is unchanged — `git diff origin/main -- .github/workflows/ci.yml` touches only check-job lines
- [x] 9 `docs/STATUS.md` says the gate checks both counts and the check job runs `./checks.sh` — rows `CI workflow`, `CI`, section `### CI` and the sentence under the Verification table
- [x] 10 no Rust test added or removed — `git diff --stat origin/main -- src tests bin` is empty; count stays 168

## Notes for reviewers

- Worth a closer look: the two `sed -nE` patterns that read the rows of `docs/STATUS.md`. A reworded row makes the gate fail with "no test count found", on purpose
- Worth a closer look: CI now gets the gate's exact steps, including the `shortcut:` marker check, which it did not run before
- Merge order with PR #5: #5 changes the count to 170. Whichever merges second must carry the right number; this new step fails the other one otherwise
- The four failure cases were run by hand and their output is quoted, not kept as evidence files: `bin/gate` writes one run per file and each case needs `docs/STATUS.md` edited and restored around it
- `clean-comment` ran inline (two "why" comments in `checks.sh`)
- `MUST KILL` flags: none. `shortcut:` markers: none
- PR: https://github.com/chai-rs/nautilus-qfex/pull/6 (draft)
