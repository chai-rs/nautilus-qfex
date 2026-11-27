WAITING: none
STOPPED: none

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-27T13:45Z | 2 | restate | no-intake | needs-info | NEEDS-INFO: a person picks the branch for the records |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.

- 2026-11-27T13:45Z · waiting · restate asked which branch holds the bloop records
- 2026-11-27T14:32Z · user · person answered "1" (merge claude/bloop-init into main, keep baseBranch main); intake ACCEPT, restate written and confirmed by the person
