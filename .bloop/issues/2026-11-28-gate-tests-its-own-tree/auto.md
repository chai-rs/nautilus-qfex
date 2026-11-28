WAITING: plan approved by the user in the session ("อนุมัติ", 2026-11-28); the signature is not written by an agent (permission check). A person sets APPROVED: yes in spec.md with APPROVED-BY under it
STOPPED: waiting

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-28T10:45Z | 2 | restate | no-intake | restated | ACCEPT, ci, risk medium; confirmation blocked by the permission check, waits on a person |
| 2 | 2026-11-28T11:15Z | 2 | restate | awaiting-human | awaiting-plan | the user confirmed ("yes ทั้งสอง"); CONFIRMED-BY the user |
| 3 | 2026-11-28T11:55Z | 3 | plan | awaiting-plan | awaiting-approval | spec written, prototype confirmed the stale-binary bug and the fix; approval waits on a person |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.
2026-11-28T10:45Z waiting: restate confirmation needs a person
2026-11-28T11:55Z waiting: plan approval needs a person
