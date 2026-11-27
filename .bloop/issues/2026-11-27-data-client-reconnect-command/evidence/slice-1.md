# Evidence · slice-1 · FAIL

| | |
| --- | --- |
| Ran | 2026-11-27 14:50:22 +0000 (2026-11-27T14:50:22Z) |
| Command | `./checks.sh` |
| Commit | `362bcb0` on `feat/2026-11-27-data-client-reconnect-command`, with uncommitted changes |
| Took | 11s |
| Exit code | 101 |
| Result | **FAIL** |

## Summary

Ran `./checks.sh` at 2026-11-27 14:50:22 +0000 against `362bcb0`, with uncommitted changes. It failed, exit code 101 after 11s. It stopped during the step "lint". The end of the output shows why.

## Output

Exactly what the gate printed, untouched.

```text

==> format

==> lint
    Checking nautilus-qfex v0.1.0 (/home/user/nautilus-qfex/.worktrees/2026-11-27-data-client-reconnect-command)
error: this function has a `#[must_use]` attribute with no message, but returns a type already considered as `#[must_use]`
   --> src/websocket/client.rs:396:5
    |
396 | /     pub fn reconnect_request(
397 | |         &self,
398 | |     ) -> impl Fn() -> ReconnectRequestOutcome + Send + Sync + 'static {
    | |_____________________________________________________________________^
    |
note: the return type is implementer of `std::ops::Fn`
   --> src/websocket/client.rs:398:10
    |
398 |     ) -> impl Fn() -> ReconnectRequestOutcome + Send + Sync + 'static {
    |          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
help: remove `must_use`
   --> src/websocket/client.rs:395:5
    |
395 |     #[must_use]
    |     ^^^^^^^^^^^
    = note: alternatively, you may add an explicit reason to the `must_use` attribute
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.99.0/index.html#double_must_use
    = note: `-D clippy::double-must-use` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(clippy::double_must_use)]`

error: could not compile `nautilus-qfex` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `nautilus-qfex` (lib test) due to 1 previous error
```
