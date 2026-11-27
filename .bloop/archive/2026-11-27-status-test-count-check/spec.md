REVIEWER: code
MODEL: default
APPROVED: yes
APPROVED-BY: trustLevel autonomous

# The gate checks the test count written in STATUS.md

## Summary

- A new `test count` step in `checks.sh` keeps the stdout of `cargo test` and of `cargo test --features examples`, sums the `N passed` of every `test result:` line of each run, and compares each sum with its own row of the Verification table of `docs/STATUS.md`: `168 passed, ...` for the `--features examples` row, `The same 168` for the `cargo test` row
- A differing number, or a row where no number can be found, fails the gate with a message naming the row, the number written and the number counted. Offline, `/bin/sh` (dash) and `awk` only, as the rest of `checks.sh`
- The `check` job of `.github/workflows/ci.yml` runs `./checks.sh` instead of repeating its five cargo steps, so the count check runs in CI too; the `fuzz` job stays as it is
- Proven by running `./checks.sh` with the right counts (passes) and with each row made wrong or emptied (fails, naming both numbers); a throwaway prototype on `origin/main` already showed both runs sum to 168
- Two slices: the gate step, then CI calling the gate with the docs that describe CI

## Context

Built on a branch cut from origin/main (`362bcb0`). Line numbers are those of origin/main; the same files exist in this checkout, where `docs/STATUS.md` still says 162.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `checks.sh` | The one gate, a POSIX sh script (dash here) with `set -eu`, a `step()` helper, `bloop:managed checks` markers. Step `unit + integration` (29-31) runs `cargo test` then `cargo test --features examples` with output straight to the terminal. Step `shortcut marker` (35-51) shows the house style for a failing check: build a message, `printf ... >&2`, `exit 1`. Header (5) says it runs the same steps as the `check` job of `.github/workflows/ci.yml` | must change: each `cargo test` keeps its stdout in a temp file (`mktemp`, removed on exit) and prints it, without losing the exit code (dash has no `pipefail`, so no plain pipe into `tee`); a new step `test count` after it sums and compares. Reuse `step()` and the `shortcut marker` failure style, do not rewrite them. Header comment says `.github/workflows/ci.yml` calls this file (slice 2) |
| `docs/STATUS.md` | Verification table: row `cargo test --features examples` (31) starts `168 passed, 0 failed: 142 unit, ...` and goes on with older counts (`166 at ...`); row `cargo test` (no feature, as in CI) (32) reads `The same 168`. Row `CI workflow` (89), row `CI` of "Tooling around the adapter" (138) and section `### CI` (169-175) describe the `check` job as its own list of cargo steps | read by the new step: the first `<N> passed` of the cell of row 31, and `The same <N>` of row 32. Must change: one sentence under the Verification table saying the gate checks these two numbers (slice 1); rows 89 and 138 and `### CI` say the `check` job runs `./checks.sh` (slice 2). The numbers themselves stay 168 |
| `.github/workflows/ci.yml` | Job `check` (6-18): checkout, the dtolnay rust-toolchain action at 1.99.1 with `rustfmt, clippy`, the Swatinem rust-cache action, then five `run:` lines that repeat `checks.sh` by hand (no `shortcut marker`, no count). Job `fuzz` (19-33) | must change: the five `run:` lines become one `run: ./checks.sh`; checkout, toolchain and cache stay. `fuzz` is not touched |
| `.github/workflows/audit.yml` | Dependency audit, kept out of `checks.sh` because it needs the network (comment line 2) | no change; example of what stays out of the gate |
| `README.md` | Line 128: `CI workflow` row, "fmt, clippy, tests, 30 s per fuzz target" | stays true; no change |
| `ARCHITECTURE.md` | G3 (23) measures the gate by test count; the `gate` row (112) says `checks.sh` runs the same steps as `.github/workflows/ci.yml` | stays true once `.github/workflows/ci.yml` calls `checks.sh`; no change |
| `AGENTS.md` | "Keep `README.md` and `docs/STATUS.md` true: a change in ... the test count updates them in the same branch" (18) | the rule this step enforces; no change |
| `.bloop/archive/2026-11-27-market-data-socket-state/retro.md` | Where the count slipped twice in one issue | why; read only |
| `rust-toolchain.toml` | Pins Rust 1.99.1, the toolchain whose `cargo test` output format the step parses | read only |

Touches no contract we give and no other system's contract: the only outside format read is the `test result:` line of the pinned toolchain's own test harness.

**This change has to reach** callers: `.github/workflows/ci.yml` job `check`; every `./checks.sh` run (each slice in box 4 Build, bloop's gate script) · tests: none added in Rust; the proof runs `./checks.sh` against a wrong `docs/STATUS.md` · fixtures: none · config: `.github/workflows/ci.yml` · exports: none

## Prototype

- Question to answer: on `origin/main` (`362bcb0`), do the `N passed` of the `test result:` lines of `cargo test` and of `cargo test --features examples` each sum to the 168 written in `docs/STATUS.md`, and are those lines on stdout?
- What was tried: a detached throwaway worktree of `origin/main` in the scratchpad, both runs with `--offline` and stdout and stderr in separate files, an `awk` that adds the field before `passed;` on each `^test result:` line, and an `awk -F'|'` that reads the two rows of `docs/STATUS.md`
- Result: yes. 7 `test result:` lines per run, all on stdout (none on stderr), the doc-tests line included as `0 passed`; both sums are 168. The rows read back as 168 (`--features examples`, first `<N> passed` of the cell) and 168 (`cargo test`, `The same <N>`). `/bin/sh` here is dash. The plan above holds unchanged
- Code thrown away, never merged

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `checks.sh`: both `cargo test` runs keep their stdout without losing the exit code, new step `test count` compares each sum with its row of `docs/STATUS.md`, failing with row, written and counted numbers, and failing when a row has no number; one sentence under the Verification table of `docs/STATUS.md` says the gate checks these two counts | `./checks.sh` exits 0 and prints `==> test count` with 168 for both rows; then, one at a time with a restore in between, `sed -i 's/ 168 passed, 0 failed/ 167 passed, 0 failed/' docs/STATUS.md`, `sed -i 's/The same 168/The same 169/' docs/STATUS.md` and `sed -i 's/The same 168/The same/' docs/STATUS.md` each make `./checks.sh` exit non-zero with a message holding the row and, for the first two, `167` / `169` and `168`; `git diff --exit-code docs/STATUS.md` is clean after the restore | done |
| 2 | `.github/workflows/ci.yml` job `check` runs `./checks.sh` in place of its five cargo steps; `checks.sh` header says `ci.yml` calls it; `docs/STATUS.md` rows `CI workflow`, `CI` and `### CI` say the `check` job runs `./checks.sh` | `python3 -c "import yaml; j = yaml.safe_load(open('.github/workflows/ci.yml'))['jobs']; print([s.get('run') for s in j['check']['steps'] if 'run' in s]); print(len(j['fuzz']['steps']))"` prints `['./checks.sh']` and `5`; `git diff origin/main -- .github/workflows/ci.yml` shows no line of job `fuzz` changed; `./checks.sh` exits 0 | done |

## Acceptance criteria

1. With `docs/STATUS.md` as on `origin/main`, the gate prints a `test count` step that names both rows with 168 written and 168 counted, verified by `./checks.sh 2>&1 | sed -n '/==> test count/,/==>/p'`
2. With the `cargo test --features examples` row changed from `168 passed` to `167 passed`, `./checks.sh` exits non-zero and its output names that row, 167 and 168, verified by the first `sed` of slice 1, then `./checks.sh; echo $?`
3. With the `cargo test` row changed to `The same 169`, `./checks.sh` exits non-zero and its output names that row, 169 and 168, verified the same way with the second `sed` of slice 1
4. With the number removed from the `cargo test` row (`The same`), `./checks.sh` exits non-zero and says no count was found in that row, verified with the third `sed` of slice 1
5. A failing `cargo test` still fails the gate at the test step, before the count step, verified by reading `checks.sh`: each `cargo test` exit code reaches `set -e` (no unguarded pipe into `tee`), and by a throwaway `assert!(false)` test making `./checks.sh` exit non-zero at `==> unit + integration`, then removed
6. The count step reads only local files and runs no new command beyond `cargo test`, `awk`, `sed`, `grep` and `mktemp`, verified by reading the `test count` step of `checks.sh`
7. The `check` job of `.github/workflows/ci.yml` has exactly one `run:` step, `./checks.sh`, and keeps checkout, `dtolnay/rust-toolchain@1.99.1` with `rustfmt, clippy` and `Swatinem/rust-cache@v2`, verified by the `python3` command of slice 2
8. The `fuzz` job is unchanged, verified by `git diff origin/main -- .github/workflows/ci.yml` touching only lines of job `check`
9. `docs/STATUS.md` says the gate checks the two counts and that the `check` job runs `./checks.sh`, verified by `grep -n "checks.sh" docs/STATUS.md`
10. The test count stays 168 (or the 170 of PR #5 if that merged first): no Rust test is added or removed, verified by `git diff --stat origin/main` listing no file under `src/`, `tests/` or `bin/`

## Risks

- `checks.sh` is the single gate for every slice and for CI. A parsing bug either blocks all work (false failure) or lets a wrong count through (false pass); criteria 1 to 4 cover both directions, and a missing number fails instead of passing
- Losing the exit code of `cargo test` when keeping its output: dash has no `pipefail`, so `cargo test | tee` would turn a failing test into a pass. Slice 1 must write stdout to a file and check the exit code, criterion 5 guards it
- PR #5 (`feat/2026-11-27-data-client-reconnect-command`) sets both rows to 170. Whichever merges second rebases onto the other; the new step makes a wrong number fail its gate, which is the point. Textual overlap is small: PR #5 edits rows 31, 32 and 77 of `docs/STATUS.md`, this work edits the lines under the table and rows 89, 138 and `### CI`. A sentence placed right under the table may still conflict with PR #5's row 32 edit when rebasing; resolve by keeping both
- `docs/STATUS.md` on `claude/bloop-init` says 162, so running the new `checks.sh` on that branch fails until it carries `main`'s code; nobody runs the gate there for adapter work, but `bin/gate` on that branch would go red
- `checks.sh` sits between `bloop:managed checks` markers: a later `/bloop:guardrails` rerun replaces the whole block and could drop the step. The step is written inside the block like the existing `shortcut marker` step, so a rerun that keeps project steps keeps it; a rerun from the bare template would drop it, and review of that rerun has to catch it
- The `--features examples` cell holds many numbers (`142 unit`, `166 at ...`); reading anything but the first `<N> passed` would compare the wrong one. The prototype read 168 with that rule
- Keeping stdout in a file delays the test output until the run ends; a slow test run shows nothing for a while. stderr (compile progress, `Running ...`) still streams
- With CI calling `checks.sh`, CI now also runs the `shortcut marker` step; a marker that passes locally passes there, since the same file runs

## Assumptions

- Both rows are checked, each against its own run, because restate confirmed it; the per-binary breakdown in the same cell is not checked
- The count is the sum of every `test result:` line of a run, doc-tests included (`0 passed` today), because that is what `cargo test` reports and what the 168 in `docs/STATUS.md` already counts
- The proof that it bites is a run of `./checks.sh` against a wrong `docs/STATUS.md`, recorded as evidence in box 4 Build, not a permanent self-test of the gate, because restate asks to show it bites and a second copy of the parser to test the first would add code without covering more
- The step lives in `checks.sh` itself, not in a separate script under `scripts/`, because the gate is one file by design (`checks.sh` header) and the step is a few lines of `awk`
- `AGENTS.md` and `README.md` are not edited: their sentences stay true, and restate did not ask to change them
- No `docs/external/` entry for the libtest output format: it is the pinned toolchain's own output, not another system that can go down; a toolchain bump that changed it would fail the gate loudly (no number found), not pass
- `nautilus-risex` and `nautilus-lighter` sources are not on this machine, so no file was followed from them; the `ci.yml` change follows the bloop template (`checks.sh` header: "Dev machines and CI call this same file")
- The baseline is 168 at `362bcb0`, read from `origin/main` and confirmed by the prototype
