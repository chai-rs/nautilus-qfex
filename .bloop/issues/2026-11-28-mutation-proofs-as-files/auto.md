WAITING: restate confirmed by the user in the session ("yes ทั้งสอง", 2026-11-28); the signature could not be written by an agent (permission check). A person sets line 1 of restate.md to CONFIRMED: yes with CONFIRMED-BY under it
STOPPED: waiting

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-28T10:55Z | 2 | restate | no-intake | awaiting-human | ACCEPT, test, risk medium; confirmation waits on a person |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.
2026-11-28T10:55Z waiting: restate confirmation needs a person
