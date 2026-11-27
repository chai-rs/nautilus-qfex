DECISION: NEEDS-INFO
TYPE: test
FR: FR-4
RISK: high
SURFACES: security
RESCOPED: no

## Summary

- The work is worth doing: authentication and execution are UNVERIFIED on the venue (`docs/STATUS.md`), and G2 in `ARCHITECTURE.md` sits at 0 of 4 "when a UAT key is available"
- Every one of the nine steps of "The UAT gate" sends an authenticated request or an order to QFEX UAT, which `AGENTS.md` forbids an agent from doing; the issue itself says a person has to hold the key and approve each step
- No agent in the loop (plan, build, review) can run the part that carries the value, so taking the issue now would build nothing an agent can prove
- Nothing in the repository records the frames of a run: `examples/exec_tester.rs` and `bin/flatten.rs` read the UAT key from the environment but write no frames to a file, so how the real frames come back is still open
- Blocked on a person committing to the run and on how its frames reach the repository; once answered, the agent part is fixtures, open questions, `docs/STATUS.md` and G2

## Why

`AGENTS.md` says "Never send an authenticated request or an order to the live QFEX venue, from a test, an example, a binary or a script", and `issue.md` agrees that a person must provide `QFEX_UAT_API_KEY` / `QFEX_UAT_API_SECRET` and approve every step that leaves the machine. The code cannot answer whether such a person is available or when: G2 in `ARCHITECTURE.md` (as on `origin/main`, `362bcb0`) has no date beyond "when a UAT key is available", and `docs/STATUS.md` records that no key was ever used. The agent's share also depends on how the frames come back: `examples/exec_tester.rs` connects to UAT with the key from the environment but has no recording option, so either the person captures the raw REST answers and trade-socket frames by hand, or a first slice adds a recording option for the person to run. Risk is high because the run touches auth, real orders and a key that must never land in git, a log line or a fixture; SURFACES is `security` for the same reason.

## Questions

1. Will a person with a UAT key run the nine steps of "The UAT gate" in `docs/STATUS.md` themselves, approving each one, and if so, when?
2. How should the real frames reach the repository: (a) that person saves the raw REST answers and trade-socket frames, key removed, and hands them over, and the agent then replaces fixtures and updates `docs/STATUS.md`, `docs/PLAN.md` and G2; or (b) a first slice adds an option to `exec_tester` that writes every frame it sends and receives to a file, with the key never written, for the person to run?
