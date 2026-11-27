STATUS: MERGED
MERGE: ce9d457
MERGED-AT: 2026-11-27
DEPLOYED-AT: none
DEPLOYED-BY: none
ROLLED-BACK-AT: none

## The run

| step | what ran | result |
| --- | --- | --- |
| sync | `bin/ship 2026-11-27-trade-socket-state` | up to date with `origin/main` (`4b46b86`) |
| re-gate | `bin/gate --e2e` inside `bin/ship` | exit 0 |
| merge | merged by the agent on the person's request in session ("merge PR 8 กับ 9 ให้หน่อย"), PR #8, merge commit; CI `check` and `fuzz` green on the PR | `ce9d457` |
| deploy | not applicable | a library; mocha pins the merge commit |

## Watch after deploy

- A hang in CI instead of a red test: the drop retry loop in `tests/exec_client.rs:1010-1015` has no upper bound (reviewer-code finding 1, [should]). If a later change stops the trade socket from publishing `Disconnected`, `checks.sh` hangs. The signal is a CI job that runs until its timeout on `exec_client`. A follow-up should bound the loop with an outer `tokio::time::timeout`
- No socket state from the trade socket under a host that builds the `LiveNode` on one thread and connects it on another. The control reads the thread-local system event sender on the first `connect()`. The signal is an actor subscribed to `SocketStateChanged` that sees `market_data` events and no `trade` events. Reviewer-code argued same-thread from the code and did not run a real node
- After a refused login, the last state an actor sees is `Connected` while the client has stopped (accepted in spec Risks and written in `docs/STATUS.md`). The signal is the error log from `consume` or the error that `connect()` returns, not a `Disconnected`
- A controller reconnect command for the execution client should still answer `Unsupported`. If it answers `UnknownEndpoint`, the control has moved inside the registry scope

## What a rollback would take

- command: revert the merge commit on `main`; mocha pins the previous rev
- what it cannot undo: nothing persistent
