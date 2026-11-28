STATUS: GATED
MERGE: none
MERGED-AT: none
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

Dates are `YYYY-MM-DD`. They are headers rather than prose because `bin/metrics` reads them, and a date nobody can parse is a date nobody counts.

## The run

| step | what ran | result |
| --- | --- | --- |
| merge condition | `git grep -n send_text_on_connection origin/main -- docs/external/nautilustrader/README.md` after `git fetch`, `origin/main` at `4a32ba5` (PR #18 merged) | lines 16 and 28 printed, exit 0. The condition in `decision.md` holds |
| sync | `bin/ship 2026-11-27-trade-late-auth-answer` | rebased onto `origin/main` `4a32ba5`, 0 conflicts. Head `575c901` became `5721890`. 4 commits remain on the branch; the record commits already on `main` through #18 dropped out as already applied, and the 3 code commits are unchanged by `git range-diff` |
| re-gate | `bin/gate --e2e` inside the worktree, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target-late-auth-deliver`, a fresh dir | exit 0 |
| push | not yet run | the rebase rewrote the branch, so the push is `--force-with-lease`. It waits on a person's yes |
| merge | not yet run | waits on the push |

## Why a person merges this

Not merged without asking. Three of the five conditions fail:

- `RISK: high` from `intake.md`. That is a floor, refused whatever the config says, and `deliver.autoMerge.high` is false as well
- the gate passed after the sync (`gate: 0`) ✓
- `bin/trust --for autonomous` exits 1: the last 30 days have 19 landings, 0 reverts, rate 0.0000, but the 30 days before have 0 landings, and 5 are needed
- nothing in `spec.md` `## Context` is a protected path (`src/websocket/client.rs`, `src/execution/mod.rs`, `tests/exec_client.rs`, issue files) ✓
- `loop.trustLevel` is `autonomous` ✓

The person said "merge for me" about PR #18 and PR #16 in this session, after being told #18 goes first.

## Watch after deploy

Copied from `decision.md`, which wrote these before the merge. The trade client stays UNVERIFIED on the venue (`docs/STATUS.md`). Every check so far is against the mock, so the first live reconnects are the real test.

- The trade socket logs in after a reconnect but never subscribes, or never turns on cancel on disconnect. Signal: `QFEX WebSocket: session frame not sent on its connection` or `login answered on a closed connection, its session frames dropped`, with no later `Authenticated` on a newer connection. On the venue, fills and order updates stop after a reconnect while `SocketStateChanged` for endpoint `trade` reads `Connected`. Resting orders survive a disconnect they should not have survived
- The trade socket never logs in after a drop. Signal: `connection replaced before auth, the next one logs in` or `reconnect notice of a replaced connection, no auth sent`, with no `Authenticated` after the next `Reconnected`. The execution client's `logged_in` runs into `ANSWER_TIMEOUT`, or `consume` stays logged out after `Reconnected`
- `read_loop` stalls on a stale send. `send_text_on_connection` waits in `wait_for_active` for up to the connect timeout, 10 s by default (spec Risks, first bullet). Signal: market or trade frames arriving in a burst several seconds after a `Reconnected`
- The known gap stays open (`2026-11-28-trade-orders-before-live-login`). Orders sent with `send` during a reconnect can be replayed before `auth`, and `logged_in` can see a stale `Authenticated`. Signal: venue errors on order requests right after a `Reconnected`

## What a rollback would take

- command: `deliver.rollbackCommand` is null and nothing deploys on merge (`deliver.deployCommand` is null; this is a library crate). A rollback is `git revert -m 1 <merge sha>` on a branch cut from `origin/main`, through a pull request a person merges, then any consumer that pinned the merged rev moves its pin back
- what it cannot undo: nothing stored. The change has no migration, no persisted state and no venue contact. A strategy that already ran on the merged rev against the live venue keeps whatever orders it placed; reverting brings back the original race, where session frames can reach the next trade connection before its `auth`
