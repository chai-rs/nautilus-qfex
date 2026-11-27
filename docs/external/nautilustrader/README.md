# NautilusTrader

## Owner

The NautilusTrader project (https://github.com/nautechsystems/nautilus_trader). We depend on the crates.io release 0.67.0, every crate pinned `=0.67.0` in `Cargo.toml`. The reference adapters are `nautilus-risex` (parity target) and `nautilus-lighter` 0.67.0.

## What we use

| crate | item | used for |
| --- | --- | --- |
| `nautilus-common` | `DataClient`, `ExecutionClient` traits, message bus, `SocketState`, `SocketStateChanged`, `DataActor::subscribe_socket_state` | the client contracts and socket state events |
| `nautilus-live` | `SocketControlFactory::new(client_id, venue).control(endpoint)`, `SocketControl::sink()` (a `SocketStateSink` handed to the WebSocket client), client factories, `LiveNode` | publishing socket state, building clients |
| `nautilus-network` | WebSocket client, reconnect, ping, `sink::SocketState` | the transport for both sockets |
| `nautilus-model` | instruments, data types, orders, events, reports | everything the adapter emits |
| `nautilus-cryptography` | `signing::hmac_signature` | HMAC |
| `nautilus-execution` | order and report types | execution client |
| `nautilus-testkit` (dev) | testers | integration tests |

## What we cannot rely on

- **API stability across versions**: names and signatures move between releases; nothing here is checked against any version but 0.67.0.
- **`python` feature from crates.io**: `nautilus-core` 0.67.0 needs a patched `pyo3-stub-gen` from the NautilusTrader repository (see `README.md`, "Python").
- **Socket state delivery**: `SocketControlFactory` takes the system event sender at construction; a factory built before the node's event sender exists publishes nothing.
- **Reconnect registry side effect**: `LiveNodeBuilder` builds every client factory inside a `SocketReconnectRegistry` scope (`nautilus-live` 0.67.0 `node/builder.rs:646`). A `SocketControlFactory::new` run inside that scope marks the client as reconnect-capable (`socket.rs`, `SocketReconnectRegistrar::owner`). If the client then never calls `SocketControl::register`, a controller reconnect command for it answers `UnknownEndpoint` instead of `Unsupported`. Any change that adds a `SocketControlFactory` to a client constructor has to read `node/builder.rs` too and say which outcome it leaves. Seen in `2026-11-27-market-data-socket-state`; the test that pins it belongs to `2026-11-27-data-client-reconnect-command`.

## What we do when it changes

- Upgrades are deliberate: bump every `=0.67.0` pin together, rebuild, run the full gate, and compare with how `nautilus-risex` and `nautilus-lighter` changed in the same release.
- A breaking change to a trait this crate implements gets an ADR in `docs/adr/`.
