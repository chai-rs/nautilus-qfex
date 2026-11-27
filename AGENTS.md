# AGENTS.md

<!-- bloop:managed agents -->
Rules an agent reads before touching this repo. People can read them too.

## Read these first

1. `ARCHITECTURE.md` business, scope, modules, the shape of the codebase
2. `docs/data-model.md` the entities and the invariants tests check
3. `docs/external/qfex/README.md` and `docs/external/nautilustrader/README.md` what we rely on from others
4. `docs/STATUS.md` what has and has not been checked against the live venue
5. `.bloop/config.json` the tools this project uses

## Every time

- Run `./checks.sh` before calling anything done. Never report a pass you did not run
- Work one slice at a time. A slice is something that can merge into main on its own
- Keep `README.md` and `docs/STATUS.md` true: a change in behaviour or in the test count updates them in the same branch
- Port from `nautilus-risex` or `nautilus-lighter` 0.67.0 when they already solve the problem, and say which file you followed
- Commit bloop records (`.bloop/issues/<id>/`) on the issue's own branch, which `bin/work` cuts from `origin/main`, so they land on `main` with the code in the same pull request. Never keep them on a long-lived side branch

## Never

- **Never send an authenticated request or an order to the live QFEX venue**, from a test, an example, a binary or a script. Public, keyless market data is the only venue contact allowed, and only when a task asks for it
- Never put a key or secret in git, a log line, a `Debug` output or a test fixture. Keys come from environment variables at run time
- Never change an accepted ADR in `docs/adr/`. Write a new one that supersedes it
- Never bump a `=0.67.0` NautilusTrader pin alone: all of them move together, with an ADR
- Never use `--no-verify`, `#[ignore]` or a skipped test to get green

## Dependency direction

```
config / factories / python  →  data / execution  →  http / websocket  →  signing / book  →  common
```

`common`, `signing` and `book` do no I/O. `data` and `execution` reach the venue only through `http` and `websocket`. Review enforces this; no tool does yet.

## When you are stuck

If requirements contradict each other, or the docs of QFEX do not cover the case in front of you, stop and ask. Record the open question in `docs/PLAN.md`. Do not guess and keep writing.
<!-- bloop:end agents -->
