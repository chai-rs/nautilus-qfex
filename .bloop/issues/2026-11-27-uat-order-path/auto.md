WAITING: NEEDS-INFO. 1) Will a person with a UAT key run the nine UAT gate steps themselves, approving each, and when? 2) How do the real frames reach the repo: (a) the person hands over raw answers and frames with the key removed, or (b) a first slice adds a frame-recording option to exec_tester that the person runs?
STOPPED: waiting

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
