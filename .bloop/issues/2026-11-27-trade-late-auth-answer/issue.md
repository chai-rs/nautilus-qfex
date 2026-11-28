FR: none
KIND: bug
GOAL: none

The QFEX execution client never sends a trade socket frame before that socket's own `auth` has succeeded, even when the socket drops right after a login.

## What was seen

- Retro of `2026-11-27-trade-socket-state`. `socket_state_follows_the_trade_socket` in `tests/exec_client.rs`, as merged in `ce9d457`, failed 1 run in 100 on the main checkout (27 Nov 2026, Linux, `CARGO_TARGET_DIR=target`). The stand-in recorded `venue refused: ["subscribe before auth"]`: a `subscribe` frame reached a trade connection that had not yet received `auth`.
- With the stand-in's drop made reliable (`notify_one` through `Venue::drop_socket`) and fired right after the first `Connected`, the same failure came 2 runs in 100. Each failure printed the same message, and no failure printed anything else.
- The retro changed the test to wait for the first `subscribe` before dropping, as `Harness::connect` already does. After that it passed 100 runs in 100. So the test no longer shows the race, and this issue does.

## Where it probably is (a hypothesis, not proven)

- `src/websocket/client.rs`, `read_loop`: on `QfexWsMessage::Authenticated` it sends `session.frames` (the private channel `subscribe` frames) with `client.send_text`, which writes to whichever socket the transport holds at that moment.
- If the venue answers `auth` on socket 1 and socket 1 drops before `read_loop` handles that answer, the transport may already be on socket 2. The late answer from socket 1 then sends `subscribe` on socket 2 before `RECONNECTED` is handled and socket 2's `auth` goes out.
- At the real venue this would show up as a refused `subscribe` after a reconnect, followed by a correct `subscribe` once the new `auth` succeeds. Whether QFEX refuses it, and what the adapter does with that error, is not known: `src/execution/mod.rs` treats only an error with no request type, or one for `auth`, as a refused login.

## What is asked

- `/bloop:debug` proves or rejects the hypothesis with a check that fails on purpose. The test drops the socket between the `auth` answer and its handling, without depending on luck.
- Then a fix, so that no frame from an earlier connection's session goes out on a later one.

## Constraints

- No authenticated request and no order to the live QFEX venue. The local stand-in in `tests/exec_client.rs` or the one in `src/websocket/client.rs` tests is enough.
- `socket_state_follows_the_trade_socket` keeps its wait for the first `subscribe`. It tests socket state, not this race.
