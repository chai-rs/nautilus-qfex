APPROVE
APPROVED-BY: none

## Reviewers

- code: PASS (round 2; one [should], severity 1: `docs/STATUS.md:77` pointed at `.bloop/issues/2026-11-27-data-client-reconnect-command`, not on the branch at `46c594b`. Resolved by the rebase onto `claude/bloop-init`, verified below)
- security: not run, SURFACES none
- ux: not run, no ui surface
- ui: not run, no ui surface

Round-1 escalation: the `UnknownEndpoint` side effect is recorded in `docs/STATUS.md:77` and the reconnect registration is tracked in `.bloop/issues/2026-11-27-data-client-reconnect-command/issue.md`, as the human chose. Documenting it, not fixing it, is the agreed scope.

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 3 (after `disconnect()` and a second `connect()`, `Connected` is sent again: the sink survives `close()`), with 1, 2 and 4 covered by the same test; 6 (no regressions); plus the round-2 lens finding
- command: in the worktree at `733509e`, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target`:
  - `git merge-base --is-ancestor b0dcd23 HEAD` and `git merge-base --is-ancestor claude/bloop-init HEAD`; `git ls-tree -r --name-only HEAD .bloop/issues | grep reconnect-command`
  - `git diff --stat 46c594b 733509e -- src tests docs Cargo.toml Cargo.lock` (rebased code against the commit the lens and gate saw)
  - `cargo test --offline --test data_client socket_state` three times, then `cargo test --offline`
- result:
  - `b0dcd23` and `claude/bloop-init` are both ancestors of `733509e`; `.bloop/issues/2026-11-27-data-client-reconnect-command/issue.md` is in the tree, so the `docs/STATUS.md:77` pointer resolves. Lens finding resolved
  - The diff from `46c594b` to `733509e` over `src`, `tests`, `docs` and the manifests is empty: the rebase changed no code, so the round-2 lens and `evidence/round-2.md` (exit 0) apply to `733509e`
  - `socket_state_follows_the_market_data_socket ... ok` on 3 of 3 runs (about 1 s each). Full suite: 142 + 8 + 2 + 4 + 12 = 168 passed, 0 failed (166 baseline + 2 new tests; spec criterion 6 says 167, `build.md` and the round-1 decision account for the second new unit test)
  - Criterion 7: `grep -n "qfex.com" tests/data_client.rs` prints nothing; the new test runs against `127.0.0.1`

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- No `SocketStateChanged` for `QFEX-DATA` / `market_data` after node start: the data client was built on a thread without the system event sender. Signal: an actor on `subscribe_socket_state` sees nothing after the first market data connect
- Events stop after the data client is disconnected and reconnected by the node: the sink taken in `close()` did not take over. Signal: one `Connected` at start, then silence after a node-driven reconnect of the data client
- Duplicate `Connected` / `Disconnected` for the same `(client_id, endpoint)`: an old sink was not retired. Signal: two events per transport change in the actor log
- Known and accepted: a controller reconnect command for `QFEX-DATA` answers `UnknownEndpoint` instead of `Unsupported` until `2026-11-27-data-client-reconnect-command` lands. Signal: that outcome in whatever tooling sends reconnect commands; it should change when the follow-up merges
- Merging this branch into `main` also lands the bloop setup and issue-record commits from `claude/bloop-init` (everything up to `41f3fc9`)

## Why escalated

Not escalated.
