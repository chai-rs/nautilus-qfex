CONFIRMED: yes
CONFIRMED-BY: trustLevel autonomous

# The trade socket's control says it takes no reconnect commands

## What we will do

- Someone reading `QfexExecutionClient` learns from names and types alone, with no comment, three things. The trade socket's control only publishes socket state. It takes no reconnect commands. It is created when the client first connects, not when the client is built. That last point is why a node answers `Unsupported`
- Under a `LiveNode`, a controller's reconnect command for the QFEX execution client still answers `Unsupported`. It must not become `EndpointNotFound`, which is what a control created during construction without a handler would give
- The trade socket keeps publishing `SocketStateChanged` (venue `QFEX`, endpoint `trade`) as it does today. That covers connect, a drop, the reconnect with a second login, a deliberate disconnect that reports nothing, and a later connect that reuses the same control
- `socket_state_follows_the_trade_socket` passes unchanged in what it asserts. A client built inside a registry scope still claims no reconnect support: `ClientNotFound` for `trade` and `market_data`
- No comment comes back to explain the control, so a `clean-comment` pass over the diff has nothing about it to delete
- `./checks.sh` passes. The socket state row of `docs/STATUS.md`, step 7 of the data flow in `ARCHITECTURE.md` and the reconnect registry bullet in `docs/external/nautilustrader/README.md` stay true. Any of them that cites a name this changes is updated in the same branch, and so is the test count in `docs/STATUS.md` and `README.md` if it moves

## What we will not do

- A reconnect handler for the trade socket, which would force a re-login and catch-up on command. It changes behaviour on the order socket, in the login path that unmerged PR #16 is still changing while its review waits on a person. PR #16 also records that orders sent during a reconnect can still be replayed before the next `auth`, and a command would make that reachable on demand. That needs its own issue once PR #16 and the order-path issue are settled
- Any change to `src/websocket/client.rs` or to the trade login. That belongs to PR #16
- Any change to the data client or its `market_data` reconnect command
- Any contact with the live QFEX venue. Tests use the stand-in on `127.0.0.1`

## Contracts affected

none
