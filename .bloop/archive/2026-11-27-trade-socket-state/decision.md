APPROVE
APPROVED-BY: none

## Reviewers

- code: PASS (round 1, one [should] finding, see Watch after deploy)
- security: not run, SURFACES none (the diff does not touch login or signing)
- ux: not run, no ui surface
- ui: not run, no ui surface

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 1 to 5 (the trade socket's state on connect, loss, reconnect with re-login, deliberate disconnect, second connect, and `ClientNotFound` in the registry). These are the criteria that would hurt most if wrong, because they cover the connect and reconnect path that also drives login. Also 6 (test count) and 9 (no host outside 127.0.0.1)
- command: in `/home/user/nautilus-qfex/.worktrees/2026-11-27-trade-socket-state` at `21c6b9e`, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target`: `cargo test --test exec_client socket_state` run 11 times; `cargo test` (all targets, summed); `git grep -n "qfex.com" -- tests/exec_client.rs`
- result: `socket_state_follows_the_trade_socket ... ok` in 11 of 11 runs (1.0 to 1.2 s each). I read the test: it asserts `Connected`, `Disconnected` then `Connected`, a second `auth` frame, no event within 500 ms after `disconnect()`, `Connected` after the second connect, and `ClientNotFound` for `trade` and `market_data`. Full `cargo test`: 171 passed, 0 failed, matching both count rows of `docs/STATUS.md` (lines 31-32). `git grep` printed nothing (exit 1)

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- A hang in CI instead of a red test: the drop retry loop in `tests/exec_client.rs:1010-1015` has no upper bound (reviewer-code finding 1, [should]). If a later change stops the trade socket from publishing `Disconnected`, `checks.sh` hangs. The signal is a CI job that runs until its timeout on `exec_client`. A follow-up should bound the loop with an outer `tokio::time::timeout`
- No socket state from the trade socket under a host that builds the `LiveNode` on one thread and connects it on another. The control reads the thread-local system event sender on the first `connect()`. The signal is an actor subscribed to `SocketStateChanged` that sees `market_data` events and no `trade` events. Reviewer-code argued same-thread from the code and did not run a real node
- After a refused login, the last state an actor sees is `Connected` while the client has stopped (accepted in spec Risks and written in `docs/STATUS.md`). The signal is the error log from `consume` or the error that `connect()` returns, not a `Disconnected`
- A controller reconnect command for the execution client should still answer `Unsupported`. If it answers `UnknownEndpoint`, the control has moved inside the registry scope

## Why escalated

Not escalated.
