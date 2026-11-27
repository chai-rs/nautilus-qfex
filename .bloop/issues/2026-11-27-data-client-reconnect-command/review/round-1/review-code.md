VERDICT: PASS

Lens: code. Diff `362bcb0..691abc8` on `feat/2026-11-27-data-client-reconnect-command`.
Companions: `.bloop/config.json` has no `companions.review` rows, so no extra skill was used.
Load assumed: one `QfexDataClient` per `LiveNode` process, one market data socket, reconnect commands sent by hand by an operator or controller (README and ARCHITECTURE describe a single-process adapter, no deploy fan-out). At that load there is no scale concern: the new lock is held only to read or swap one `Option<WebSocketReconnectHandle>`.

## What I checked and found sound

- `src/data/mod.rs:284-288` `market_data_socket` pairs `socket.sink()` and `socket.register(..)` on the same `SocketControl` (not a clone), in this order. Against nautilus-live 0.67.0 `src/socket.rs`: `sink_with` advances the generation and drops the old registration; `register` returns early on generation 0 or a stale generation. Sink first, then register, is the only order that registers. Both call sites (`new` at 322, `Feed::close` at 275) use the helper.
- `src/websocket/client.rs:488` the slot is filled only after `start_session` succeeds; on its failure the transport is disconnected and the slot stays empty (`connect()` opens with `self.disconnect()`, which empties it at 503). No path leaves a handle to a dead transport in the slot.
- `src/websocket/client.rs:395-405` the closure holds the `parking_lot` lock while `WebSocketReconnectHandle::request_reconnect` runs. That call may publish to the state sink synchronously (nautilus-network `client.rs:2585-2603`), which takes nautilus-live's `publish_lock`, never this slot's lock. The only other users of the slot (`connect`, `disconnect`) take it for one statement and hold no other lock at the time. No lock-order cycle; `SocketReconnectHandle::request_reconnect` (nautilus-live `socket.rs:78`) calls the function with no registry lock held.
- A request landing between line 488 and the spawn of `read_loop`: the `RECONNECTED` frame is buffered in `raw_rx` and `read_loop` resubscribes when it starts. No lost resubscribe.
- `stop()` / `reset()` / `dispose()` go through `stop_in_background` -> `Feed::close`, so a stopped client also answers `Closed`, not a stale `Accepted`.
- Dependency direction: `data` -> `websocket` -> `nautilus_network`; nothing new crosses a boundary. Execution client untouched (criterion 7).
- Tests: `mds_reconnect_request_follows_the_connection` pins `Closed` / `Accepted` / `Closed` and the resubscribe at the socket level; `reconnect_command_reconnects_the_market_data_socket` covers both registration sites (the second `connect()` runs on the socket built by `Feed::close`), with the 10 s timeouts of `next_state` / `next`, no fixed sleeps. Build notes record a mutation run removing `register` from `Feed::close` going red.
- `docs/STATUS.md` count (168 -> 170) and row 77 match the diff.

Considered and dropped: `tests/data_client.rs:581-585` asserts `assert_ne!(.., Accepted)` after `disconnect()` where the answer is deterministically `Closed`. A regression to another non-`Accepted` value would pass that line, but the same closure is pinned to `Closed` by the unit test at `src/websocket/client.rs:1027-1031`, so I could not build a mutant that slips through both. Not a finding.

## What I did not cover

- Did not rerun the suite or compile; relied on `evidence/slice-1-rerun.md` and `evidence/slice-2.md`.
- Did not exercise a full `LiveNode` `ReconnectSocket` command; read `nautilus-live` `node/mod.rs` mapping only through the spec's citation, as the spec's assumption does.
- The deferred rewrites of `ARCHITECTURE.md` step 7 and `docs/external/nautilustrader/README.md` (spec slice 2, held back for PR #4) are a spec/docs completeness question for the spec lens, not code; they must land before merge as `build.md` says.
- Behaviour when the transport exhausts its own reconnect attempts and what `request_reconnect` then answers is nautilus-network's, not reviewed.
- Security aspects belong to `reviewer-security`.
