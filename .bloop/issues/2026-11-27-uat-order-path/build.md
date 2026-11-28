STATUS: BLOCKED
BLOCKED-ON: UAT steps 3 to 7 need a session with a two-sided book. Steps 1 and 2 passed on 28 Nov 2026 after the login fix `26750aa`; step 3 sent no order on Saturday (the `weekend` session, no quote). Next run: `AAPL-USD` pre-market, Monday 04:00 venue time. Slices 3 and 4 start when steps 3 to 7 and 9 are recorded

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `afc30c1` | `./checks.sh` (holds `cargo test --test exec_client recording_holds_the_frames_and_no_key`) | PASS, exit 0, 85 s: 180 passed (151 unit, 14 execution client), count rows as written. Red first: `no field record_path`. Mutation: with the `auth` redaction off the test fails on the signature in `sent[0]` | `evidence/slice-1.md` |
| 2 | `45a1d1c` | `./checks.sh` (holds `cargo test --features examples --example exec_tester`) | PASS, exit 0, 21 s: 193 passed with the feature (13 `exec_tester`), 180 without, both count rows as written. Red first: `cannot find type UatStep`. Also ran: step `9` prints the usage, exit 2, no `recordings/` created | `evidence/slice-2.md` |
| UAT fix | `26750aa` | `./checks.sh` (holds `cargo test --lib login_answer`) | PASS, exit 0: 197 with the feature, 184 without. Red first: 4 failed. Found by UAT step 2: the login answer is `{"authenticated":true}` | `evidence/uat-login-fix.md`, `evidence/uat-run-1.md` |
| UAT step 3 record | `c4c0eb9` | `./checks.sh` | PASS, exit 0 (docs only: the step 3 outcome in `docs/STATUS.md`) | `evidence/uat-step-3-no-quote.md` |

## Acceptance criteria

Copied from `spec.md`. Ticked only where evidence backs it.

- [x] 1. With `record_path` set, one JSON line per REST answer and per trade-socket frame sent or received evidence `evidence/slice-1.md`, `recording_holds_the_frames_and_no_key`
- [x] 2. The recording never holds the API key, the secret, any HMAC signature, the REST auth header values or `api_key=` evidence `evidence/slice-1.md`, the same test
- [x] 3. With `record_path` unset nothing is written and every existing test passes unchanged evidence `evidence/slice-1.md`
- [x] 4. `Debug` shows `record_path` and hides the secret evidence `debug_shows_the_record_path`, `debug_hides_the_secret`
- [x] 5. `exec_tester` cannot point at mainnet evidence `evidence/slice-2.md`, `every_step_runs_on_uat_and_records_to_its_own_file`, `no_step_an_unknown_step_or_mainnet_is_refused`
- [x] 6. "The UAT gate" gives one exact command per step evidence `docs/STATUS.md` "The UAT gate", "Commands" (read, at `45a1d1c`)
- [ ] 7. Fixtures carry the recorded shapes, no `signature`, `public_key` or `api_key` field: no evidence yet, slice 3 waits on the recording
- [ ] 8. G2 no longer says "now 0"; the five open questions answered: no evidence yet, slice 4 waits on the recording

## Notes for reviewers

- Worth a closer look: `src/common/recording.rs`, the redaction by kind (`sent` and `redact_echoed_auth`); `src/execution/mod.rs` `write_lines_until_closed`, which closes the receiver on `stop` and still writes the lines already handed over
- Decision: the recording file is opened in append mode, so a reconnect through a second `connect()` adds to the same file
- Decision: `self.recording` is set only after `connect()` succeeds. A failed connect drops every recorder, so its writer drains and ends by itself (the clean-comment pass found the stale handle in the first draft)
- Decision: the stand-in records every nonce and signature `check_auth` saw (`seen_nonces_and_signatures`), and the test searches the file for each one
- Comment pass: 7 comments deleted, 5 renames or extractions done (`write_lines_until_closed`, `append_line_flushed`, `trade_recorder`, `seen_nonces_and_signatures`, recording handle stored after success)
- Slice 2 deviation from the plan: step `2` exists as a dry run (connect, log in, reconcile, no order), so the person sees steps 1 and 2 pass before the first order. The plan named steps 3 to 7 only
- Slice 2: `checks.sh` (bloop-managed) changed in one regex. The no-feature count row now reads `180 passed: ...` instead of `The same 180`, because the 13 `exec_tester` tests are built with the `examples` feature only. Worth a look by whoever owns the guardrails
- Slice 2 comment pass: 6 module and function comments deleted; the step descriptions moved into `UatStep::what_it_does`, printed in the usage and at start
- Not done: `data_tester` still reads mainnet public data only; the plan does not ask for a UAT mode there
- UAT run 1 (28 Nov 2026) was run by the agent at the person's request in this session, with the key the person put in `.env`. The plan had the person run it. Steps 1 and 2 passed; the login fix `26750aa` is work the plan puts in slice 3, done early because steps 3 to 7 cannot run without it
- `MUST KILL` flags left unresolved: none
