WAITING: none
STOPPED: none

## Runs

One row per skill run, oldest first. `before` and `after` come from `bin/job`, never from what the skill reported.

| # | when | box | skill | before | after | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-27T12:30Z | 3 | plan | awaiting-approval | bad-context | trustLevel autonomous approved the plan; bin/job then flagged crate paths in Context |
| 2 | 2026-11-27T12:32Z | 3 | plan | bad-context | planned | Context paths rewritten so every repo path exists |
| 3 | 2026-11-27T12:46Z | 4 | build | planned | built | 2 slices green, 168 tests pass, build.md READY |

## Answered by trust level

Every question `loop.trustLevel` answered instead of a person, so nothing it decided is invisible. Empty at `supervised`.

| when | level | skill | question | answer |
| --- | --- | --- | --- | --- |
| 2026-11-27T12:30Z | autonomous | plan | Approve this plan? | yes |
| 2026-11-27T12:47Z | autonomous | build | Push the branch and open a draft PR? | yes |

## Stops

One line each time a run ended, newest last. The reason is one of `done` `waiting` `broken` `stall` `budget` `user`.
