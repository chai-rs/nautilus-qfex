VERDICT: PASS

## Findings

Severity 0 to 4. 3 and 4 are [must], 1 and 2 are [should], 0 is not written down.

1. [should] (severity 1) src/data/mod.rs:309-310. `SocketControlFactory::new` runs inside the `socket_registry.scope(..)` that `LiveNodeBuilder` wraps around every data client factory (`nautilus-live-0.67.0/src/node/builder.rs:646`). Under that scope the factory creates a reconnect owner, and creating the owner marks `QFEX-DATA` as "supported" in the registry (`nautilus-live-0.67.0/src/socket.rs`, `SocketReconnectRegistrar::owner`: `inner.supported.insert(client_id)`). The data client never calls `SocketControl::register`, so the registry now says this client has controller-reconnectable sockets but none of its endpoints can be found. A controller's reconnect command for `QFEX-DATA` used to resolve to `Unsupported`. It now resolves to `UnknownEndpoint`, even for endpoint `market_data`, which is the endpoint the same client now publishes state for (`node/mod.rs:755-757`). No reconnect is lost, because none was possible before either. The cost is a wrong outcome reported to whoever sends the command. The spec and `build.md` do not mention this side effect.
   Proof: a scratch binary (`regprobe`, outside the repo) calls `registry.scope(|| QfexDataClient::new(ClientId::from("QFEX-DATA"), default config))` and then `registry.get(id, "market_data")`. At `1c17205` it prints `lookup market_data = EndpointNotFound`. Against the main checkout (no `SocketControl` in `src/data/mod.rs`) it prints `lookup market_data = ClientNotFound`. The node's `register_client` then turns `ClientNotFound` into `Unsupported`.
   Fix options: register the market data socket's reconnect (out of this issue's scope), or record in `docs/STATUS.md` / `docs/PLAN.md` that a reconnect command for the data client reports `UnknownEndpoint`.

## What I did not cover

- Load assumed: one node, one QFEX data client, one market data socket. State events happen only on connect, loss and reconnect, so I did not look for scale issues. Nothing in README or deploy config suggests more.
- I did not rerun the suite. I took the gate results from `evidence/slice-1-rerun.md`, `evidence/slice-2.md` and `evidence/final-e2e.md` (168 passed). The only thing I compiled and ran was the scratch probe for finding 1.
- I did not repeat the mutation run reported in `build.md` (sink removed from `Feed::close` makes the test fail). I checked by reading instead. `Feed.socket` is one `Arc<SocketControl>` shared by every `Feed` clone. The generation counter `active_generation` sits behind an `Arc` in the publisher, so the sink taken in `close()` retires the previous one. `close()` calls `ws.disconnect()` before it takes the new sink, and a deliberate disconnect is silent upstream. So the old transport cannot publish a late `Disconnected`.
- I looked at whether a failure in `start_session` could leave a `Connected` with no matching `Disconnected`. `QfexWebSocketClient::connect` disconnects deliberately on that path. For the market data socket it cannot happen in practice: `close()` always rebuilds the client with no recorded subscriptions, so `start_session` sends nothing. I did not write a finding.
- I checked that a node builds the data client after `bind_senders_for_node` (`builder.rs:608` before `:646`), so `try_get_system_event_sender` finds a sender in `QfexDataClient::new`. I did not check the Python factory path end to end.
- The trade socket and `QfexExecutionClient` are unchanged by this diff (`new_trade` sets `state_sink: None`). I did not review them.
- Security belongs to `reviewer-security`. `companions.review` in `.bloop/config.json` has no rows, so no companion skill applied.
