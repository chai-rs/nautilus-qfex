WAITING: none
STOPPED: none

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-27T13:51Z | 2 | restate | no-intake | needs-info | NEEDS-INFO: every step sends authenticated requests or orders; an agent may not |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.

- 2026-11-27T13:51Z · waiting · restate needs answers only the person can give
- 2026-11-27T14:38Z · user · person answered "1" to both questions (they run UAT; recording option in exec_tester first) and "1" to the restate; confirmed
