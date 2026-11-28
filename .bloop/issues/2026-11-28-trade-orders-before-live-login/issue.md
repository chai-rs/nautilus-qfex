FR: none
KIND: bug
GOAL: none

An order the QFEX execution client sends on the trade socket only reaches a connection that has logged in with its own `auth`, and the execution client reports itself logged in only once the live connection has.

## Where it comes from

- The user answered "เปิด issue เลย" ("open the issue now") to point 3 of `2026-11-27-trade-late-auth-answer/decision.md` ("## Why escalated"): agree a box 2 issue for the order path and the stale `Authenticated` before PR #16 merges, so the only record is not `docs/STATUS.md`.
- `2026-11-27-trade-late-auth-answer/build.md`, "Notes for reviewers": "Left out on purpose (restate): orders sent with `send` during a reconnect still use `send_text` and can be replayed before the next `auth`. Not opened as an issue yet".
- `2026-11-27-trade-late-auth-answer/review/round-1/review-security.md`, finding 1 `[should]` and "What I did not cover", first bullet.

## What is known (read, not proven by a run)

1. **Orders can reach the next connection before its `auth`.** Orders go out with the ordinary `send` / `send_text` of `QfexWebSocketClient`, which is not bound to a connection. During a reconnect, nautilus-network 0.67.0 keeps such a send in its replay buffer and writes it to the replacement connection as soon as that is active, before `RECONNECTED` reaches the frame handler, so before that connection's `auth` goes out (`docs/external/nautilustrader/README.md`, "`send_text` is not bound to a connection"). The venue refuses requests before `auth`, so the effect expected is a refused order right after a reconnect. Whether QFEX answers such a frame, and with what, is not known (`docs/STATUS.md` marks the trade client UNVERIFIED on the venue).
2. **A stale `Authenticated` counts as a login.** After PR #16, `read_loop` in `src/websocket/client.rs` (`Authenticated` arm, around lines 690-697 on `9cea260`) knows an `auth` answer belongs to a connection that is gone and drops its session frames, but still forwards it upward as `QfexWsMessage::Authenticated`. On the first connect, `logged_in` in `src/execution/mod.rs` (around line 405) returns `Ok` on that stale answer, so the execution client reports a login while the live connection has not yet sent or had answered its own `auth`. The branch's own check `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth` has to skip that message in a loop until `Reconnected`.

## What is asked

- `/bloop:debug` proves or rejects each of the two with a check that fails on purpose against the local stand-in.
- Then a fix, so that no order goes out on a connection before that connection's own `auth` has succeeded, and `logged_in` waits for a login that belongs to the live connection. The security lens offered two shapes for point 2: forward a stale answer as something other than `Authenticated`, or have the execution client wait for a login that carries the live epoch. Decide in restate and plan.
- `docs/STATUS.md` (rows for the trade client) and `README.md` stay true.

## Constraints

- **Duplicate orders.** The security lens of `2026-11-27-trade-late-auth-answer` asks that a fix which gates or drops the replay buffer (for example on login, through `AuthTracker`) be reviewed for duplicate orders: the replay can resubmit an order whose first write is undetermined. An order must never be sent twice, and never silently dropped without an event the strategy sees.
- **Depends on PR #16** (`fix/2026-11-27-trade-late-auth-answer`), not yet merged. The epoch-bound session frames, `send_text_on_connection` and the line numbers above come from that branch (`9cea260`). Restate waits for it, or works from that branch.
- No authenticated request and no order to the live QFEX venue. The stand-in `MockVenue` on `127.0.0.1` in `src/websocket/client.rs` tests, or the one in `tests/exec_client.rs`, is enough.
