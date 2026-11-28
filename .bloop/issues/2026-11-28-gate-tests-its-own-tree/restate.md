CONFIRMED: no

# The gate tests its own tree and says which tree that was

## What we will do

- Each run of `./checks.sh` rebuilds this crate's own lib, test and example binaries from the files of the tree it runs in before clippy and the tests use them, even when `CARGO_TARGET_DIR` points at a target dir another checkout, worktree or throwaway copy also builds into. A binary built by another tree is never what the gate runs
- The compiled dependencies already in the target dir are kept, so the extra gate time is only the rebuild of this crate. The plan measures that time on this machine and records it
- `./checks.sh` prints one line that names the git tree hash of what it tested, uncommitted and untracked (not ignored) files included, without touching the real index or the working tree
- From that line and a commit, review can tell whether the gate tested exactly that commit. The plan settles how the issue records under `.bloop/` that are still uncommitted at gate time are handled, since slice commits carry code and records separately and those records would otherwise make every hash differ from the code commit
- Outside a git work tree (a `git archive` copy) the gate still runs and its line says no tree hash could be named, rather than failing the gate
- The same script passes in the `check` job of `ci.yml` on a fresh runner
- The `gate` row of `ARCHITECTURE.md`, `README.md` and `docs/STATUS.md` say what the gate now does

## What we will not do

- Bare `cargo test` or `cargo clippy` loops run by hand outside `./checks.sh` (the approver's spot check of `2026-11-27-trade-late-auth-answer`): nothing in this repo can guard them; they stay under the rule "a target dir per tree" and the retro of this issue decides whether that rule needs more
- A target dir per tree made automatic by `checks.sh`: it is the 55 GB disk cost the issue reports
- Changes to `bin/gate` or `evidence-pack`: they belong to the bloop plugin. They capture the new line as printed output
- Deleting the extra target dirs under the main checkout: housekeeping, not the gate
- Any contact with the live QFEX venue

## Contracts affected

none
