VERDICT: PASS

Lens: security. Diff `92dca56..9cea260` on `fix/2026-11-27-trade-late-auth-answer`. Read: `AGENTS.md`, `spec.md`, `build.md`, `evidence/`, the diff, `docs/external/nautilustrader/README.md`, and the 0.67.0 sources of `nautilus-network` (`send_text_on_connection`, the writer's `SendOnConnection` arm, `epoch_builder`, the `RECONNECTED` hand-off, the reader fence on reconnect). No `companions.review` key in `.bloop/config.json`, so no companion skills ran. Nothing was executed; no finding needed a run to prove it.

## Findings

1. [should] src/websocket/client.rs:690-697 (`read_loop`, `Authenticated` arm, then `out_tx.send(message)`) — the loop now knows an `auth` answer belongs to a connection that is gone (`send_session_frames` drops its frames at line 642), but still forwards it upward as `QfexWsMessage::Authenticated`. On the first connect, `logged_in` in `src/execution/mod.rs:405` returns `Ok` on that stale answer, so the execution client reports itself logged in while the live connection has not yet sent or had answered its own `auth`. Severity 1: the venue refuses requests before `auth`, so nothing reaches another account or moves money; the effect is refused orders in that window, the same order path the restate left out, and the spec says on purpose that the execution client sees the same messages as today.
   Proof: the branch's own check `trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth` (src/websocket/client.rs, around line 1216) has to skip messages in a loop until `Reconnected`. Those skipped messages include the `Authenticated` of connection 0, which is the one `logged_in` would take. A fix would forward a stale answer as something other than `Authenticated`, or have the execution client wait for a login that carries the live epoch. That belongs with the order path issue, not this one.

## What the change gets right, checked

- **Who is asking.** Session frames (private `subscribe`, cancel on disconnect) go only on the epoch whose `auth` answer arrived, and are refused by the writer (`SendError::ConnectionChanged`) if that epoch was replaced in between (nautilus-network 0.67.0 `src/websocket/client.rs:2177-2184`). A stale `RECONNECTED` no longer puts a second `auth` on a later socket. The old reader is fenced and aborted before the replacement reader starts (`:1579-1587`), so a frame from epoch N cannot arrive after `RECONNECTED` of N+1 on the channel.
- **Fail-open check.** `start_session` returns `Ok(())` without sending `auth` in two cases: the epoch check at line 599, and `ConnectionChanged` at line 611. In both, the connection that replaced it gets a `RECONNECTED` with its own epoch, which is emitted only while that replacement is `Active` (`:3866-3874`), and sends its own `auth`. No path leaves a live connection that private frames reach without an `auth` first. An `auth` that fails or times out after the write began triggers a reconnect in the writer (`:2219-2230`), so the next connection logs in again and turns cancel on disconnect back on. A live socket is never left without cancel on disconnect.
- **What leaks.** The new log lines (lines 600, 612, 641-643, 651) carry no frame body, nonce, signature or epoch. The `SendError` variants carry no payload (`nautilus-network src/error.rs:24-50`). `send_text_on_connection` logs only the byte length at trace level. The `auth` frame is never logged.
- **Secrets.** No key, token or password in the diff. Tests use `fixed_signer()` with `FAKE_SECRET` from `src/signing`. A grep of `evidence/`, `build.md`, `debug.md` and `repro/*.txt` for secret, key, signature, token and `QFEX_` found only test names.
- **Venue contact.** `MockVenue` binds `127.0.0.1:0` (line 765). Neither new test names a host other than the stand-in.
- **Money and data.** The order path (`QfexWebSocketClient::send`) is unchanged. No operation in the diff can run twice: bound sends are never replayed.
- **Randomness and time.** Unchanged. The nonce and timestamp still come from `signer.auth()` on every `auth`.
- **Dependencies.** None added. `Cargo.toml` and `Cargo.lock` are unchanged in the diff.

## What I did not cover

- The order path left out by the restate: orders sent with `send` during a reconnect go through the ordinary send and its replay buffer, and can reach the next socket before its `auth` (written in `docs/STATUS.md`). `build.md` says no issue is open for it yet. If a later change gates that buffer on login, for example with `AuthTracker`, the replay can resubmit an order whose first write is undetermined. That issue should be reviewed for duplicate orders.
- The `api_key` query parameter in the trade socket URL (`new_trade`, line 417) is older than this diff and unchanged. I did not check whether the transport or the state sink logs that URL.
- Real venue behaviour: whether QFEX ties a connection to the account from the URL `api_key` before the HMAC `auth`, and whether it answers a refused pre-`auth` frame. `docs/STATUS.md` marks the trade client UNVERIFIED on the venue. This lens cannot check that without the venue contact that `AGENTS.md` forbids.
- `nautilus-risex` and `nautilus-lighter` are not on this machine, so I could not compare their epoch handling.
- I did not rerun the test suite, as this lens requires. The pass counts are taken from `evidence/`.
