VERDICT: PASS

Lens: security. Round 2 is `9cea260..575c901` on `fix/2026-11-27-trade-late-auth-answer`, and the whole change is `92dca56...575c901`. Read: `AGENTS.md`, `build.md` `## Round 2`, `evidence/round-2.md`, `evidence/round-2-e2e.md`, the round 2 diff, `start_session` at `src/websocket/client.rs:593-618` as it stands at `575c901`, and the follow-up issue `2026-11-28-trade-orders-before-live-login/issue.md`. `.bloop/config.json` has no `companions.review` key, so no companion skills ran. Nothing was built or run. No finding needed a run to prove it.

Round 2 changes only a unit test, the test-only `MockVenue`, and two rows of `docs/STATUS.md`. It changes no production code, so the attack surface I reviewed in round 1 is the same.

## Checked in round 2

- **The fail-open branch is now proven by a run.** In round 1, my "Fail-open check" relied on reading the code to argue that `start_session`'s `SendError::ConnectionChanged` arm (`src/websocket/client.rs:611-614`) never leaves a live trade connection without `auth`. The new check `trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in` (`:1194`) now runs that branch, and its assertions prove what this lens needs:
  - The arm returns `Ok` only for `ConnectionChanged`. Any other error goes through `sent => Ok(sent?)` and would fail `is_ok`. The check is green on the branch and red under M1, where the arm is removed. So the call really took that arm, and did not take a successful send on epoch 1.
  - The epoch-0 `auth` was not written to the next socket. Socket 2 receives exactly `["auth", "subscribe"]` (`:1238-1241`), and M2 (`send_text` put back) gives `["auth", "auth", "subscribe", "subscribe"]`. A connection never gets two `auth` frames, or private frames, from a stale call.
  - The replacement connection logs in on its own. The check gets `Reconnected` and then `Authenticated`, and `auth` comes before `subscribe` on socket 2. No live connection is left without `auth`.
- **Test-only code.** `next_upgrade_gate` (`:741`, `:755`) sits inside `#[cfg(test)] mod tests` (`:701`), so no lock or delay reaches a production upgrade path.
- **Venue contact.** The new check uses `MockVenue` only, which binds `127.0.0.1:0` (`:767`). It names no other host.
- **Secrets.** The check uses `fixed_signer()`. I searched `evidence/round-2.md` and `evidence/round-2-e2e.md` for secret, signature, api_key, token, `QFEX_`, bearer, password and long hex strings. The only hits are test names (`debug_hides_the_secret`) and the hashes of the target binaries. The round 2 diff contains no key, token or password.
- **What leaks.** Round 2 adds no log line. The two warnings in `start_session` are unchanged from round 1 and carry no frame body, nonce or signature.
- **Dependencies.** None added. The whole change touches only `docs/STATUS.md`, `src/websocket/client.rs` and `tests/exec_client.rs`. `Cargo.toml` and `Cargo.lock` are unchanged.
- **Money and data.** The order path (`QfexWebSocketClient::send`) is still unchanged. Nothing new can run twice.

## Checked from last round

- #1 from round 1 (`[should]`: a stale `Authenticated` is still forwarded upward, so `logged_in` can report a login before the live connection has one): not addressed in this branch, and still true in the code. It is now `read_loop` at `src/websocket/client.rs:690` and `logged_in` at `src/execution/mod.rs:409`. This was the agreed outcome: it moved, with the order replay gap, to `2026-11-28-trade-orders-before-live-login`. That issue states both points and repeats my constraint that a fix to the replay buffer must be reviewed for duplicate orders. Severity is unchanged at 1, so it does not block this branch. That issue file exists only on the unmerged records branch `chore/2026-11-28-auto-run-records`, the same branch the approver's point 2 (criterion 10) is waiting on. Until that branch merges, the only record on `main` is the `docs/STATUS.md` row, which now names the issue id.

## What I did not cover

- I did not run the new check or the mutations. The 20 green runs and the M1 and M2 results come from `build.md`, which says the 20 runs came before the rename from `upgrades_held` to `next_upgrade_gate`. Both gates ran after the rename. `evidence/round-2.md` says it ran on `9cea260` with uncommitted changes, and `evidence/round-2-e2e.md` covers the commit.
- Timing. The check uses a 200 ms sleep to show that the `auth` is still waiting. Whether that is reliable on a slow CI runner belongs to `reviewer-code`, not this lens.
- The check runs the arm on the trade socket only with `cancel_on_disconnect` set to `false` (`new_trade(.., false)`). It does not show by a run that cancel on disconnect is turned back on for the next connection on this path. That still rests on my round 1 reading: the frame is sent from that connection's own `Authenticated`, through the same `send_session_frames` that `trade_socket_authenticates_then_starts_the_session_again_after_reconnect` covers.
- These are carried over from round 1 and still open:
  - the `api_key` query parameter in the trade socket URL, and whether anything logs that URL
  - how the real venue behaves before `auth`, which `AGENTS.md` forbids testing live
  - `nautilus-risex` and `nautilus-lighter` are not on this machine, so I could not compare against them
