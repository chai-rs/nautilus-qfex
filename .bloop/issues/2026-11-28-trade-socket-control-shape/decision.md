APPROVE
APPROVED-BY: none

## Reviewers

- code: PASS (round 1, `review/round-1/review-code.md`, diff `origin/main...b9d820e`, no findings)
- security: not run, SURFACES none
- ux: not run, no ui surface
- ui: not run, no ui surface

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 5. Under a registry scope the client still claims no reconnect support, and the trade socket still publishes its states. I chose it because a control built inside the scope turns a node's answer from `Unsupported` into `EndpointNotFound` without any error. I also checked that the test really catches that regression, and I re-ran the gate for 1 to 4, 6 and 7, because the lens re-ran nothing ("What I did not cover", first two bullets)
- command: in worktree `.worktrees/2026-11-28-trade-socket-control-shape` at `b9d820e` (clean), `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target-control-shape`:
  - `cargo test --test exec_client socket_state_follows_the_trade_socket`
  - the greps of criteria 2, 3 and 4, `grep -n trade_socket_control src/execution/mod.rs`, and `git diff origin/main --stat -- tests docs README.md ARCHITECTURE.md src/websocket/client.rs`
  - mutation: a throwaway `git archive HEAD` copy in the scratchpad, with `QfexExecutionClient::new` changed to call `trade_socket_control.sink(client_id)` once before it returns, so the control is built inside the scope. Then the same test. The branch was not edited, and the copy is deleted
  - `cargo clean -p nautilus-qfex`, then `./checks.sh`
- result:
  - branch: `socket_state_follows_the_trade_socket ... ok` (1 passed, 0 failed)
  - all greps printed nothing (exit 1), and the stat was empty. `trade_socket_control` appears at `mod.rs:498` (field), `:522` (`built_on_first_connect()` in `new`) and `:755` (`sink(client_id)` in `connect()`)
  - mutation: FAILED at `tests/exec_client.rs:1031`, `matches!(registry.get(..), SocketReconnectLookup::ClientNotFound)`. The test pins the timing that the new names describe
  - gate: `all checks passed`. `cargo test` and `cargo test --features examples` each gave 171 passed, matching both count rows
  - note: my first `./checks.sh` run failed in this same test, because it reused the mutant's test binary from the shared target dir (the backtrace pointed into `scratchpad/mutant/`). After `cargo clean -p nautilus-qfex`, the rebuild from the worktree passed. This repeats spec Risk 4: a shared target dir can run another tree's binaries
  - spot check PASS

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- A reconnect command for the QFEX execution client answers `EndpointNotFound` (or `Accepted`) instead of `Unsupported`. That would mean the control is now built inside the node's registry scope. Signal: the outcome in whatever tooling sends reconnect commands to the execution client
- `SocketStateChanged` for venue `QFEX`, endpoint `trade`, stops arriving or arrives without `Disconnected`/`Connected` around a drop. Signal: system events on the node's message bus during a venue flap
- Merge order with PR #16: whichever lands second must re-run `./checks.sh` after a rebase, with its own target dir or a cleaned crate. The count rows say 171 here and 173 on PR #16. Signal: the `test count` step of the gate on the second branch

## Why escalated

Not escalated.
