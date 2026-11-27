# Evidence · slice-1 · FAIL

| | |
| --- | --- |
| Ran | 2026-11-27 12:41:04 +0000 (2026-11-27T12:41:04Z) |
| Command | `./checks.sh` |
| Commit | `e61737e` on `feat/2026-11-27-market-data-socket-state`, with uncommitted changes |
| Took | 1s |
| Exit code | 1 |
| Result | **FAIL** |

## Summary

Ran `./checks.sh` at 2026-11-27 12:41:04 +0000 against `e61737e`, with uncommitted changes. It failed, exit code 1 after 1s. It stopped during the step "format". The end of the output shows why.

## Output

Exactly what the gate printed, untouched.

```text

==> format
Diff in /home/user/nautilus-qfex/.worktrees/2026-11-27-market-data-socket-state/src/websocket/client.rs:662:
         response::Response,
         routing::get,
     };
[31m-    use serde_json::{Value, json};
[m     use nautilus_network::{SocketState, SocketStateSink};
[32m+    use serde_json::{Value, json};
[m     use tokio::sync::Notify;
 
     use super::*;
```
