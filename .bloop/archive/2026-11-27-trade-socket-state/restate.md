CONFIRMED: yes

# Trade socket publishes its connection state

## What we will do

- An actor subscribed to socket state receives `Connected` when the QFEX execution client's trade socket comes up, `Disconnected` when that socket is lost, and `Connected` again once the adapter has reconnected.
- `Connected` means the transport is up, as NautilusTrader defines it. It does not wait for the `auth` answer. The login after a reconnect keeps working as today: the client logs in again, catches up, and stops with an error if QFEX refuses the login.
- Every one of those events names the execution client's `client_id`, venue `QFEX` and one stable trade endpoint label, different from `market_data`.
- This keeps working after the execution client disconnects and connects again: the fresh trade socket it builds on each connect still reports its state.
- A test against the local mock venue in `tests/exec_client.rs` proves the sequence connect, drop, reconnect, using dummy keys and `127.0.0.1` only.
- All 168 existing tests still pass, and `./checks.sh` passes.
- `docs/STATUS.md` records the new behaviour, the new test count, and what a controller reconnect command for the execution client answers under a `LiveNode`. If the socket control is built while the node constructs the client, that answer becomes `UnknownEndpoint` instead of `Unsupported`, as it did for the data client, and the row names a follow-up. `ARCHITECTURE.md` step 7 of the data flow no longer says only the market data socket publishes state.

## What we will not do

- Holding `Connected` back until the login succeeds: the framework's state is about the transport, and the market data socket uses it the same way.
- Registering a reconnect handler for the trade socket: a commanded reconnect would force a re-login and a catch-up, so it belongs in its own issue after `2026-11-27-data-client-reconnect-command` settles the pattern.
- Changing the reconnect log line, the login, the catch-up or any order handling: the event is added next to them.
- Any contact with the live QFEX venue: no authenticated request and no order leave the machine.

## Contracts affected

- `SocketStateChanged` (NautilusTrader system event) gains a publisher: the trade endpoint, venue `QFEX`
- `ReconnectSocket` system command: the outcome for the QFEX execution client may change from `Unsupported` to `UnknownEndpoint`, depending on where planning builds the socket control; recorded either way
