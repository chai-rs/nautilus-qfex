STATUS: ROOT-CAUSE
REPRO: in a throwaway worktree at main, cp .bloop/issues/2026-11-27-trade-late-auth-answer/repro/src/websocket/client.rs.txt src/websocket/client.rs && cargo test --lib trade_late_auth_answer
RATE: 20/20
CHECK: src/websocket/client.rs
ROUNDS: 1
ROOT-CAUSE: the trade socket's read loop sends the session frames for an `auth` answer with an ordinary `send_text`, which is not bound to the connection that answered, so nautilus-network 0.67.0 can deliver them on the next connection before that connection's `auth`

## Report

From the retro of `2026-11-27-trade-socket-state` (`.bloop/archive/2026-11-27-trade-socket-state/retro.md`, row 2), written up in `issue.md` and in the `websocket` trade client row of `docs/STATUS.md`.

`socket_state_follows_the_trade_socket` in `tests/exec_client.rs`, as merged in `ce9d457`, failed 1 run in 100 on 27 Nov 2026. The stand-in recorded `venue refused: ["subscribe before auth"]`: a `subscribe` frame reached a trade connection that had not yet received `auth`. With the drop made reliable and fired right after the first `Connected`, it failed 2 runs in 100, always with that message. The retro then made the test wait for the first `subscribe` before the drop, so the test no longer shows the race.

The issue's hypothesis: the venue answers `auth` on socket 1, socket 1 drops before `read_loop` in `src/websocket/client.rs` handles that answer, and the late answer then sends `subscribe` on socket 2 before socket 2's `auth`.

## Feedback loop

Two loops, both run in a throwaway worktree at `5c63bd5` and deleted afterwards.

**Loop A, the reported shape.** `tests/exec_client.rs` with one added test: `socket_state_follows_the_trade_socket` as it is on `main`, without its wait for the first `subscribe` before `drop_socket()`. That is the shape the retro measured. The test is kept in `repro/loop-a.txt`.

- Run alone 100 times: 0/100 red. Alone under CPU load: 1/40
- The whole `exec_client` binary, all 14 tests at once: **12/90 red** with no probe (3/30, then 9/60). The race needs the other tests of the binary competing for the 4 cores, which is how `cargo test` runs it
- Same failure as reported: yes. Each red run printed `venue refused: ["subscribe before auth"]` from `Harness::drop`, from the added test and from no other

**Loop B, the tightened loop, and the check.** One unit test in the `tests` module of `src/websocket/client.rs`, against that module's `MockVenue`: `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth`. The venue holds its answer to the first `auth`, then sends a `subscribed` frame with the `auth` success behind it, and drops the socket. The test holds the subscribe gate's lock, so `read_loop` stops at the `subscribed` frame with the `auth` answer unread behind it. It lets go only once socket 2 is connected. No timing is left to chance.

- Command: the `REPRO` line above
- **20/20 red**, every run with the same output: socket 2 received `["subscribe", "auth", "subscribe"]`, where `["auth", "subscribe"]` is right
- Same failure as reported: yes. A `subscribe` reaches the next trade socket before its `auth`, which is what the stand-in of `tests/exec_client.rs` refuses as `subscribe before auth`

Red output of both: `repro/red.txt`.

## Hypotheses

Written before any probe ran. One row per hypothesis, in the order they were tested. Every probe was a change in the throwaway worktree, and every log line carried the tag `bloop-debug-<id>`.

| # | round | hypothesis | if true, then | probe | result |
| --- | --- | --- | --- | --- | --- |
| H1 | 1 | The session frames that `read_loop` sends on an `auth` answer are not bound to the connection that answered. `client.send_text` (`src/websocket/client.rs:659`) hands them to whatever connection nautilus-network 0.67.0 holds or reaches next, and the replacement is up before `RECONNECTED` reaches `read_loop` | sending them only on the connection the answer came from turns loop A and loop B green, and taking that out turns both red again | `channel_epoch_message_handler` and `WebSocketClient::epoch_builder` in `connect`, so `read_loop` knows each answer's connection epoch, and `send_text_on_connection(frame, None, epoch)` in the `Authenticated` arm | **confirmed**. With the probe: loop B 0/20, loop A 0/60, the whole unit suite 144 of 144. Probe taken out: loop B 20/20 red, loop A 9/60 red |
| H2 | 1 | The issue's narrower story explains loop A: `read_loop` handles socket 1's answer only after the transport has moved to socket 2 | a log line at the `Authenticated` arm shows the connection epoch already moved when the first answer is handled, in the red runs | a log line with the client's address, `connection_epoch()` and `connection_mode()`, at the `Authenticated` arm and at `RECONNECTED`; 90 runs of the whole binary, 14 red | falsified. In 1620 handled `auth` answers, none had an epoch ahead of the last `RECONNECTED` its read loop had handled. In every red run the first answer was handled at epoch 0, mode `Active`, and the frames were queued at epoch 0, mode `Active` |
| H3 | 1 | In loop A, `read_loop` handles socket 1's answer while the transport is reconnecting, and `send_text` waits in `wait_for_active` until socket 2 is up | the same log line shows mode `Reconnect` at the first answer, in the red runs | the H2 log line, same runs | falsified. No answer was handled in mode `Reconnect`. The one answer not in `Active` was a deliberate `disconnect`, mode `Disconnect`, in a green run |
| H4 | 1 | The stand-in is wrong: the `subscribe` it counts as before `auth` came after connection 2's `auth`, or was read on connection 1 | a log of each frame the stand-in reads, with its venue and connection, shows the `subscribe` somewhere other than first on connection 2 | a log line in `Venue::serve` of `tests/exec_client.rs` for each frame read and for the drop; 90 runs, 11 red | falsified. In all 11 red runs connection 1 read `auth`, then dropped, and never read a `subscribe`. Connection 2 read `subscribe` (not authenticated), then `auth`, then `subscribe` again |

One more observation, not a hypothesis of its own, from a logger the loop A test installed for nautilus-network's log lines (90 runs, 10 red): every red run logged `Buffering message for replay` and `Sending 1 buffered messages after reconnection` (or `2`), one of them after `Failed to send message`. None of the 80 green runs logged either.

## Root cause

`read_loop` (`src/websocket/client.rs:625`) reads the frames of every connection through one channel, and `connect` builds that channel with `channel_message_handler()` (`:472`), which drops the connection epoch nautilus-network attaches to each frame. On an `auth` answer the loop sends the session frames with `client.send_text` (`:659`). In nautilus-network 0.67.0 that is an ordinary send: it waits through a reconnect (`websocket/client.rs:3515`), and a frame the writer takes during a reconnect, or fails to write, goes into a replay buffer (`:2116-2120`, `:2233-2237`). The buffer is written to the replacement as soon as it is active (`:2029`), before `RECONNECTED` is handed to the read loop (`:3878`) and so before `start_session` sends that connection's `auth`.

In loop A the answer was handled on time, on socket 1 (H2 and H3 falsified). The `subscribe` was then replayed from that buffer onto socket 2. Loop B shows the issue's late answer leads to the same place by the direct path. Both paths come from the same unbound send. Binding the frames to the answer's epoch made both loops green, and taking it out made both red. That probe is evidence only. The fix belongs to box 3 Planning. nautilus-network also has a gate for its replay buffer (`AuthTracker`, `reconnect_buffer_waits_for_auth`) that the adapter does not use.

## Where the check goes

The check is saved at `repro/src/websocket/client.rs.txt`, a whole copy of `src/websocket/client.rs` at `5c63bd5` that differs only inside `mod tests`. It adds the `late_auth` field to `MockVenue`, 10 lines in `MockVenue::serve`, and the test `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth`. The `.txt` suffix keeps test runners from picking it up. It was run against unfixed code and failed 20 of 20 times, and the rest of the unit suite stayed green (143 passed, 1 failed). That output is in `repro/red.txt`. It is formatted, and `cargo clippy --all-targets -- -D warnings` passes with it. With the H1 probe in place, it passed 20 of 20.

Why this seam: it drives `QfexWebSocketClient` through `new_trade`, `connect` and its stream, against the same stand-in the neighbouring trade socket tests use. It asserts what the issue asks for: the next socket gets its own `auth` first, and the session frames once.

This seam has a weakness, and that is a finding for box 3 Planning. **No public interface lets a test hold the read loop between reading a frame and handling it.** The check does it by taking the lock of the subscribe gate (`client.session.gate.state`). That only works because `read_loop` awaits that lock for every `subscribed` frame, on the trade socket too. If the fix changes how `read_loop` treats `subscribed` frames, the check stops holding anything and passes whether the bug is there or not. Box 3 has to decide whether to give the read loop a seam that a test can drive one frame at a time, with its connection epoch, and move the check onto that seam.

The path loop A actually took, the writer's replay buffer, cannot be forced from a test. When the writer takes a frame is decided inside nautilus-network. Loop A (`repro/loop-a.txt`) is the evidence for that path. A fix that binds the frames to the answer's connection covers both paths, as the H1 probe did.

## Why the gate missed this

Fix the architecture. The read loop throws away the connection epoch that nautilus-network gives every frame, and then sends with the one call that is allowed to cross connections. nautilus-network 0.67.0 offers `epoch_builder` with `send_text_on_connection` for exactly this case. The same shape hides the bug from tests: a test can only make it show by luck, or through an internal lock as the check does. A test alone (the check) would catch this one case. It would not stop the next frame sent with `send_text` from the read loop.

## Not covered

- **Orders and other requests around a reconnect.** `QfexWebSocketClient::send` (`src/websocket/client.rs:578`) also uses `send_text`. Going by the same nautilus-network code, an `add_order` or `cancel_order` sent while the trade socket reconnects can be replayed on the next socket before its `auth`. Nothing probed this. It is the same root cause, not a separate bug, so no issue was opened. Box 3 has to decide whether the fix covers it
- **The market data socket.** Its `start_session` and `subscribe_pump` use the same send. With no `auth` there, a replayed `subscribe` is only a repeat, so this was not probed
- **The real venue.** Whether QFEX refuses a `subscribe` before `auth`, and what the adapter does with that error, is unknown. `src/execution/mod.rs:467` treats only an error with no request type, or one for `auth`, as a refused login. Nothing here contacted QFEX
- **`nautilus-risex` and `nautilus-lighter` 0.67.0.** Their read loops were not read, so this run does not say whether they bind session frames to a connection. They are not on this machine
- **`cargo test` in the main checkout.** The first `./checks.sh` after the worktree was removed failed: the shared `CARGO_TARGET_DIR` held test binaries built in the worktree, with its deleted path compiled in. After `cargo clean -p nautilus-qfex`, `./checks.sh` passed (171 passed, as written). That was an artifact of the shared target directory, not of the code
