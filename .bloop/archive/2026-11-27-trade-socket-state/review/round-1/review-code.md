VERDICT: PASS

Reviewed: `git diff 4b46b86..21c6b9e` on `feat/2026-11-27-trade-socket-state`.
Load assumed: one `LiveNode` process holding one QFEX execution client with one trade socket (README and ARCHITECTURE describe a single-node adapter, and no deploy config says otherwise). At that load nothing in this change scales with the number of users or orders. The change adds one control per client, and each `connect()` makes one sink.
Companions: `.bloop/config.json` has no `companions.review` key, so no extra skill applied.

## Findings

1. [should] (2) tests/exec_client.rs:1010 - the retry loop around `drop_connection.notify_waiters()` has no upper bound. If a regression stops the trade socket from publishing `Disconnected` (for example, a later change drops the sink on reconnect or swallows the loss), the test does not fail. It spins every 200 ms forever, and `checks.sh` hangs instead of going red, because neither `checks.sh` nor a nextest config sets a per-test timeout (`grep -n "timeout\|nextest" checks.sh .config/nextest.toml` prints nothing).
   Proof: the loop's only exit is `break` on a received event (lines 1010-1015). `tokio::time::timeout(200 ms, ...)` returning `Err` goes straight back to the next `notify_waiters()`. The fix is to bound it, either with a fixed number of tries or with an outer `tokio::time::timeout(Duration::from_secs(10), ...)` like `next_state` uses, so a missing `Disconnected` fails with a message.

Checked and found sound (no finding):
- Sender scope: `SocketControlFactory::new` reads `try_get_system_event_sender()`, which is thread-local (`nautilus-common-0.67.0/src/live/runner.rs:231`), when the factory is made, which here means the first `connect()`. In a `LiveNode`, `bind_senders_for_node` (`nautilus-live-0.67.0/src/runner.rs:342`) sets the sender on the build thread, and `connect_exec_clients` is awaited inside the node's own `!Send` future (`node/mod.rs:2290`, clients behind `RefCell`), so both run on the same thread. In the test, `replace_system_event_sender` and `connect()` both run in the `block_on` future on the test thread, so other tests cannot leak events into this channel.
- Registry: `SocketControlFactory::new` takes the registrar from `SOCKET_REGISTRARS`, which is set only inside `scope(..)`. Building the control in `connect()` therefore leaves `owner` as `None`, and the test's `ClientNotFound` assertion shows this.
- Successive sockets: each `connect()` calls `socket.sink()`, and `sink_with` advances the shared generation (`socket.rs:547`), so a socket that has been replaced cannot publish over the current one.
- Failure paths in `connect()`: an error before `ws.connect()` publishes nothing. A refused login publishes `Connected` with no `Disconnected` after it. The spec Risks accepted this and `docs/STATUS.md` records it.
- `Debug` derive on `QfexExecutionClient` covers `SocketControl`, which holds ids, an endpoint and a sender, and no secret.
- Docs: the counts in `docs/STATUS.md` (171, 13 execution client) agree with `evidence/slice-1.md`, and invariant 9 and the relationship line match the new endpoint.

## What I did not cover

- I did not rerun the suite or check the flakiness numbers in build.md (5 of 30 red before the retry, 30 of 30 after). I took them from `evidence/`.
- I did not run a real `LiveNode` end to end. The same-thread argument above comes from reading `nautilus-live` 0.67.0, not from a run. A host that builds the node on one thread and runs it on another would leave the trade control without a sender, and it would publish nothing. The data client has the same exposure at `new`.
- The existing `reconnect_logs_in_again_and_catches_up` (tests/exec_client.rs:958) may have the same lost-`notify_waiters` race that build.md describes. It is outside this diff, so I did not judge it.
- Security of the login and signing paths: the diff does not touch them, and they belong to `reviewer-security`.
