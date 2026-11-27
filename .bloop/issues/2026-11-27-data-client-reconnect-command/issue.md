FR: FR-7
KIND: request
GOAL: none

A controller in a `LiveNode` can ask the QFEX data client to reconnect its market data socket, which today answers `UnknownEndpoint`.

## Where it comes from

- Review of `2026-11-27-market-data-socket-state`, round 1: `review/round-1/review-code.md` finding 1 and `decision.md` (ESCALATE).
- `QfexDataClient::new` builds a `SocketControl` for endpoint `market_data` inside the node's `SocketReconnectRegistry` scope (`nautilus-live` 0.67.0 `node/builder.rs`), which marks the client as reconnect-capable. The client never calls `SocketControl::register`, so a reconnect command for `market_data` resolves to `EndpointNotFound` / `UnknownEndpoint`. Before that issue it resolved to `Unsupported`.

## What is asked

- Register a reconnect handler for the market data socket so the command reconnects it, or make the client answer `Unsupported` again. Decide which in restate.
- A test that builds the data client inside a registry scope and checks the lookup for `market_data`.

## Constraints

- No authenticated request and no order to the live QFEX venue.
