# NautilusTrader

## Owner

The NautilusTrader project (https://github.com/nautechsystems/nautilus_trader). We depend on the crates.io release 0.67.0, every crate pinned `=0.67.0` in `Cargo.toml`. The reference adapters are `nautilus-risex` (parity target) and `nautilus-lighter` 0.67.0.

## What we use

| crate | item | used for |
| --- | --- | --- |
| `nautilus-common` | `DataClient`, `ExecutionClient` traits, message bus, `SocketState`, `SocketStateChanged`, `DataActor::subscribe_socket_state` | the client contracts and socket state events |
| `nautilus-live` | `SocketControlFactory::new(client_id, venue).control(endpoint)`, `SocketControl::sink()` (a `SocketStateSink` handed to the WebSocket client), client factories, `LiveNode` | publishing socket state, building clients |
| `nautilus-live` | `SocketControl::register(fn)`; in tests `SocketReconnectRegistry::scope` and `get` | answering a controller `ReconnectSocket` command for the market data socket |
| `nautilus-network` | WebSocket client, reconnect, ping, `sink::SocketState` | the transport for both sockets |
| `nautilus-network` | `WebSocketClient::reconnect_handle()`, `WebSocketReconnectHandle::request_reconnect()`, `mode::ReconnectRequestOutcome` | the transport's own answer to a controller reconnect command |
| `nautilus-network` | `WebSocketClient::epoch_builder()`, `websocket::channel_epoch_message_handler()`, `WebSocketClient::send_text_on_connection(data, keys, epoch)`, `WebSocketClient::connection_epoch()` | binding the trade socket's `auth` and session frames to the connection they belong to (`2026-11-27-trade-late-auth-answer`) |
| `nautilus-model` | instruments, data types, orders, events, reports | everything the adapter emits |
| `nautilus-cryptography` | `signing::hmac_signature` | HMAC |
| `nautilus-execution` | order and report types | execution client |
| `nautilus-testkit` (dev) | testers | integration tests |

## What we cannot rely on

- **API stability across versions**: names and signatures move between releases; nothing here is checked against any version but 0.67.0.
- **`python` feature from crates.io**: `nautilus-core` 0.67.0 needs a patched `pyo3-stub-gen` from the NautilusTrader repository (see `README.md`, "Python").
- **Socket state delivery**: `SocketControlFactory` takes the system event sender at construction; a factory built before the node's event sender exists publishes nothing.
- **Reconnect registry side effect**: `LiveNodeBuilder` builds every client factory inside a `SocketReconnectRegistry` scope (`nautilus-live` 0.67.0 `node/builder.rs:646`). A `SocketControlFactory::new` run inside that scope marks the client as reconnect-capable (`socket.rs`, `SocketReconnectRegistrar::owner`). If the client then never calls `SocketControl::register`, a controller reconnect command for it answers `UnknownEndpoint` instead of `Unsupported`. Any change that adds a `SocketControlFactory` to a client constructor has to read `node/builder.rs` too and say which outcome it leaves. Seen in `2026-11-27-market-data-socket-state`; fixed for the data client by `2026-11-27-data-client-reconnect-command` (`f9b4079`), which registers the handler and pins it with `reconnect_command_reconnects_the_market_data_socket` in `tests/data_client.rs`. The execution client avoids it by building its `trade` control on the first `connect()`, outside the scope; `socket_state_follows_the_trade_socket` in `tests/exec_client.rs` builds the client inside a registry scope and fails if the client claims reconnect support (`ClientNotFound` for `trade` and `market_data`).
- **`send_text` is not bound to a connection**: it waits through a reconnect, and the writer keeps an ordinary send it took during a reconnect, or failed to write, in a replay buffer that it writes to the replacement connection as soon as that is active, before `RECONNECTED` reaches the frame handler (`nautilus-network` 0.67.0 `websocket/client.rs:2116-2120`, `:2233-2237`, `:3511`, `:3877`). A frame that must reach one connection only goes out with `send_text_on_connection` and the epoch that `epoch_builder` attaches to each frame and to `RECONNECTED`: the first connection is epoch 0, each replacement one more, and the writer answers `SendError::ConnectionChanged` instead of writing it elsewhere (`:2125-2128`, `:2177-2184`, `:3539`). It still waits for an active connection first (`wait_for_active`, `:3389`), up to the connect timeout. Seen in `2026-11-27-trade-late-auth-answer` (`debug.md`, loops A and B).
- **Reconnect registration follows the sink**: `SocketControl::register` does nothing unless it is called on the same `SocketControl` instance after its latest `sink()` (a clone starts at generation 0), and every new `sink()` drops the previous registration, so the lookup answers `EndpointNotFound` until `register` is called again (`nautilus-live` 0.67.0 `socket.rs:547-598`, checked by a prototype on 27 Nov 2026).

## What we do when it changes

- Upgrades are deliberate: bump every `=0.67.0` pin together, rebuild, run the full gate, and compare with how `nautilus-risex` and `nautilus-lighter` changed in the same release.
- A breaking change to a trait this crate implements gets an ADR in `docs/adr/`.
