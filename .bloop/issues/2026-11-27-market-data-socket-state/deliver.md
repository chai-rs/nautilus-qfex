STATUS: MERGED
MERGE: 362bcb0
MERGED-AT: 2026-11-27
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

Dates are `YYYY-MM-DD`. They are headers rather than prose because `bin/metrics` reads them, and a date nobody can parse is a date nobody counts.

## The run

| step | what ran | result |
| --- | --- | --- |
| sync | `bin/ship 2026-11-27-market-data-socket-state` | already up to date with `origin/main` (`4f4ed16`), nothing replayed |
| re-gate | `bin/gate --e2e` inside `bin/ship` | exit 0 |
| merge | merged on GitHub by the repository owner (PR #3, merge commit) | `362bcb0`, tree identical to the gated `733509e`. A person merged because `deliver.autoMerge.medium` is false and `bin/trust --for autonomous` is not met (4 landings, 5 needed) |
| deploy | not applicable | a library; mocha pins the merge commit |

## Watch after deploy

- No `SocketStateChanged` for `QFEX-DATA` / `market_data` after node start: the data client was built on a thread without the system event sender. Signal: an actor on `subscribe_socket_state` sees nothing after the first market data connect
- Events stop after the data client is disconnected and reconnected by the node: the sink taken in `close()` did not take over. Signal: one `Connected` at start, then silence after a node-driven reconnect of the data client
- Duplicate `Connected` / `Disconnected` for the same `(client_id, endpoint)`: an old sink was not retired. Signal: two events per transport change in the actor log
- Known and accepted: a controller reconnect command for `QFEX-DATA` answers `UnknownEndpoint` instead of `Unsupported` until `2026-11-27-data-client-reconnect-command` lands. Signal: that outcome in whatever tooling sends reconnect commands; it should change when the follow-up merges
- Merging this branch into `main` also lands the bloop setup and issue-record commits from `claude/bloop-init` (everything up to `41f3fc9`)

## What a rollback would take

- command: revert the merge commit on `main` (`git revert -m 1 <merge sha>`) and have mocha pin the previous rev
- what it cannot undo: nothing persistent; the adapter keeps no state
