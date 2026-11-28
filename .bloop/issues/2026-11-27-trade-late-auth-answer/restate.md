CONFIRMED: yes
CONFIRMED-BY: trustLevel autonomous

# Trade socket session frames stay on the connection that logged in

## What we will do

- When the venue answers `auth` with success on a trade connection, the private channel `subscribe` frames are sent on that same connection and on no other
- If that connection is gone by the time the answer is handled, or before the frames are written, the frames are dropped with a log line instead of being carried over to the next connection
- A replacement trade connection always gets its own `auth` as its first frame, and gets the private `subscribe` frames once, only after its own `auth` succeeds
- The check saved by `debug` passes: `in a throwaway worktree at main, cp .bloop/issues/2026-11-27-trade-late-auth-answer/repro/src/websocket/client.rs.txt src/websocket/client.rs && cargo test --lib trade_late_auth_answer` goes green, with the check moved into `src/websocket/client.rs` (or onto a seam box 3 chooses), and it goes red again when the fix is taken out
- `socket_state_follows_the_trade_socket` keeps its wait for the first `subscribe`, and the reported shape (`repro/loop-a.txt`, run with the whole `exec_client` binary) no longer shows `subscribe before auth`
- The `websocket` trade client row of `docs/STATUS.md` stops listing the race as known and not fixed, and the test counts in `README.md` and `docs/STATUS.md` follow the added check
- Everything is proven against the local stand-ins on `127.0.0.1`; nothing contacts the QFEX venue

## What we will not do

- Orders and other requests sent with `QfexWebSocketClient::send` while the trade socket reconnects. Same unbound send, but whether such an order fails, waits for the next login or is replayed is an order-state decision that changes what a strategy sees. It gets its own issue
- The market data socket's `start_session` and subscribe pump. They use the same send, but with no `auth` there a replayed `subscribe` is only a repeat
- What QFEX itself does with a `subscribe` before `auth`, and how the adapter handles that error. Finding out needs the live venue, which this work never contacts
- The trade socket's reconnect command answering `Unsupported`. That is `2026-11-28-trade-socket-control-shape`
- Bumping or patching nautilus-network. The fix uses what 0.67.0 already offers

## Contracts affected

none. The frames sent to QFEX are the same; only which connection carries them changes.
