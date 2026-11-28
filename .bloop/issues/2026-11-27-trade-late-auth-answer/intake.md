DECISION: ACCEPT
TYPE: fix
FR: none
RISK: high
SURFACES: security
RESCOPED: yes

## Summary

- `debug.md` is `ROOT-CAUSE`, proven both ways: the private `subscribe` frames that `read_loop` sends on an `auth` answer are not bound to the connection that answered, so nautilus-network 0.67.0 can deliver them on the next trade connection before that connection's `auth`. Binding them made both loops green; taking it out made both red
- Accepted as a `fix` to the trade socket's session frames: they go out on the connection whose `auth` succeeded, or not at all. One pull request in `src/websocket/client.rs`, with the check `debug.md` saved in `repro/`
- Rescoped: `issue.md`'s first line asks that no trade socket frame at all goes out before that socket's `auth`. Orders and other requests sent through `QfexWebSocketClient::send` during a reconnect share the root cause but are left out, because what an order sent mid-reconnect should do is an order-state decision, not a transport fix
- Risk is high: the change sits in the login and session path of the socket that carries orders, and it is a concurrency fix whose bad path a test reaches only through an internal lock
- Handed to box 3 Planning: whether to bind with `epoch_builder` and `send_text_on_connection` or to use nautilus-network's `AuthTracker` gate, and whether the read loop gets a seam a test can drive one frame at a time, since the saved check holds the loop through the subscribe gate's lock (`debug.md`, "Where the check goes")

## Why

`connect` builds the frame channel with `channel_message_handler()` (`src/websocket/client.rs:471`), which drops the connection epoch of each frame, and the `Authenticated` arm of `read_loop` (`:656-661`) sends `session.frames` with `client.send_text`, the one send that may cross connections. A `subscribe` before `auth` reaching QFEX would at best be refused and at worst leave the private channels unsubscribed after a reconnect, which FR-7 (reconnect and catch up) depends on; `docs/STATUS.md` already lists it as a known race. `QfexWebSocketClient::send` (`:572-577`) uses the same send for orders, but making it safe means choosing whether an order sent during a reconnect fails, waits for the next login, or is replayed, and that choice changes what a strategy sees, so it belongs in its own issue.
