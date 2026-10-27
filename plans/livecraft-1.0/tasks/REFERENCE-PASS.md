# Reference Pass

Purpose: finish `[~]` lines in `CHECKLIST.md` whose owner input has arrived. Used by every `Reference pass` line and by rule 11 of `START-HERE.md`. Size: one commit per finished line, plus fix tasks when needed.

## Before You Start

1. `git pull --ff-only` (the owner may have pushed queue updates), then `git status --short`: empty (line-ending-only files excepted).
2. Run `cargo xtask ci`. Expected: passes. Record the test count.

## Steps

1. Open `plans/livecraft-1.0/OWNER-QUEUE.md`. For each row whose Status is `Open`, check whether the result file or answer it names now exists (for example `ls .agent-local/reference/oracle/<name>.wav`, or an answer recorded in `reference/DECISIONS.md#owner-decisions`). If it exists, set Status to `Arrived <date>` (the owner may also have done this).
2. For each `[~]` line in `CHECKLIST.md`, top to bottom, whose queue rows are all `Done` or `Arrived`:
   1. Open that line's task file. Find the steps and Verify rows marked `Owner-gated: Qnn`.
   2. Do those steps exactly as written (typically: run `cargo xtask oracle <name>` or `./.venv/Scripts/python plans/livecraft-1.0/tools/ui_diff.py <state>`, then replace fallback values with measured ones).
   3. If a gated check fails and the fix fits the task's file list and size limit (about 5 files, 200 lines), fix it, at most 3 attempts, then the Unblock Procedure in `START-HERE.md`.
   4. If the fix is larger: write fix task files `<line ID>-R1-<slug>.md`, `<line ID>-R2-<slug>.md`, and so on from `templates/TASK-TEMPLATE.md`, insert them in `CHECKLIST.md` directly below this Reference pass line as `[ ]`, commit, push, and do them now, in order, before continuing this pass.
   5. When every gated check passes: change the line to `[x]`, set its queue rows to `Done <date>`, update the parity matrix row with the evidence if the task names one, and commit `Complete <line ID> with owner references` with one bullet per check. Push.
3. Lines whose queue rows are still `Open`: leave them `[~]`. They are not failures.
4. Finish the pass: if this pass came from a `Reference pass` checklist line, change that line to `[x]`, unless it is the Final reference pass and any `[~]` line remains, in which case change it to `[~]` and add one queue row "Final reference pass waits on rows Qnn, Qmm" listing the open rows. Commit `Run reference pass after <phase>` and push. Continue with the next `[ ]` line.

## Do Not

- Mark a gated check as passed without its reference file or answer.
- Lower a threshold from `reference/QUALITY-GATES.md` or `reference/UI-FIDELITY.md` to make a check pass.
- Commit anything under `.agent-local/`.

## Verify

| Command                                            | Expected result                                         |
| -------------------------------------------------- | ------------------------------------------------------- |
| `cargo xtask ci`                                   | exits 0                                                 |
| `git ls-files .agent-local`                        | no output                                               |
| `grep -c "\[~\]" plans/livecraft-1.0/CHECKLIST.md` | equal to the number of lines still waiting on Open rows |

## If Something Fails

- A gated check fails after the fix attempts: run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure.
