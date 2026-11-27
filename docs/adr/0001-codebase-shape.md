# ADR-0001: Keep the NautilusTrader adapter layout instead of DDD + hexagonal

## Status

Accepted on 2026-11-27, by the repository owner during the bloop init interview.

## Context

- The crate is a NautilusTrader 0.67.0 adapter for QFEX, about a dozen modules under `src/`, with 162 passing tests.
- Its ports are already defined outside the crate: NautilusTrader's `DataClient`, `ExecutionClient` and factory traits. Its domain types come from `nautilus-model`.
- The crate tracks parity with `nautilus-risex` and `nautilus-lighter`, which use the same module layout (`common`, `http`, `websocket`, `data`, `execution`, `config`, `factories`, `python`).
- bloop's default codebase shape is DDD + hexagonal (`domain/`, `application/`, `port/`, `adapter/`).

## Decision

We keep the existing Nautilus adapter layout. `common`, `signing` and `book` stay free of I/O. `data` and `execution` reach the venue only through `http` and `websocket`. `config`, `factories` and `python` only wire the clients together. `ARCHITECTURE.md` records the allowed import direction.

## Consequences

- No file moves; existing tests, benches, fuzz targets and the `pub` API that mocha uses stay as they are.
- A change in `nautilus-risex` or `nautilus-lighter` can be ported here by matching module names.
- The boundary rules are checked by review only; there is no Rust equivalent of `dependency-cruiser` wired in, so a violation can slip through.
- Pure logic and I/O live side by side inside `websocket` and `http`, so some unit tests need a mock server.

## What we did not choose

- DDD + hexagonal (`domain/`, `application/`, `port/`, `adapter/`) — it would move every file, duplicate ports Nautilus already defines, and break line-by-line parity with the other Nautilus adapters.
