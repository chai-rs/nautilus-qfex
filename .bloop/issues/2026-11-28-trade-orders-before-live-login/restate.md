CONFIRMED: no

# Trade requests and the login wait for the live connection's own auth

## What we will do

- A request the execution client sends on the trade socket (a new order, a cancel, a modify, the open orders query) goes out only on a connection whose own `auth` the venue has answered with success. Sent while the socket is reconnecting, it waits for the replacement's login and is then written once, on that connection, after its `auth` and not before
- A request is never carried over to another connection by itself, so an order is never sent twice. If the connection it was bound for goes away first, or the login does not come in time, a request that surely did not leave ends in the rejection event the strategy sees for it (order, cancel or modify); one whose write is undetermined stays submitted and is settled by the engine's in-flight check, as it is today. None is dropped without an event
- An `auth` answer that belongs to a connection already replaced is never a login: it is logged and goes no further. `connect` reports the execution client logged in only once the live connection's own `auth` has succeeded, and after a reconnect the catch-up starts only on the live connection's login, not on a late answer of the old one
- The check `debug` saved passes: `in a throwaway worktree at origin/main (4134e4c), cp .bloop/issues/2026-11-28-trade-orders-before-live-login/repro/src/websocket/client.rs.txt src/websocket/client.rs && cargo test --lib -- trade_order_sent_during trade_late_auth_answer_is_not` goes green with the fix, with both tests moved into `src/websocket/client.rs` (or onto a seam box 3 chooses), and goes red again when the fix is taken out. The unforced variant of loop A (`evidence/loop-a-unforced.patch`) stays green too
- PR #16's checks stay green, among them `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth`, which then no longer has to skip a stale `Authenticated` before `Reconnected`
- The trade client row of `docs/STATUS.md` stops naming this as a known gap and says how orders and the login behave across a reconnect, correcting "can be replayed" to what `debug.md` found (the order waits out the reconnect and is written before the next `auth`). The test counts in `README.md` and `docs/STATUS.md` follow the added checks
- Everything is proven against the local stand-ins on `127.0.0.1`; nothing contacts the QFEX venue

## What we will not do

- The window where the read loop handles the old connection's `auth` answer before the transport has noticed the drop. The answer still looks live to any epoch check; it is inherent, and the `Reconnected` that follows already resets the login in `consume`
- Forcing nautilus-network's replay buffer path in a test. `debug.md` (H2) shows it cannot be forced and that the reported order does not take it; the fix has to keep that path from writing a request before `auth` too, which box 3 Planning checks in the 0.67.0 source rather than by a run
- The market data socket. It has no login, so a frame written before its session start is only a repeat
- What QFEX answers to an order or a login before `auth`. Finding out needs the live venue, which this work never contacts
- A read loop seam a test can drive one frame at a time. Both saved tests hold the subscribe gate's lock, as PR #16's do; box 3 Planning decides whether to add the seam here or keep the lock
- Bumping or patching nautilus-network. The fix uses what 0.67.0 already offers, such as `send_text_on_connection` and the connection epoch

## Contracts affected

none. The frames sent to QFEX are the same; only which connection carries them and when changes. Inside the crate, `QfexWsMessage::Authenticated` and the error that `QfexWebSocketClient::send` returns may change shape, which is not an outside contract.
