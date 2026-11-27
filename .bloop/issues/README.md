# .bloop/issues

Where this project keeps its work. No external API, no token.

## Folder shape

```
.bloop/issues/
  2026-11-23-add-order-cancel/
    intake.md      do we take this, how risky. written every time
    restate.md     what we agreed to build. written only on ACCEPT
    spec.md        the plan from box 3 Planning
    build.md       the record from box 4 Build
    evidence/      real run output for each slice, plus screenshots on a ui surface
    review/        one folder per review round
    decision.md    the record from box 5 Review
```

The first line of each file is machine-readable state. `intake.md` starts with `DECISION:` · `restate.md` with `CONFIRMED:`, which is a human signature an agent may never set to `yes` itself · `spec.md` with `REVIEWER:` `MODEL:` `APPROVED:` · `build.md` with `STATUS:` · `review-*.md` with `VERDICT:` · `decision.md` with `APPROVE` or `ESCALATE`, then `APPROVED-BY:`, which only a human may fill in.

## Rules

* **The id is `<YYYY-MM-DD>-<slug>`**, usable as a branch name directly, with no counter for parallel sessions to fight over
* **No central index file.** To see what work exists, read the folders. A central index is where parallel sessions overwrite each other
* **An issue's state is the stage of the SDLC it is in**, read from which files exist. There is no separate status field

| Files present | Stage |
| --- | --- |
| nothing | box 2 Issue/Story/Task, not yet triaged |
| `intake.md` saying `NEEDS-INFO` | box 2 Issue/Story/Task, waiting on answers |
| `intake.md` saying `REJECT` | not taken, done |
| `intake.md` saying `ACCEPT` | box 2 Issue/Story/Task, waiting on `restate.md` |
| `restate.md` with `CONFIRMED: no` | box 2 Issue/Story/Task, waiting on a human |
| `restate.md` with `CONFIRMED: yes` | box 2 Issue/Story/Task done, box 3 Planning can start |
| `CONFIRMED: yes` but no `spec.md` | box 3 Planning, waiting for a plan |
| `spec.md` with `APPROVED: no` | box 3 Planning, waiting on approval |
| `spec.md` with `APPROVED: yes` | box 3 Planning done, box 4 Build can start |
| `build.md` with `STATUS: IN-PROGRESS` | box 4 Build, code in progress |
| `build.md` with `STATUS: BLOCKED` | box 4 Build, stuck, a human has to decide |
| `build.md` with `STATUS: READY` | box 4 Build done, box 5 Review can start |
| `review/round-N/` holding a `CHANGES` verdict | box 5 Review sent it back for another round |
| `review/round-N/` all `PASS`, no `decision.md` | box 5 Review, waiting on the approver |
| `decision.md` starting `ESCALATE` | box 5 Review, waiting on a human |
| `decision.md` starting `APPROVE` | box 5 Review done, box 6 Deliver can start |

Run the plugin's `bin/job` to read this table for you instead of walking the folders yourself. Its `skill` field names the skill that moves each issue on, which is what `/bloop:auto` runs.

Two more files can sit in an issue folder:

* `issue.md` carries `KIND: bug` or `KIND: request` on its second line. A `bug` with no finished `debug.md` goes to `/bloop:debug` before `restate`
* `auto.md` is written by `/bloop:auto`. Its first line, `WAITING:`, holds a question a run left for a person. While it is not `none`, the issue sits in `0-waiting` on the board and `--next` skips it

## Where issue files live

Only in the main checkout. Code is written in a worktree under `.worktrees/<id>/`, but every file of the issue folder is written here, so `bin/job`, the board and `/bloop:auto` see each change the moment it happens. `bin/job --dir <id>` prints the folder's absolute path from anywhere, a worktree included.

## The board, and the archive

Issue folders never move while the work is open. A folder moved on two branches does not merge, and the issue ends up in both places.

To see the work grouped by state, run `bin/job --board`. It builds `.bloop/board/`, one folder per state holding symlinks back to `.bloop/issues/`:

```
.bloop/board/          gitignored, rebuilt from the files every time
  0-waiting/           waiting on a person: an answer, a signature, an approval
  1-broken/            a file's header line is unreadable, bin/job says which
  2-intake/            box 2 Issue/Story/Task
  3-plan/              box 3 Planning
  4-build/             box 4 Build
  5-review/            box 5 Review
  6-ship/              box 6 Deliver
  7-done/              closed, not yet archived
  8-rejected/          not taken
```

Never edit through the board or move anything inside it. The next rebuild throws it away. Once the board exists, the plugin's hooks keep it fresh: at session start, before each prompt, and after any edit under `.bloop/` or any git command that moves the checkout. The rebuild runs in the background, so for a second or so after a change the board can still show the old state.

A closed issue moves once, into `.bloop/archive/<id>/`, through `bin/archive` at the end of `/bloop:retro`. Only on the base branch, only when nothing else can still touch it, and in a commit that holds only the move. `bin/job <id>` and `bin/metrics` still read archived issues. `bin/job --all` lists them.

`.bloop/fuzz/` sits beside `issues/` and holds one file per fuzzing run. Those are not issues: a finding worth acting on becomes an issue of its own.
