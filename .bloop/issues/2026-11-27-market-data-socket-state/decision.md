ESCALATE
APPROVED-BY: none

## Reviewers

- code: PASS (one [should], severity 1: reconnect command outcome for `QFEX-DATA` changes from `Unsupported` to `UnknownEndpoint`)
- security: not run, SURFACES none
- ux: not run, no ui surface
- ui: not run, no ui surface

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 3 (after `disconnect()` and a second `connect()`, `Connected` is sent again: the sink survives `close()`), with 1, 2 and 4 covered by the same test; also 6 (no regressions)
- command: in the worktree at `1c17205`, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target`: `cargo test --offline --test data_client socket_state` three times, then `cargo test --offline`; read `src/data/mod.rs` lines 272-275 and 309-316 to confirm both `new_mds` calls take `self.socket.sink()` / `socket.sink()` from one shared `Arc<SocketControl>`
- result: `socket_state_follows_the_market_data_socket ... ok` on 3 of 3 runs (about 1 s each). Full suite: 142 + 8 + 2 + 4 + 12 = 168 passed, 0 failed (166 baseline + 2 new tests, as `build.md` says). Criterion 5 by reading: `new_trade` sets `state_sink: None`, no `src/execution` change. Criterion 7: the diff adds no host; the new test uses `start_venue` on `127.0.0.1`

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- No `SocketStateChanged` for `QFEX-DATA` / `market_data` after node start: the data client was built on a thread without the system event sender. Signal: an actor on `subscribe_socket_state` sees nothing after the first market data connect
- Events stop after the data client is disconnected and reconnected by the node: the sink taken in `close()` did not take over. Signal: one `Connected` at start, then silence after a node-driven reconnect of the data client
- Duplicate `Connected` / `Disconnected` for the same `(client_id, endpoint)`: an old sink was not retired. Signal: two events per transport change in the actor log
- A controller reconnect command for `QFEX-DATA` now returns `UnknownEndpoint` instead of `Unsupported` (see below). Signal: that outcome in whatever tooling sends reconnect commands

## Why escalated

Every lens passed and the spot check passed. The one thing I could not decide is a behaviour change the spec never asked for and does not mention.

- What: `SocketControlFactory::new` in `QfexDataClient::new` (`src/data/mod.rs:309-310`) runs inside the `socket_registry.scope(..)` that `LiveNodeBuilder` wraps around data client factories (`nautilus-live-0.67.0/src/node/builder.rs:646`). That marks `QFEX-DATA` as supported in the reconnect registry, but the data client never calls `SocketControl::register`. A controller reconnect command for `QFEX-DATA` / `market_data` therefore changes from `Unsupported` to `UnknownEndpoint`. Reported by `review/round-1/review-code.md` finding 1, with a probe showing `ClientNotFound` before and `EndpointNotFound` at `1c17205`.
- Why it matters: no reconnect is lost (none was possible before), but the outcome reported to the command sender is now wrong, and the endpoint it calls unknown is the one this change starts publishing state for. Neither `spec.md` nor `build.md` nor `docs/STATUS.md` records it.
- What the human has to decide: accept it and have `build` record the `UnknownEndpoint` outcome in `docs/STATUS.md` (or `docs/PLAN.md`); or open a follow-up issue to register the market data socket's reconnect; or ask for the factory to be built outside the registry scope.
- Also note before merging: per `build.md` notes, the branch is cut from `claude/bloop-init`, so merging it into `main` also lands the bloop setup commits.
