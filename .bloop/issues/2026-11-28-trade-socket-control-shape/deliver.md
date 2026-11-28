STATUS: MERGED
MERGE: d779309
MERGED-AT: 2026-11-28
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

Dates are `YYYY-MM-DD`. They are headers rather than prose because `bin/metrics` reads them, and a date nobody can parse is a date nobody counts.

## The run

One row per step, in order. A step that was skipped says why, because a blank row reads like a step that passed.

| step | what ran | result |
| --- | --- | --- |
| sync | `bin/ship 2026-11-28-trade-socket-control-shape` at 2026-11-28T06:44Z, after `cargo clean -p nautilus-qfex` in `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target-control-shape` | `synced: true`, 0 conflicts. The branch already sat on `origin/main` at `92dca56`, so the rebase replayed nothing: `b9d820e` is 1 commit ahead, the same sha as `origin/refactor/2026-11-28-trade-socket-control-shape` |
| re-gate | `bin/gate --e2e` inside the worktree, through `bin/ship`, then once more by hand to keep the output | `gate: 0` both times. `all checks passed`: format, lint, unit + integration, test count (`cargo test` and `cargo test --features examples` each 171 passed, as written), shortcut marker, e2e smoke |
| push | none | not needed: the sync rewrote nothing, so the remote branch already holds `b9d820e` |
| ask | merge question to the user | asked because `deliver.autoMerge.low` is false and `bin/trust --for autonomous` exits 1 (previous window 0 landings, 5 needed). The user answered "merge ให้หน่อย" (please merge it), a yes |
| merge | `bin/ship 2026-11-28-trade-socket-control-shape --merge` | synced again (no-op, `origin/main` still `92dca56`) and `gate: 0` again, but `gh pr merge` got HTTP 403 because GraphQL is closed to this session, so `merged: false` |
| merge, REST | `gh api -X PUT repos/chai-rs/nautilus-qfex/pulls/17/merge -f merge_method=merge -f sha=b9d820e…` | merged as `d779309` on `main`, a `merge`-style merge commit, pinned to the gated head `b9d820e` |
| deploy | none | `deliver.deployCommand` is null and CI does not deploy: `ci.yml` runs `./checks.sh` and the fuzz build only. CI run on the merge commit: https://github.com/chai-rs/nautilus-qfex/actions/runs/46118159407 (in progress when recorded). Nothing ships from this repo, so `STATUS` stays `MERGED` |
| fuzz | not run | `/bloop:auto` drives this run and checks `bin/fuzz-due` itself |
| docs | `bin/docs` | exit 1, `docs/product` does not exist yet; nothing to refresh |
| cleanup | `git worktree remove .worktrees/2026-11-28-trade-socket-control-shape` | removed after the user said yes ("ลบได้"); it held no uncommitted files |

## Watch after deploy

Copied from `decision.md`, which wrote these before the merge. `watch` reads this list and nothing else, so a signal that is not here is a signal nobody looks at.

- A reconnect command for the QFEX execution client answers `EndpointNotFound` (or `Accepted`) instead of `Unsupported`. That would mean the control is now built inside the node's registry scope. Signal: the outcome in whatever tooling sends reconnect commands to the execution client
- `SocketStateChanged` for venue `QFEX`, endpoint `trade`, stops arriving or arrives without `Disconnected`/`Connected` around a drop. Signal: system events on the node's message bus during a venue flap
- Merge order with PR #16: whichever lands second must re-run `./checks.sh` after a rebase, with its own target dir or a cleaned crate. The count rows say 171 here and 173 on PR #16. Signal: the `test count` step of the gate on the second branch
- From `deliver.watchFor` in `.bloop/config.json`: mocha pins the merged rev and its socket state check passes

## What a rollback would take

Written **before** the deploy, while there is time to think. Empty here means nobody knows how to undo this, which is itself worth saying out loud.

- command: `deliver.rollbackCommand` is null. The undo is `git revert -m 1 <merge sha>` on a branch cut from `origin/main`, with `./checks.sh` green, through a pull request a person merges. Never a push to `main`
- what it cannot undo: nothing persistent. The change moves the trade socket control into a private `StateOnlyTradeSocketControl` in `src/execution/trade_socket.rs` and touches only `src/execution/mod.rs` and that new file: no migration, no config, no public API, no order or message sent to QFEX
- watch out: if PR #16 merged after this one and edited `src/execution/mod.rs` near the control, the revert may conflict there, and the gate must be re-run on the result
