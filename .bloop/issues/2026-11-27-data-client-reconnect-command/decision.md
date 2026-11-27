ESCALATE
APPROVED-BY: none

## Reviewers

- code: PASS (round 1, diff `362bcb0..691abc8`, no findings)
- security: not run, SURFACES none
- ux: not run, no ui surface
- ui: not run, no ui surface

## Spot check

The approver verifies at least one acceptance criterion itself, rather than trusting the report.

- criterion: 5 (after `disconnect()` and a second `connect()`, a fresh lookup's handle answers `Accepted` and `Disconnected`, `Connected` follow). Chosen because a missing second `register` in `Feed::close` fails silently in production; 1 to 4 run in the same test. Also 6 and 7
- command: in the worktree at `691abc8`, with `CARGO_TARGET_DIR=/home/user/nautilus-qfex/target`:
  - `cargo test --test data_client reconnect_command`
  - `cargo test --lib reconnect_request`
  - `grep -n "SocketControl\|\.register(" src/execution/mod.rs`
  - read `git diff 362bcb0..691abc8 -- src/data/mod.rs` and `tests/data_client.rs:560-605`
- result:
  - `reconnect_command_reconnects_the_market_data_socket ... ok` (1 passed, 0 failed, 0.90 s)
  - `websocket::client::tests::mds_reconnect_request_follows_the_connection ... ok` (1 passed, 0 failed)
  - the grep printed nothing (exit 1): the trade socket registers nothing
  - `market_data_socket` (`src/data/mod.rs:284-288`) takes `sink()` then `register` on the same `SocketControl`, and both `QfexDataClient::new` and `Feed::close` go through it. The test asserts `Accepted`, `Disconnected`, `Connected` after the second `connect()`, which is the socket that `Feed::close` built
  - Passed. The full suite (criterion 8, 170 passed) was not rerun here; it rests on `evidence/slice-2.md`

## Watch after deploy

What could still go wrong once this is live, and what would show it.

- A reconnect command for `QFEX-DATA` / `market_data` answers `UnknownEndpoint` again after the node stops and starts the data client. That would mean the registration was lost in `Feed::close`. Signal: the outcome in whatever tooling sends reconnect commands
- An `Accepted` command that never resubscribes. Signal: `SocketStateChanged` shows `Disconnected` then `Connected`, but no book or trade data arrives for the instruments that were subscribed
- A command that lands while the transport is already reconnecting by itself answers `AlreadyReconnecting` (handled by `nautilus-network`, not reviewed here). Signal: that outcome during venue flaps. It is expected, not a fault
- When the transport gives up its own reconnect attempts, what `request_reconnect` answers is up to `nautilus-network`. The lens did not review it (`review-code.md`, "What I did not cover"). Signal: a stale `Accepted` with no `Connected` after it

## Why escalated

The code is sound: the lens returned PASS, my spot check passed, and no starred assumption is open. I am escalating because the branch does not contain everything the spec asks for, and how it gets there is a merge-ordering decision for a person.

1. **Spec slice 2 is incomplete on this branch.** `spec.md` slice 2 also rewrites `ARCHITECTURE.md` dataflow step 7 and the "Reconnect registry side effect" bullet in `docs/external/nautilustrader/README.md`. `build.md` ("Notes for reviewers", third bullet) defers both until PR #4 (`claude/bloop-init`) merges, and says they land "before this one merges". As of now `origin/claude/bloop-init` (`770da23`) is not in `origin/main`. Both texts there still say the command answers `UnknownEndpoint`:
   - `origin/claude/bloop-init:ARCHITECTURE.md:52`
   - `origin/claude/bloop-init:docs/external/nautilustrader/README.md:26`

   Approving `691abc8` would approve a commit that is not the one meant to merge. The doc commits that complete slice 2 have not been written or reviewed yet.
2. **Whichever order is chosen, the docs end up wrong unless someone acts.** If PR #5 merges first, PR #4 then brings in text that says `UnknownEndpoint` after the behaviour has changed. AGENTS.md ("Every time") asks for behaviour changes to update the docs in the same branch. If PR #4 merges first, this branch needs a rebase and the two rewrites, and those new commits need review.

What the human has to decide:

- Pick the order. The recommended one is: merge PR #4, rebase this branch onto `main`, add the two rewrites, then send the branch back through review for a short docs-only round. Another option is to accept a follow-up issue for the rewrites and merge this branch as it is
- Optionally, confirm that a test asserting `!= Accepted` after `disconnect()` (`tests/data_client.rs:581-585`) is enough for criterion 4. The lens considered this and dropped it, because the unit test at `src/websocket/client.rs:1027-1031` pins `Closed`. I agree that is not a blocker
