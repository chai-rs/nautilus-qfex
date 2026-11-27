DECISION: ACCEPT
TYPE: ci
FR: none
RISK: medium
SURFACES: none
RESCOPED: yes

## Summary

- Accepted as asked: `./checks.sh` fails when the test count in `docs/STATUS.md` differs from what `cargo test` reports, printing both numbers
- Comes from the retro of `2026-11-27-market-data-socket-state`, where a hand-written count was wrong twice in one issue
- Two rows carry a count, because the gate runs two test commands: `cargo test --features examples` (`168 passed, ...`) and `cargo test` (`The same 168`). Each run is checked against its own row
- Medium risk: `checks.sh` is the single gate for every slice and for CI, so a parsing bug blocks all work or lets a wrong count through
- Must be built on a branch cut from `origin/main` (`362bcb0`): `docs/STATUS.md` on `claude/bloop-init` still says 162 and would fail the new step

## Why

`AGENTS.md` already requires that a change in the test count updates `docs/STATUS.md` in the same branch, but nothing enforces it, and the retro (`.bloop/issues/2026-11-27-market-data-socket-state/retro.md`) shows it slipped twice. `checks.sh` on `origin/main` already runs `cargo test` and `cargo test --features examples` offline, so the totals are at hand without a new tool or network access. `ARCHITECTURE.md` goal G3 measures the gate by test count, which makes a stale count in `docs/STATUS.md` a wrong measurement, not just a stale note. The change is one gate step plus the proof that it bites, which fits one pull request.
