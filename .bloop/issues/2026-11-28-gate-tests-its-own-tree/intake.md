DECISION: ACCEPT
TYPE: ci
FR: none
RISK: medium
SURFACES: none
RESCOPED: no

## Summary

- Accepted: `./checks.sh` makes sure the test binaries of this crate it runs were built from the tree it runs in, and prints the git tree hash of what it tested
- Cause is known and seen four times: a target dir shared by two trees let `./checks.sh` run a binary another tree built (two checkouts, a worktree, a mutant copy in the scratchpad)
- The fix stays inside `checks.sh`; `bin/gate` and `evidence-pack` belong to the plugin
- Bare `cargo test` loops run by hand outside `./checks.sh` are out of scope, see `restate.md`
- `RISK` medium: every pull request and the `check` job of `ci.yml` go through this one script

## Why

`checks.sh` today runs `cargo clippy` and `cargo test` against whatever `CARGO_TARGET_DIR` holds and never names the tree it tested, so a stale binary from another tree can pass or fail the gate, and `evidence/slice-*.md` can only record `<parent sha> ... with uncommitted changes`. Three target dirs (`target`, `target-late-auth`, `target-control-shape`) still sit under the main checkout, which shows the "a target dir per tree" workaround is in use and costs disk (about 55 GB at the retro of `2026-11-27-trade-late-auth-answer`). The `check` job of `.github/workflows/ci.yml` runs `./checks.sh` after `actions/checkout@v4` and `Swatinem/rust-cache@v2`, so a change has to hold on a fresh runner with a restored dependency cache. The work is one script plus three docs rows (`ARCHITECTURE.md` `gate` row, `README.md`, `docs/STATUS.md`), so it lands as one pull request.
