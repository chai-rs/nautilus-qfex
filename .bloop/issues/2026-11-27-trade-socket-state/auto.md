WAITING: none
STOPPED: none

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-27T13:50Z | 2 | restate | no-intake | awaiting-human | ACCEPT; restate written, CONFIRMED: no |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.

- 2026-11-27T13:50Z · waiting · restate needs a person's yes; it settles the Connected-means-what question the issue left open
- 2026-11-27T14:26Z · user · person answered "1" (yes, Connected means transport up); restate confirmed
