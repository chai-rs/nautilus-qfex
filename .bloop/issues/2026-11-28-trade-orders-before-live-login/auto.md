WAITING: restate confirmed by the user in the session ("ถูก ยืนยัน", 2026-11-28); the signature is not written by an agent (permission check). A person sets line 1 of restate.md to CONFIRMED: yes with CONFIRMED-BY under it
STOPPED: waiting

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-28T11:00Z | 2 | restate | no-intake | no-intake | not run: bin/job names debug for this bug, restate was started by mistake and stopped without writing |
| 2 | 2026-11-28T11:40Z | 2 | debug | no-intake | no-intake (debugged) | ROOT-CAUSE in 1 round: orders via send_text replay before the next auth (20/20 forced, 50/50 unforced); a stale auth answer passes as Authenticated (20/20) |
| 3 | 2026-11-28T12:05Z | 2 | restate | no-intake | awaiting-human | ACCEPT, fix, risk high, surfaces security; confirmation waits on a person |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.
2026-11-28T12:05Z waiting: restate confirmation needs a person
