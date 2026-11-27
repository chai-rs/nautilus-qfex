STATUS: READY

## Round 1

| slice | commit | check | result | evidence |
| --- | --- | --- | --- | --- |
| 1 | `1de6e4c` | `cargo test --lib mds_state_sink_sees_connect_loss_and_reconnect`, then `./checks.sh` | red first (`no method named with_state_sink`), then green 10 of 10 runs; gate failed once on `cargo fmt` (import order), passed on rerun: 167 passed, 0 failed per `cargo test` run | `evidence/slice-1.md` (FAIL, format) · `evidence/slice-1-rerun.md` (PASS) |
| 2 | `1c17205` | `cargo test --test data_client socket_state`, then `./checks.sh` | red first (`no socket state within 10 s`), then green 10 of 10 runs; gate: 168 passed, 0 failed per `cargo test` run (142 unit, 8, 2, 4 data client, 12 exec client) | `evidence/slice-2.md` (PASS) |
| all | `1c17205` | `./checks.sh --e2e` on the committed branch | exit 0, 14 `test result: ok` lines, nothing uncommitted | `evidence/final-e2e.md` (PASS) |

## Acceptance criteria

- [x] After `connect()`, `Connected` with the data client's `client_id`, venue `QFEX`, endpoint `market_data` — evidence `evidence/slice-2.md`, test `socket_state_follows_the_market_data_socket` (`next_state` asserts all three fields on every event)
- [x] When the stand-in drops the socket, `Disconnected` then `Connected` follow — evidence `evidence/slice-2.md`, same test
- [x] After `disconnect()` and a second `connect()`, `Connected` is sent again — evidence `evidence/slice-2.md`, same test. Mutation run: with the sink removed from `Feed::close`, the test fails with `no socket state within 10 s` (run by hand, not kept as a file)
- [x] A deliberate `disconnect()` sends no `Disconnected` within 500 ms — evidence `evidence/slice-2.md`, same test; also the unit test of slice 1 (200 ms)
- [x] The trade socket publishes nothing new — `QfexWebSocketClient::new_trade` sets `state_sink: None` and `QfexExecutionClient` is untouched (`git diff 4f4ed16 -- src/execution` is empty)
- [x] The 166 tests that passed at `4f4ed16` still pass — evidence `evidence/slice-2.md`: 168 passed, 0 failed. The spec said 167; slice 1 added a unit test too, so 166 + 2 = 168
- [x] No test opens a connection outside `127.0.0.1` — both new tests use the existing stand-ins (`MockVenue::start`, `start_venue`), which bind `127.0.0.1:0`

## Notes for reviewers

- Worth a closer look: `Feed.socket` is an `Arc<SocketControl>` on purpose. `SocketControl::clone` starts a control without the active generation, and `Feed` is cloned into every task, so one shared control is what lets the sink taken in `close()` retire the previous one
- Worth a closer look: `SocketControlFactory::new` reads the thread-local system event sender in `QfexDataClient::new`. A client built on a thread without that sender publishes nothing and does not fail, the same as upstream adapters
- Decision: `with_state_sink(sink)` instead of a new `new_mds` parameter, so `bin/trades_probe.rs` and five existing unit tests need no change (recorded in `spec.md`)
- Decision: the branch is cut from `claude/bloop-init`, not `origin/main`, because `checks.sh` and the issue tooling exist only there. Merging this branch into `main` also lands the bloop setup commits
- `clean-comment` ran inline, not through a separate `comment-cleanser` agent: the diff adds four short comments (two doc comments on new items, two "why" lines in the test). Say so if you want the separate pass
- `MUST KILL` flags left unresolved: none. `shortcut:` markers added: none
