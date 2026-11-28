REVIEWER: security, code
MODEL: opus
APPROVED: yes
APPROVED-BY: user, in the session ("แทน id", 2026-11-28)

# The order path has run on QFEX UAT

## Summary

- Slice 1 gives `QfexExecClientConfig` an optional recording path: when set, the execution client writes every REST answer and every trade-socket frame it sends or receives to that file, one JSON line each, and never writes the API key, the secret, the HMAC signature, the REST auth headers or the trade-socket URL (which carries `?api_key=`); a test against the local stand-in proves both halves
- Slice 2 makes `examples/exec_tester.rs` runnable for the UAT gate without editing code: a step argument picks the tester setup of each step, pins both configs to `QfexEnvironment::Uat`, turns recording on, and `docs/STATUS.md` "The UAT gate" gets the exact command for each of the nine steps
- Then the person runs the nine steps themselves with their UAT key and hands back the recording files; no agent sends anything to QFEX, UAT or mainnet, and no agent reads the key
- Slice 3 replaces hand-made fixtures in `tests/fixtures/` with recorded frames where they differ and fixes the code where the venue differs from the docs; slice 4 writes the answers into `docs/STATUS.md`, `docs/PLAN.md`, `docs/external/qfex/README.md` and G2 of `ARCHITECTURE.md`
- Slices 1 and 2 can merge before the run; slices 3 and 4 wait for the recording

## Context

Code is read at origin/main (`4b46b86`, 170 tests); the source tree of this checkout is the same. Line numbers are from there.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `src/config/exec.rs` | `QfexExecClientConfig` (line 16) with `environment`, URLs, `api_key`, `api_secret`, `cancel_on_disconnect`, `leverages`; manual `Debug` (line 58) redacts the secret; `debug_hides_the_secret` test | must change: one new field `record_path: Option<PathBuf>`, default `None`, shown in `Debug` |
| `src/python/config.rs` | `py_new` of `QfexExecClientConfig` (line 87) lists every field by name | must change: set `record_path: None`; the Python constructor gains no argument (assumption below) |
| `src/common/mod.rs` | declares `consts`, `credential`, `enums`, `parse`, `urls`; `common` does no I/O (`AGENTS.md`) | must change: a new `recording` module holding only the record line type and a cloneable handle that hands lines to a channel; no file access in `common` |
| `src/execution/mod.rs` | `connect()` (line 716) resolves the credential, builds `QfexHttpClient::new` (line 728) and `QfexWebSocketClient::new_trade` (line 750), waits in `logged_in` (line 398), then spawns `consume` (line 416) | must change: when `config.record_path` is set, open the file, spawn the writer task that owns it, and hand the recording handle to both clients; close the writer on `disconnect()` |
| `src/http/client.rs` | `QfexHttpClient` (line 126); `get` (line 392) signs each attempt and parses the response body; `set_leverage` (line 261) posts and reads only the status; `check_status` (line 437) turns an error answer into `QfexHttpError` | must change: with a handle set, record method, endpoint, query params, status and body of every answer, including error answers and each retry; never the headers. Reuse the existing `get` and `set_leverage` paths, do not add a second request path |
| `src/websocket/client.rs` | `new_trade` (line 413) puts `api_key` in the URL; `start_session` (line 593) sends `auth.ws_message()` (lines 599-602); `send` (line 573) sends order requests; `read_loop` (line 625) receives every text frame and re-sends the session frames after `auth` | must change: with a handle set, record every received text frame and every sent frame; the `auth` frame is recorded as a fixed redacted line, and the URL is never recorded. The market data socket (`new_mds`) records nothing |
| `src/signing/mod.rs` | `QfexAuth` holds `signature` (line 27); `headers()` (line 52) and `ws_message()` (line 63) carry it | reuse, do not rewrite: the redaction relies on these two being the only places the signature leaves the signer |
| `src/common/credential.rs` | `QfexCredential::resolve` reads `QFEX_UAT_API_KEY` / `QFEX_UAT_API_SECRET` on UAT; `Debug` redacts the secret | reuse, do not rewrite: the example keeps reading the key this way, never from a file |
| `tests/exec_client.rs` | local stand-in REST + trade socket on `127.0.0.1` (`Venue`, line 85, `check_auth` checks every signature); `Harness` (line 366) builds the client from a config with `base_url_http` / `base_url_ws`; `order_is_accepted_filled_and_cancelled` (line 583) | reuse, do not rewrite: the new test sets `record_path` to a temporary file in the same harness, runs connect, one submit and one cancel, then reads the file |
| `examples/exec_tester.rs` | `DRY_RUN = true` (line 28), a comment that says it trades on mainnet; data and exec configs are `Default`, so `Mainnet` (lines 57, 62); one fixed `ExecTesterConfig` with post-only limits | must change in slice 2: a step argument picks the `ExecTesterConfig` of UAT steps 3 to 7, both configs use `QfexEnvironment::Uat`, recording on; the example refuses to start without a step |
| `Cargo.toml` | `[[example]] exec_tester` (line 85), `required-features = ["examples"]`, no `test` key | must change in slice 2 only if the step mapping gets unit tests in the example file (`test = true`) |
| `bin/flatten.rs` | `qfex-flatten uat` reads the key from the environment (line 192) and prints its plan before Enter | reuse: step 9 of the gate as is; it records nothing |
| `tests/fixtures/ws/trade.jsonl` | hand-made trade-socket frames from the docs (ACK with empty `client_order_id`, `MODIFIED`, `CANCELLED`, fill with `REDACTED` ids, three error shapes) | must change in slice 3 where the recording differs |
| `tests/fixtures/http/user_positions.json` | hand-made /user/positions answer | must change in slice 3 where the recording differs; the same for `tests/fixtures/http/user_trades.json`, `tests/fixtures/http/historic_orders.json` and `tests/fixtures/http/user_fees.json` |
| `docs/STATUS.md` | "The UAT gate" (line 269) lists the nine steps; "Not verified against the venue" lists the assumptions; Verification rows say 170 passed | must change: the commands in slice 2, results in slice 4, the counts in every slice that adds a test |
| `docs/PLAN.md` | "Open questions" (line 111): ACK echo, `CANCELLED` after `MODIFIED`, stop order listing, refused login, error shapes | must change in slice 4 with the answers the run gave |
| `ARCHITECTURE.md` | G2 (line 22) "4 of 4 (now 0)", "when a UAT key is available" | must change in slice 4 |
| `docs/external/qfex/README.md` | owner, what we use, "What we cannot rely on", what we do when it is down | must change in slice 4: that list shrinks by what UAT confirmed; complete for planning, no change now |
| `docs/external/nautilustrader/README.md` | lists `nautilus-testkit` testers as a dev dependency | reuse: `ExecTesterConfig` 0.67.0 already has `modify_orders_to_maintain_tob_offset`, `open_position_on_start_qty`, `enable_stop_buys` / `enable_stop_sells` and `cancel_orders_on_stop`, which steps 4 to 6 need |
| `README.md` | says to start on UAT and to do the UAT steps of `docs/STATUS.md` | must change in slice 2 (how to run a step with recording) and slice 4 (state) |

This work depends on QFEX UAT (`docs/external/qfex/README.md`) and on NautilusTrader 0.67.0 (`docs/external/nautilustrader/README.md`, nautilus-testkit 0.67.0 src/testers/exec/config.rs). Planning sent nothing to QFEX. The tests use the stand-in on `127.0.0.1` only.

Nearby work not yet merged: PR #8 (origin/feat/2026-11-27-trade-socket-state) changes `connect()` in `src/execution/mod.rs` around line 750 (a `SocketControl` and `with_state_sink` on the trade client) and takes the count to 171. Slice 1 touches the same lines; whichever lands second rebases them and the two count rows of `docs/STATUS.md`.

**This change has to reach** callers: `examples/exec_tester.rs`, `src/factories/exec.rs` (passes the config through unchanged), `src/python/config.rs` · tests: `tests/exec_client.rs`, the unit tests of `src/config/exec.rs` · fixtures: `tests/fixtures/ws/trade.jsonl`, `tests/fixtures/http/` (slice 3) · config: `QfexExecClientConfig::record_path`, `.gitignore` for the recording folder · exports: the new `common::recording` module.

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | `record_path` in `QfexExecClientConfig`; record line type and handle in `common::recording`; writer task in `execution`; `QfexHttpClient` records every answer (endpoint, params, status, body, no headers); trade `QfexWebSocketClient` records every frame sent and received, the `auth` frame as a fixed redacted line, never the URL; one test against the stand-in; test counts in `docs/STATUS.md` | `cargo test --test exec_client recording_holds_the_frames_and_no_key`: after connect, one submit and one cancel the file holds the `/user/positions` answer, the `auth` success frame, the sent `add_order`, the `ACK` and the `CANCELLED` row, and holds neither `API_KEY`, `API_SECRET`, any signature the stand-in checked, nor `api_key=` | done |
| 2 | `exec_tester` takes a step argument (`3` post-only submit and cancel, `4` modify, `5` market order and fill, `6` stop order, `7` resting order for the drop), pins data and exec configs to `QfexEnvironment::Uat`, sets `record_path` to `recordings/uat-step-<n>.jsonl` (folder in `.gitignore`), refuses to start without a step; `docs/STATUS.md` "The UAT gate" and `README.md` give the exact command for each of the nine steps (steps 1 and 2 are the connect of any run, step 8 is slice 3, step 9 is `qfex-flatten uat`) | `cargo test --features examples --example exec_tester`: the step mapping gives the expected `ExecTesterConfig` flags and `QfexEnvironment::Uat` for steps 3 to 7 and an error for no step, mainnet or an unknown step; `cargo build --examples --features examples` passes | done |
| — | **Person, not an agent:** runs steps 1 to 7 and 9 on `qfex.io` with their UAT key, approving each, and puts the files of `recordings/` where the agent can read them. Build waits here (`WAITING`) | the recording files exist; no agent command contacts QFEX | waiting |
| 3 | Fixtures from the recording: `tests/fixtures/ws/trade.jsonl` and the files of `tests/fixtures/http/` take the recorded frames where they differ, user and account ids replaced; code fixed where the venue differs from the docs, with the test that shows it | `cargo test --test exec_client` passes on the new fixtures; `git diff --stat tests/fixtures` shows only recorded lines; `grep -rE '"(signature|public_key|api_key)"' tests/fixtures` finds nothing | todo |
| 4 | Results into the docs: each step of "The UAT gate" in `docs/STATUS.md` with its date and what came back, the matching lines of "Not verified against the venue" removed or kept; answers in "Open questions" of `docs/PLAN.md`; "What we cannot rely on" of `docs/external/qfex/README.md` shrunk; G2 in `ARCHITECTURE.md` moved from 0 of 4 | `grep -n "now 0" ARCHITECTURE.md` finds nothing on the G2 row; each numbered step under "The UAT gate" in `docs/STATUS.md` names a date and a result; `grep -n "Does an ACK row echo" docs/PLAN.md` shows the line answered | todo |

## Acceptance criteria

1. With `record_path` set, a run of the execution client against the stand-in writes one JSON line per REST answer and per trade-socket frame sent or received, verified by `recording_holds_the_frames_and_no_key` in `tests/exec_client.rs`
2. The recording never holds the API key, the secret, any HMAC signature, the REST auth header values or `api_key=` from the trade URL, verified by the same test, which searches the file for each value the stand-in saw
3. With `record_path` unset nothing is written and every existing test still passes unchanged, verified by `cargo test` with the existing `tests/exec_client.rs` tests, none of which sets the path
4. `QfexExecClientConfig`'s `Debug` shows `record_path` and still hides the secret, verified by the unit tests of `src/config/exec.rs`
5. `exec_tester` cannot point at mainnet: every accepted step uses `QfexEnvironment::Uat` for the data and the exec config, and a missing or unknown step stops it before any connect, verified by the unit tests of the step mapping in `examples/exec_tester.rs`
6. `docs/STATUS.md` "The UAT gate" gives one exact command for each of the nine steps, each with the environment variables it needs named but no values, verified by reading that section
7. After slice 3, the fixtures of `tests/fixtures/ws/trade.jsonl` and `tests/fixtures/http/` carry the recorded shapes and hold no `signature`, `public_key` or `api_key` field, verified by `grep -rE '"(signature|public_key|api_key)"' tests/fixtures` finding nothing and `cargo test --test exec_client` passing
8. After slice 4, G2 of `ARCHITECTURE.md` no longer says "now 0", and the five open questions the run covers (ACK echo, `CANCELLED` after `MODIFIED`, stop order listing, refused login, error frame shapes) each carry an answer or say the run did not show it, verified by reading `ARCHITECTURE.md` and `docs/PLAN.md`

## Risks

- A secret reaches the recording through a path the test does not cover (a new request kind, an error body that echoes a header, a retry). Cost: a UAT key in a file the person may share. Mitigation: record only at the three send and receive points named in Context, redact the `auth` frame by kind rather than by searching for the value, and the test checks each value the stand-in saw
- An error answer from the venue echoes the request, and the `err` frame of the docs carries `incoming_message`. If the venue ever echoes an `auth` frame there, the signature lands in the file. Mitigation: a received frame whose `incoming_message.type` is `auth` is recorded with `params` redacted, covered by the stand-in's refused-login path
- The UAT market data host `wss://mds.qfex.io` is assumed, not documented. If it is wrong, `exec_tester` on UAT gets no quotes and steps 3 to 6 cannot place orders off the book. Cost: the person's run stops at step 3; the recording of steps 1 and 2 still holds
- The run waits on a person, so slices 3 and 4 may wait days after 1 and 2 have merged. Cost: the issue sits in box 4 Build at `WAITING`; slices 1 and 2 leave `main` no worse in the meantime
- Step 7 (drop with an open order) has no in-process switch: the trade socket registers no reconnect handler. The person drops the network by hand. Cost: the catch-up may not be seen if the drop is shorter than the heartbeat
- The venue differs from the docs in a way that needs more than a fixture swap (for example no `client_order_id` echo on ACK). Cost: slice 3 grows; if the fix is larger than one slice, it becomes its own issue rather than growing this one
- PR #8 touches the same lines of `connect()`. Cost: a rebase of slice 1, and of the count rows

## Assumptions

- `record_path` lives in `QfexExecClientConfig`, not only in the example, because `exec_tester` builds the client through `QfexExecutionClientFactory` and the config is the only thing it passes in
- Only the execution client records: the restate names REST answers and trade-socket frames; market data is already recorded and public
- The Python constructor gets no `record_path` argument, because the restate asks for `exec_tester` only and the Python binding is not part of the UAT run
- The record format is JSON lines with a direction (`rest`, `sent`, `received`), a receive time and the raw body or frame text, because slice 3 copies frames out of it into `.jsonl` and `.json` fixtures as they are
- The redacted `auth` line keeps `"type":"auth"` and drops `params`, and the nonce and timestamp are not kept, because a signature is meaningless without them and they are useless without the secret
- The recording folder `recordings/` is ignored by git; only fixtures cut from it are committed, because the raw files carry the person's user and account ids
- \* Product decision, decided by the person on 2026-11-28 ("แทน id", replace the ids): user and account ids from the UAT account are replaced in committed fixtures with the placeholder ids the hand-made fixtures already use; UAT ids are never committed as recorded
- `exec_tester` steps 1 and 2 are the connect of every run (`GET /user/positions` and the trade socket login), so they need no step of their own
- Steps 3 to 7 use `ExecTesterConfig` flags that exist in nautilus-testkit 0.67.0; if one of them does not drive the venue the way the step needs, the command in `docs/STATUS.md` says what the person does by hand instead
- No port from `nautilus-risex` or `nautilus-lighter`: neither is in this checkout, and the recording is a narrow hook on our own clients
- The test counts move by the tests slices 1 to 3 add; the rows in `docs/STATUS.md` change in the same slice, as `checks.sh` requires
