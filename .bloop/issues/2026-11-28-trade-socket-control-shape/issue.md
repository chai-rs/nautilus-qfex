FR: none
KIND: request
GOAL: none

The code of `QfexExecutionClient` says by itself why a reconnect command for the trade socket answers `Unsupported`, without a comment.

## Where it comes from

- `clean-comment` over the merged diffs of PR #3, #5, #6, #8 and #11 (branch `chore/2026-11-28-clean-comment-merged`). `comment-cleanser` deleted the doc comment on `QfexExecutionClient::socket` and two test comments in `socket_state_follows_the_trade_socket`; all three existed only to explain that the control is built lazily in `connect()` (`get_or_insert_with`), outside the node's `SocketReconnectRegistry` scope, so no reconnect handler exists and a `LiveNode` answers `Unsupported`.
- `MUST KILL` flag: `src/execution/mod.rs` `QfexExecutionClient::socket` → restructure.

## What is asked

- Reshape so the fact is in the code, for example: build the control in `QfexExecutionClient::new` as `QfexDataClient::new` does and register a reconnect handler for the trade socket (which forces a re-login and catch-up on command), or give the field a type whose name says it carries no reconnect handler. Decide in restate.
- `docs/STATUS.md` row "Socket state" already records the `Unsupported` answer; keep it true.

## Constraints

- No contact with the live QFEX venue; tests use the stand-in on `127.0.0.1`.
