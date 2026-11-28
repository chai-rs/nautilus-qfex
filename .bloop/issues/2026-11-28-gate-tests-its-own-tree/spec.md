REVIEWER: code
MODEL: default
APPROVED: no

# The gate tests its own tree and says which tree that was

## Summary

- `./checks.sh` gets a step `own build` that runs `cargo clean -p nautilus-qfex` before clippy and the tests, so the lib, test, bin and example binaries it runs are always compiled from the files of the tree it runs in; the compiled dependencies in the target dir stay
- `./checks.sh` gets a first step `tree` that prints one line, `tested tree: <hash> ...`: the git tree of the working files (tracked, changed and untracked not ignored), with `.bloop/` taken as committed at `HEAD`, built in a temporary index so the real index and the working tree are not touched. A code commit made right after the gate on that `HEAD` has exactly that tree, so review compares the line with `git rev-parse <commit>^{tree}`
- Outside the top of a git work tree (a `git archive` copy, or a copy nested inside another checkout) the line reads `tested tree: none, ...` and the gate goes on
- Measured on this machine (4 cores, Linux), with the dependencies already compiled: a gate run with no source change goes from 11 s to 55 to 57 s; a gate run after a source change, the usual case in box 4 Build, goes from 54 s to 55 to 57 s. CI already rebuilds the crate on every run, so it costs about nothing there
- Proven by two shell recipes on throwaway clones (one sharing a target dir with a panicking mutant, one committing code next to an uncommitted record), then by the `check` job of `ci.yml` on the pull request. Two slices, one pull request

## Context

Line numbers are those of origin/main at `1c52db6`.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `checks.sh` | POSIX `sh` (shebang #!/bin/sh, `dash` on this machine and on the `ubuntu-latest` runner), `set -eu`; `step()` (16) prints `==> <name>`; steps `format` (18), `lint` (21-23: clippy without and with `--features examples`), `unit + integration` (28-35), `test count` (37-55), `shortcut marker` (59-75) | must change: a new first step `tree` that prints the line, and a new step `own build` (`cargo clean -p nautilus-qfex`) between `format` and `lint`. `step()` is reused, do not rewrite: the plugin's bin/gate lists the steps from the `==> ` lines. The other steps stay as they are |
| `Cargo.toml` | one package, `nautilus-qfex` (lib `nautilus_qfex`), no build script, no workspace members | the package `cargo clean -p` names. `fuzz/` is a separate workspace that `checks.sh` does not build |
| `.github/workflows/ci.yml` | job `check`: actions/checkout@v4, dtolnay/rust-toolchain@1.99.1, Swatinem/rust-cache@v2, then `./checks.sh` (14) | no change: the same script runs there. The checkout is a git work tree with a detached `HEAD` (the pushed commit, or the merge commit on `pull_request`), so the line names a tree |
| `.gitignore` | ignores /target, `.worktrees/`, /fuzz/target and others | reused by `git add -A` in the temporary index, so the tree line leaves ignored files out. The extra `target-*` dirs of the main checkout are ignored only through `.git/info/exclude` on this machine, which `git add` also honours |
| `ARCHITECTURE.md` | `gate` row of the stack table (112) lists the steps of `./checks.sh` | must change: names the `tree` and `own build` steps and why |
| `README.md` | `CI workflow` row (128) says the `check` job runs `./checks.sh` | must change: says the gate prints the tree it tested and rebuilds the crate each run |
| `docs/STATUS.md` | the `cargo test` rows and the line under them (32-42); `CI workflow` row (92); `CI` row of the parity table (141); `### CI` section (172-178) listing the gate's steps | must change: the `### CI` section and line 42 say what the two new steps do, the measured cost, and how review matches the tree line with a commit. The test counts do not move: this work adds no Rust test |
| `/root/.claude/plugins/cache/bloop/bloop/809798b4119e/bin/gate` (the bloop plugin) | runs `./checks.sh`, writes evidence/slice-N.md with the raw output, a `Commit` row of `HEAD` plus "with uncommitted changes", and the step list from the `==> ` lines | read only, out of reach. The new line reaches the evidence because it is printed output |
| `/root/.claude/plugins/cache/bloop/bloop/809798b4119e/skills/evidence-pack/SKILL.md` and `/root/.claude/plugins/cache/bloop/bloop/809798b4119e/skills/build/SKILL.md` | build gates, then commits the slice, then writes evidence; slice commits in this repo carry code and `docs/` only (for example `1606b1c`, `2cbfcbf`), the `.bloop/` records go in separate commits | why `.bloop/` is taken at `HEAD`: the code commit made right after the gate then has the printed tree |

Neither `nautilus-risex` nor `nautilus-lighter` is on this machine, and their adapter code has no gate of this kind to follow, so nothing is ported.

**This change has to reach** callers: the plugin's bin/gate and the `check` job of ci.yml run `./checks.sh` unchanged · tests: none in Rust; the checks are the two shell recipes of `## Slices` · fixtures: none · config: none · exports: none

## Prototype

- Question to answer: does `cargo clean -p nautilus-qfex` stop `./checks.sh` from running another tree's binaries out of a shared `CARGO_TARGET_DIR` while it keeps the compiled dependencies, what does it cost in gate time, and does a tree hash from a temporary index match the tree of the code commit made after the gate?
- What was tried: two `git clone --shared` copies A and B of `1c52db6` in the scratchpad, sharing one target dir copied from `target-control-shape`. B got `panic!("MUTANT OF COPY B")` in `debug_hides_the_secret` and ran `cargo test --lib debug_hides` with and without `--features examples`; then A ran `./checks.sh`, once as on `main` and three times with `cargo clean -p nautilus-qfex` added before `lint`. A `sh` function built the tree in `GIT_INDEX_FILE=<temp dir>/index` (`git read-tree HEAD`, `git add -A -- . ':(exclude).bloop'`, `git write-tree`), run on a clean copy, on A with an edited `checks.sh`, an untracked `src/` file and uncommitted `.bloop/` edits, then again after committing only the code; also in a `git archive` copy and in a directory nested inside A
- Result: as on `main`, A's gate failed in 4 s on B's mutant (the panic message and a backtrace into B's path), without compiling anything. With `cargo clean -p` A's gate passed (174 and 174, as written), compiled and checked only `nautilus-qfex` (no dependency rebuilt), removed 3330 files of 7.1 GiB each run, and took 57, 56 and 55 s, against 11 s for a warm run with no change and 54 s for a run after a source change without it. `cargo clean -p` on an empty target dir answers "Removed 0 files" and exit 0. The tree function took 25 ms, matched `git rev-parse HEAD^{tree}` on the clean copy, printed `3bcf06c…` on the dirty one, the code-only commit made afterwards had tree `3bcf06c…`, and the sha1 of the real index and the `git status --porcelain` output were the same before and after. The archive copy and the nested directory printed `none`. The plan holds as written
- Code thrown away, never merged

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `checks.sh` gets the step `own build` (`cargo clean -p nautilus-qfex`) between `format` and `lint`; the `gate` row of `ARCHITECTURE.md`, the `CI workflow` row of `README.md` and the `### CI` section of `docs/STATUS.md` say the gate rebuilds the crate each run and what that costs | Two `git clone --shared` copies A and B of the branch in the scratchpad with one shared `CARGO_TARGET_DIR`: in B add `panic!("MUTANT OF COPY B");` as the first line of `debug_hides_the_secret` in `src/config/exec.rs` and run `cargo test --lib debug_hides` and `cargo test --features examples --lib debug_hides`; then `./checks.sh` in A exits 0, prints `all checks passed`, and `grep -c 'MUTANT OF COPY B'` on its output is 0. The same recipe on `origin/main`'s `checks.sh` exits 1 on the mutant (the red run) | todo |
| 2 | `checks.sh` gets the first step `tree`, printing `tested tree: <40 hex> (...)` from a temporary index with `.bloop/` as at `HEAD`, or `tested tree: none, <reason>` outside the top of a git work tree; `docs/STATUS.md` (line 42 and `### CI`) and the `gate` row of `ARCHITECTURE.md` say how review matches the line with `git rev-parse <commit>^{tree}` | In a `git clone --shared` copy of the branch: with a clean tree the line's hash equals `git rev-parse HEAD^{tree}`; after editing a tracked file, adding an untracked file under `src/` and an uncommitted file under `.bloop/issues/`, the sha1 of `$(git rev-parse --git-path index)` and the `git status --porcelain` output are the same before and after `./checks.sh`, and committing only the code gives a commit whose `git rev-parse HEAD^{tree}` equals the printed hash; in a `git archive` copy `./checks.sh` prints `tested tree: none,` and still exits 0. The red run: `origin/main`'s `checks.sh` prints no `tested tree:` line | todo |

## Acceptance criteria

1. With a `CARGO_TARGET_DIR` that another tree has just filled with test binaries of a mutant that panics, `./checks.sh` in this tree passes and its output never shows the mutant's panic message or path, verified by the recipe of slice 1
2. `./checks.sh` recompiles only `nautilus-qfex`: in its output every `Compiling` and `Checking` line names `nautilus-qfex`, verified by `./checks.sh 2>&1 | grep -E '^ *(Compiling|Checking) ' | grep -vc 'nautilus-qfex'` printing 0 on a target dir whose dependencies are already built
3. `./checks.sh` prints exactly one line starting `tested tree: `, before the `format` step, verified by `./checks.sh 2>&1 | grep -c '^tested tree: '` printing 1
4. On a clean checkout the hash in that line equals `git rev-parse HEAD^{tree}`, verified by comparing the two
5. With tracked changes, untracked not ignored files and uncommitted files under `.bloop/`, the hash equals the tree of a commit made right after the gate that holds the code changes and no `.bloop/` change, verified by the recipe of slice 2
6. Running `./checks.sh` leaves the real index and the working tree as they were, verified by the sha1 of `$(git rev-parse --git-path index)` and the output of `git status --porcelain` before and after (recipe of slice 2)
7. In a `git archive` copy, and in a copy placed inside another git work tree, the line reads `tested tree: none,` followed by the reason, and the gate's exit code is that of the other steps, verified by the recipe of slice 2 and by `./checks.sh; echo $?` there
8. The `check` job of `ci.yml` passes on the pull request, and its log holds the `tested tree:` line and the `own build` step, verified by `gh run view --log` of that run
9. The `gate` row of `ARCHITECTURE.md`, the `CI workflow` row of `README.md` and the `### CI` section of `docs/STATUS.md` name both new steps, verified by reading them

## Risks

- Every gate run with no source change costs about 45 s more on this machine (11 s to 55 to 57 s). In box 4 Build the gate usually follows a source change, which already rebuilds the crate (54 s), so the cost there is 1 to 3 s. Paid on every pull request, which is why `RISK` is medium
- `cargo clean -p` deletes this crate's binaries in a shared target dir. Two gates running at the same moment on one target dir can now delete each other's binaries and fail with a missing file. They fail loudly, never pass on the wrong binary; sharing a target dir between two gates running at once was already unsafe
- Each run deletes and rewrites about 7 GiB of this crate's artifacts. Disk use does not grow, but a disk with less than that free fails the build step, as it would for any rebuild
- The tree line hashes every untracked, not ignored file. A big untracked directory that `.gitignore` misses (a target dir named other than `target` on a machine without the local `.git/info/exclude` lines) makes the step slow and its loose objects big. `git gc` prunes those loose objects later
- The hash matches a commit only when that commit's `.bloop/` is the gate's `HEAD`'s. If a records commit lands between the gate and the code commit, the hashes differ, and review has to fall back to timestamps as today
- In the main checkout the `shortcut marker` step's `grep -r` walks the extra `target-*` dirs: it took 52 s there on 28 Nov 2026. Out of scope here (restate: deleting those dirs is housekeeping), but it is gate time the measurements above, taken in a scratchpad copy, do not include

## Assumptions

- `cargo clean -p nautilus-qfex` rather than touching the source files or keeping a stamp of the last tree that built: a stamp misses the bare `cargo test` of another tree (the mutant of `2026-11-28-trade-socket-control-shape`), and touching changes the working tree, which restate rules out for the tree line and which a concurrent tree defeats in the same way
- The rebuild runs every time, even when this tree was the last to build, because no file in the target dir says reliably which tree built it
- `.bloop/` is taken as committed at `HEAD` in the hashed tree, because slice commits carry code and `docs/` only and the records are committed apart (restate left this to the plan)
- The line is computed once, as the first step, before anything is built; a file changed while the gate runs is not caught, because the gate takes about a minute and re-checking at the end would fail on every agent commit that moves `HEAD` meanwhile
- The line names the full 40-character hash, so review can compare it with `git rev-parse` output as text
- The tree step writes loose objects into `.git/objects` (blobs and trees of the temporary index). Restate forbids touching the index and the working tree, not the object store, and `git write-tree` cannot work without it
- The checks of both slices are shell recipes run by hand and recorded by `evidence-pack`, not a test kept in the repo: the repo has no harness for testing `checks.sh` itself, and adding one is more than restate asked
- On the `ubuntu-latest` runner `Swatinem/rust-cache@v2` is taken to leave this crate's own artifacts out of its cache, so `cargo clean -p` should remove little or nothing there and add no time. Not measured: no slice depends on it, and the log of criterion 8 shows the `Removed` count
