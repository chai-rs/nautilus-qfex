FR: none
KIND: request
GOAL: none

`./checks.sh` passes or fails on test binaries built from the tree it runs in, and says which tree that was, so a gate result and its evidence cannot belong to another checkout.

## Where it comes from

- Retro of `2026-11-28-trade-socket-control-shape`. A `CARGO_TARGET_DIR` shared between checkouts (the main checkout, `.worktrees/<id>`, a throwaway copy in the scratchpad) let `./checks.sh` run test binaries that another tree had built. Seen 3 times, each cleared by hand with `cargo clean -p nautilus-qfex`:
  - `2026-11-27-trade-late-auth-answer`, `debug.md`: the first `./checks.sh` in the main checkout after the worktree was removed failed on test binaries built in that worktree, with its deleted path compiled in
  - `2026-11-28-trade-socket-control-shape`, `spec.md` Risk 4: on 28 Nov 2026 `./checks.sh` in the main checkout counted 173 (binaries of PR #16's worktree) against count rows of 171, until a source file was touched
  - `2026-11-28-trade-socket-control-shape`, `decision.md`: the approver's first `./checks.sh` failed in `socket_state_follows_the_trade_socket` because it reused the test binary of a mutant copy (backtrace into `scratchpad/mutant/`)
- Same root, the evidence side: every `evidence/slice-*.md` since 27 Nov 2026 records `<parent sha> ... with uncommitted changes`, because the gate runs before the commit. Review cannot match the tree the gate tested with the commit it reviews (`2026-11-28-trade-socket-control-shape`, `review/round-1/review-code.md`, "What I did not cover", second bullet); it relied on timestamps

## What is asked

- `./checks.sh` makes sure the test binaries of this crate it runs were built from the tree it runs in, even when `CARGO_TARGET_DIR` points at a directory another checkout also uses. For example `cargo clean -p nautilus-qfex` before the build steps, or an equivalent that keeps the dependency cache. Decide in restate and plan, with the cost in gate time
- `./checks.sh` prints one line naming the git tree hash of what it tested, uncommitted changes included (for example through a temporary index and `git write-tree`), so `evidence-pack` captures it and review compares it with `git rev-parse <commit>^{tree}`
- `README.md`, `docs/STATUS.md` and `ARCHITECTURE.md` (the `gate` row) stay true

## Constraints

- No contact with the live QFEX venue
- The `check` job of `ci.yml` runs `./checks.sh` itself, so whatever is added must work on a fresh CI runner with no shared target dir
- `bin/gate` and `evidence-pack` belong to the bloop plugin and are out of reach here; the change lives in `checks.sh`
