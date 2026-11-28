DECISION: ACCEPT
TYPE: fix
FR: none
RISK: high
SURFACES: security
RESCOPED: no

## Summary

- `debug.md` is `ROOT-CAUSE`, proven both ways on `origin/main` (`4134e4c`, the merge of PR #16, so the dependency in `issue.md` is met): an order sent with `QfexWebSocketClient::send` during a reconnect reaches the next trade connection before that connection's `auth` (H1, 20/20 forced, 50/50 unforced), and an `auth` answer of a replaced connection reaches the stream as `QfexWsMessage::Authenticated`, which `logged_in` takes as the login (H4, H6, 20/20)
- Accepted as one `fix` with both claims, since they share one cause: nothing on the trade socket ties a request, or the login signal, to the login of the connection that is live now
- Point 2's shape is decided here as behaviour: an answer from a replaced connection is never a login, neither for `logged_in` at connect nor for `consume` after a reconnect. Whether the read loop drops it or the login signal carries its epoch (or a login state the client owns, as `debug.md` "Why the gate missed this" suggests) is left to box 3 Planning
- Risk is high: the change sits in the login path of the socket that carries orders, it is a concurrency fix whose bad path a test reaches only through an internal lock, and it decides what a strategy sees for an order sent mid-reconnect (never sent twice, never dropped without an event)
- Not a rescope: `issue.md` asks for both claims, the duplicate-order constraint and true docs, and all of that is in. What stays out is what `debug.md` "Not covered" already places outside this issue (the live venue, the market data socket, the window before the transport notices a drop)

## Why

`send` (`src/websocket/client.rs:573-579`) writes every trade request, orders, cancels, modifies and `GetUserOrders` alike, with `client.send_text`, which waits out a reconnect in nautilus-network's `wait_for_active` and writes the frame as soon as the replacement is active, before `start_session` sends that connection's `auth`. The `Authenticated` arm of `read_loop` (`:690`) checks the epoch only to drop the session frames and forwards the message anyway, and `logged_in` (`src/execution/mod.rs:399-414`) returns on the first `Authenticated`, so `connect` (`:758`) goes on to the REST `set_leverage` calls and reports the client connected while the live connection has not logged in. A refused order right after every reconnect and a login reported for a dead connection are worth a pull request; PR #16 already gave the socket `send_text_on_connection` and connection epochs to build on, and `docs/STATUS.md` (trade client row) names this issue as the known gap.
