REVIEWER: code
MODEL: default
APPROVED: yes

# The trade socket's control says it takes no reconnect commands

## Summary

- A new private module of `execution` holds `StateOnlyTradeSocketControl`: it keeps the trade socket's `Option<SocketControl>` in a private field and offers only a constructor named `built_on_first_connect()` and a `sink(client_id)` that builds the control on the first call and hands out a `SocketStateSink`. Nothing outside that module can reach the control, so nothing in the execution client can call `register` on it
- `QfexExecutionClient` keeps `trade_socket_control: StateOnlyTradeSocketControl` instead of `socket: Option<SocketControl>`, builds it in `new` with `StateOnlyTradeSocketControl::built_on_first_connect()` and takes the sink in `connect()` with `self.trade_socket_control.sink(client_id)`. The names say the three facts, with no comment
- Behaviour stays as it is: the control is still built on the first `connect()`, outside the node's registry scope, so a node answers `Unsupported`; `SocketStateChanged` for `trade` is unchanged
- Proven by `socket_state_follows_the_trade_socket`, left as it is: the prototype showed it fails as soon as the control is built in `new` inside a registry scope, so it pins the timing the new names describe
- One slice, no new test, no doc and no count row changes

## Context

Line numbers are those of origin/main at `92dca56`, which the work branch is cut from. `src/` and `tests/` there are the same as in this checkout.

| path | what it does today | how this work relates |
| --- | --- | --- |
| `src/execution/mod.rs` | `QfexExecutionClient` (struct 497-505) keeps `socket: Option<SocketControl>` (504), set to `None` in `new` (513-530, field at 528); `connect()` fills it with `get_or_insert_with(SocketControlFactory::new(client_id, Some(*QFEX_VENUE)).control(TRADE_ENDPOINT))` (760-762) and passes `socket.sink()` to `QfexWebSocketClient::new_trade(..).with_state_sink(..)` (763-764); `TRADE_ENDPOINT` (76) is used only there; imports `SocketControl`, `SocketControlFactory` (32) and `consts::QFEX_VENUE` (57) only for it | must change: declare a private `mod trade_socket;` next to `pub mod dispatch;` and `pub mod order;` (6-7); field `trade_socket_control: StateOnlyTradeSocketControl`, built with `StateOnlyTradeSocketControl::built_on_first_connect()` in `new`; `connect()` calls `.with_state_sink(self.trade_socket_control.sink(client_id))`; the `TRADE_ENDPOINT` constant and the imports `SocketControl`, `SocketControlFactory`, `QFEX_VENUE` move out. Nothing else in the file changes, the login path included |
| new file src/execution/trade_socket.rs (does not exist yet) | none | must be created: `pub(super) struct StateOnlyTradeSocketControl { control: Option<SocketControl> }` with `#[derive(Debug)]` (the client derives `Debug`), `pub(super) const fn built_on_first_connect() -> Self`, and `pub(super) fn sink(&mut self, client_id: ClientId) -> SocketStateSink` holding the `get_or_insert_with` moved from `connect()`; holds `TRADE_ENDPOINT`. No `register`, no `deregister`, no comment |
| `src/execution/dispatch.rs` | a submodule of `execution` | an example of the shape to follow for a file under `src/execution/` |
| `src/data/mod.rs` | `QfexDataClient::new` builds its `market_data` control in the constructor, inside the registry scope (318), and registers a reconnect request with each sink (`market_data_socket`, 283) | no change; the opposite case, read only. Restate keeps the data client out of scope |
| `src/factories/exec.rs` | `create` calls `QfexExecutionClient::new(core, config)` (62), which `LiveNodeBuilder` runs inside the node's registry scope | no change: the signature of `new` stays |
| `tests/exec_client.rs` | `Harness::start_in` (397) builds the client inside `registry.scope(..)` (425-426); `socket_state_follows_the_trade_socket` (1004-1037) checks connect, drop, reconnect with a second login, a deliberate disconnect that reports nothing, a later connect, then `ClientNotFound` for `trade` and `market_data` (1030-1035); `next_trade_state` (991) | reuse, do not rewrite: this test is the check. The file stays untouched, because unmerged PR #16 deletes the comment at 1012-1013 |
| `src/websocket/client.rs` | `QfexWebSocketClient::new_trade`, `with_state_sink` (388) | no change. Unmerged PR #16 rewrites this file; this work stays out of it |
| `docs/STATUS.md` | test count rows (31-32: 171 passed); socket state row (79) says the trade socket registers no handler because its control is built on the first `connect()`, outside the node's registry scope, so the command answers `Unsupported` | no change: the row names no field or type this work renames and stays true; the count does not move |
| `ARCHITECTURE.md` | Dataflow step 7 (52): the trade socket publishes through one `SocketControl` that `execution::QfexExecutionClient` builds on its first `connect()`; a reconnect command for it answers `Unsupported` | no change: still true, names nothing renamed |
| `docs/external/nautilustrader/README.md` | "Reconnect registry side effect" (27) says the execution client avoids `UnknownEndpoint` by building its `trade` control on the first `connect()`, and names `socket_state_follows_the_trade_socket`; the "What we use" rows (12, 14) list `SocketControlFactory::new(..).control(..)`, `SocketControl::sink()`, `SocketStateSink` | no change: still true, and every item the new module uses is already listed |
| nautilus-live 0.67.0 src/socket.rs | `SocketControlFactory::new` (396-399) takes the registrar of the current scope and, through `SocketReconnectRegistrar::owner` (191-198), marks the client as supported; it also takes the system event sender at construction (`from_registrar`, 438-449); `SocketReconnectRegistry::get` (137-156) answers `EndpointNotFound` for a supported client with no registered endpoint | why the timing matters; read only |
| nautilus-live 0.67.0 src/node/builder.rs | builds every execution client factory inside `socket_registry.scope(..)` (701) and then calls `register_client` (717) | why a node answers `Unsupported` rather than `ClientNotFound`; read only |

Depends on one outside system: NautilusTrader 0.67.0, recorded in `docs/external/nautilustrader/README.md`, unchanged. No contact with QFEX at all.

**This change has to reach** callers: `QfexExecutionClient::new` and `ExecutionClient::connect` in `src/execution/mod.rs` only; `src/factories/exec.rs` keeps calling `new` unchanged · tests: none changed, `socket_state_follows_the_trade_socket` is the check · fixtures: none · config: none · exports: none, the new module and type are private to `execution`

## Prototype

- Question to answer: does `socket_state_follows_the_trade_socket` fail if the trade control is built in `QfexExecutionClient::new`, inside the test's registry scope, instead of on the first `connect()`? If not, a refactor of that timing has no check
- What was tried: in a throwaway worktree of origin/main at `92dca56` outside the repo, the `StateOnlyTradeSocketControl` sketch of `## Context` (clippy clean with and without `examples`, all 171 tests pass, the test passes), then a mutant that calls `sink(client_id)` once in `new`
- Result: the mutant fails the test at the registry assertion (`tests/exec_client.rs:1031`, `ClientNotFound` expected) after every socket state assertion before it passed, and `exec_client` reports 12 passed, 1 failed. The test pins the timing, so the plan needs no new test
- Code thrown away, never merged

## Slices

| # | slice | check that proves it | status |
| --- | --- | --- | --- |
| 1 | Add the private module `trade_socket` of `execution` with `StateOnlyTradeSocketControl` (`built_on_first_connect()`, `sink(client_id)`, private `control: Option<SocketControl>`, `TRADE_ENDPOINT`); `QfexExecutionClient` replaces `socket: Option<SocketControl>` with `trade_socket_control: StateOnlyTradeSocketControl` and takes its sink in `connect()`. No comment, no test, no doc change | `cargo test --test exec_client socket_state_follows_the_trade_socket` passes; `grep -nw -e SocketControl -e SocketControlFactory -e register src/execution/mod.rs` prints nothing; `git diff origin/main -- tests docs README.md ARCHITECTURE.md` is empty; `cargo test` reports the count `docs/STATUS.md` states (171 at `92dca56`), 0 failed | todo |

## Acceptance criteria

1. `QfexExecutionClient` keeps the trade socket's control as `trade_socket_control: StateOnlyTradeSocketControl` and builds it with `StateOnlyTradeSocketControl::built_on_first_connect()` in `new`, verified by `grep -n "trade_socket_control" src/execution/mod.rs` showing the field, the constructor call in `new` and the `sink(client_id)` call in `connect()`
2. The execution client cannot reach the `SocketControl` itself: `src/execution/mod.rs` names neither `SocketControl` nor `SocketControlFactory` nor `register` as a word, verified by `grep -nw -e SocketControl -e SocketControlFactory -e register src/execution/mod.rs` printing nothing, and the control field of `StateOnlyTradeSocketControl` is private to its module, verified by the struct in the trade socket module having no `pub` on that field
3. The trade socket module registers no reconnect handler and has no comment, verified by `grep -n -e register -e // src/execution/trade_socket.rs` printing nothing
4. The diff adds no comment to `src/execution/mod.rs`, verified by `git diff origin/main -- src/execution/mod.rs | grep -E '^\+.*//'` printing nothing
5. Under a registry scope the client still claims no reconnect support and the trade socket still publishes `Connected`, `Disconnected`, `Connected` around a drop, nothing on a deliberate disconnect and `Connected` on a later connect, verified by `cargo test --test exec_client socket_state_follows_the_trade_socket` passing with `tests/exec_client.rs` unchanged (`git diff origin/main -- tests/exec_client.rs` empty)
6. No test, count row or doc changes: `git diff origin/main --stat -- tests docs README.md ARCHITECTURE.md` prints nothing, and `cargo test` and `cargo test --features examples` each report the number the two count rows of `docs/STATUS.md` state, 0 failed
7. `src/websocket/client.rs` is untouched, verified by `git diff origin/main -- src/websocket/client.rs` printing nothing

## Risks

- Building the control anywhere `new` runs, for example by moving `sink` or a `SocketControlFactory::new` into the constructor while tidying, turns the answer into `EndpointNotFound` under a `LiveNode` and also changes which system event sender the control holds; `socket_state_follows_the_trade_socket` fails on it, as the prototype showed
- Rebuilding the control on every `connect()` (dropping `get_or_insert_with`) would hand each trade socket a new control; socket state would still publish, but `ARCHITECTURE.md` step 7 says one control is kept for every later trade socket; review has to read the `sink` body, no test separates the two
- PR #16 and this work meet in two files only by line context: it deletes `tests/exec_client.rs` 1012-1013 and rewrites `docs/STATUS.md` lines 3-5, 31-32 and 76; this work touches neither file, so whichever merges first, the other rebases without a conflict
- A target directory shared with the worktree of PR #16 can run that branch's test binaries: on 28 Nov 2026 `./checks.sh` in the main checkout counted 173 (145 unit with `examples`) against rows of 171 until a source file was touched. Build must touch a source file, or use its own target directory, before it trusts a count
- Test count rows: this work adds and removes no test, so it writes no count. If it merges first, PR #16's 173 stays right on top of it; if PR #16 merges first, the rebased branch counts 173 and the rows already say 173. `./checks.sh` compares the rows with the run either way

## Assumptions

- One new private file under `src/execution/` rather than a type inline in `mod.rs`, because only a module boundary makes the control unreachable for the rest of the execution client, which is what lets the type say "takes no reconnect commands" instead of a comment
- The names `StateOnlyTradeSocketControl`, `built_on_first_connect()`, `sink(client_id)` and `trade_socket_control`, because they carry the three facts restate asks for at the two places a reader looks: the field and `new`; build may adjust a name if clippy or review asks, as long as the three facts stay readable from names alone
- `SocketStateSink` is imported from `nautilus_network` in the new module, as `src/websocket/client.rs` does; an outside crate is not a layer of the dependency direction in `AGENTS.md`
- No new test, because the existing test already fails on the one regression that matters (prototype) and restate asks for it to pass unchanged in what it asserts; a test of `nautilus-live` behaviour alone would test their code
- `docs/STATUS.md`, `ARCHITECTURE.md` and `docs/external/nautilustrader/README.md` stay as they are, because none cites `socket` or another name this renames and the behaviour they describe does not change; restate asks for an update only where a cited name changes
- `nautilus-lighter` and `nautilus-risex` sources are not on this machine, so no file was followed from them; the shape follows the `nautilus-live` 0.67.0 API read above
