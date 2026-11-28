VERDICT: PASS

Lens: code. Diff `92dca56..9cea260` on `fix/2026-11-27-trade-late-auth-answer`. Load assumed: one node per process (`ARCHITECTURE.md:99`), so one trade socket and one market data socket per process; nothing in this change runs per user or per item.

## Findings

None. What was checked, and why it holds:

- **Epoch of `RECONNECTED` against `connection_epoch()`.** A bound `auth` could be skipped forever if the notice's epoch lagged the counter. It does not: nautilus-network 0.67.0 increments the counter in the writer on `WriterCommand::Update` (`websocket/client.rs:2107`), the controller waits for that confirmation (`:1557`) before it spawns the new read task (`:1605`), and it reads the same counter when it hands `RECONNECTED` over (`:3874-3878`). Only the controller task reconnects, so the notice for epoch N is handled with `connection_epoch() == N` unless a later reconnect has finished. In that case the skip in `start_session` (`src/websocket/client.rs:599`) is right, and the later notice sends its own `auth`.
- **Epoch 0 on the first `auth`.** The counter starts at `AtomicU64::new(0)` (`:250`, `:495`), and `connect()` builds a new `WebSocketClient` each time, after `self.disconnect()`. So `start_session(&client, &self.session, 0)` (`src/websocket/client.rs:483`) names the first connection.
- **The hand-off on `ConnectionChanged` in `connect()` cannot hang the login.** `connect()` returns `Ok` with the read task started. The `RECONNECTED` that follows sits in the unbounded `raw_rx` until `read_loop` runs. `logged_in` (`src/execution/mod.rs:405`) is bounded by `ANSWER_TIMEOUT`, so a reconnect that never succeeds still ends in an error, as the old `wait_for_active` timeout did.
- **Order within the channel.** The old read task is fenced and aborted (`:1578-1586`) before `RECONNECTED` of the replacement is emitted, so an `auth` answer of epoch N can never be read after the notice of N+1. `send_session_frames` therefore never races a newer `auth`.
- **Session frames written before the execution client sees `Authenticated`.** `send_session_frames` awaits each `send_text_on_connection` response before `out_tx.send(message)` (`src/websocket/client.rs:690-697`), the same order as before the change.
- **Blocking in `read_loop`.** A frame bound to epoch N while the transport is reconnecting waits in `wait_for_active` for up to the connect timeout and is then refused. The old `send_text` waited the same way, so this is not a regression. The spec lists it under Risks.
- **The market data socket.** `start_session`'s market data branch and `subscribe_pump` still use `send_text`. Only the frame channel type changes.
- **Tests.** Both new checks hold `read_loop` through the gate lock and assert `rx.try_recv().is_err()` before letting go, so they fail rather than pass if the seam stops holding. The evidence shows each one red with its own binding put back to `send_text` (`evidence/slice-1.md`, `evidence/slice-2.md`).
- **Dependency direction.** The change stays inside `websocket`. It adds `nautilus_network::error::SendError` and touches no `data` or `execution` code.

## What I did not cover

- The `Err(SendError::ConnectionChanged)` hand-off branch of `start_session` (`src/websocket/client.rs:611`) is reached by no test (`grep -n ConnectionChanged src/websocket/client.rs tests/*.rs` prints only that line). Both unit checks stop at the pre-check at `:599`, because they hold the loop until the replacement is fully up. Reaching the branch needs a drop between the epoch check and the writer, which the `MockVenue` cannot force. I read the branch and judged it correct, but did not prove it with a mutation, so this is not written up as a finding.
- I did not rerun the suite or the 90-run loop-A replay. I took the counts from `build.md` and `evidence/`.
- Orders sent with `QfexWebSocketClient::send` during a reconnect still go through the replay buffer. The restate puts them out of scope, and `docs/STATUS.md` records it.
- Security aspects of `auth` delivery (the nonce and the key) are left to `reviewer-security`.
- `.bloop/config.json` has no `companions.review` rows, so no companion skill applied.
- The behaviour against the real QFEX venue was not checked. Nothing here contacts it.
