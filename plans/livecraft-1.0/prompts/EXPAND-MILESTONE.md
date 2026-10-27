# Expand A Milestone Into Tasks

Purpose: the exact procedure for an Expand line in `CHECKLIST.md`. Read when: the first unticked line is `Expand Mxx`.

In this step you are the planner for this milestone: you write task files and change no product code. When the expansion is committed and pushed, you continue the run with the first new task, in the same conversation.

## Inputs To Read (in this order)

1. `plans/livecraft-1.0/START-HERE.md` and `CHECKLIST.md`.
2. The milestone file `plans/livecraft-1.0/milestones/Mxx-*.md`.
3. `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`, `DECISIONS.md`, `QUALITY-GATES.md`; `UI-FIDELITY.md` if the milestone has UI.
4. The rows for this milestone in `reference/PARITY-MATRIX.md`.
5. The manual sections the milestone lists: run `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py section "<number> <title>"` for each. Read them fully.
6. The current code the milestone touches. Use `ARCHITECTURE.md` to find it; read the files, do not guess their contents.
7. `templates/TASK-TEMPLATE.md`.

## Procedure

1. Run `cargo xtask ci` and record the result (test count) for the first task's "Before You Start".
2. Break the milestone's "Task Outline" into tasks. Each task:
   - has one observable outcome and ends in one commit that passes `cargo xtask ci`;
   - touches at most about 5 files and 200 changed lines (split otherwise; a mechanical rename across many files is allowed as one task if a script does it);
   - comes after every task it depends on; shared types and tests come before their users.
3. Write each task as `plans/livecraft-1.0/tasks/Mxx-NN-<slug>.md` (NN = 01, 02, ...) from the template. Each file must be self-contained:
   - Quote the current code it changes with path and line numbers, and the manual sentences it implements with page numbers.
   - New files and any non-trivial logic (algorithms, DSP, parsing, state machines, scheduling maths): the complete code.
   - Edits: a unique anchor copied from the file, and the exact replacement.
   - Tests first in the steps, with complete test code and expected values. DSP tests state the numeric expectation and tolerance.
   - A Verify table with exact commands and expected results, and an exact commit message.
   - Dependency additions name the exact version from `DECISIONS.md` (or move it forward under D28 and add the DECISIONS row in the same task).
   - Upgrades and migrations: write the steps from the crate's changelog and docs (in `~/.cargo/registry/src` once fetched, or docs.rs), list the expected compiler errors and fixes, and give a fallback. Do not trial the task while expanding: it runs straight after, and its fix attempts and the Unblock Procedure correct what the notes missed.
4. Owner-only work: wherever the milestone says HUMAN, owner, oracle render, capture, measurement, credentials, or install checklist, no checklist line waits for it. Instead the task that needs it:
   - implements everything with a fallback stated as fact: values from the manual, from `DESIGN.md`, or from a named standard (for example constant-power -3 dB pan law), and a test that pins the fallback;
   - writes the comparison against the owner reference as steps and Verify rows marked `Owner-gated: Qnn`, with the exact command (`cargo xtask oracle <name>`, `tools/ui_diff.py <state>`) and threshold from `QUALITY-GATES.md` or `UI-FIDELITY.md`;
   - appends a row Qnn to `OWNER-QUEUE.md` with the exact owner steps (copied from `prompts/OWNER-GUIDE.md` and the fixture names), the result path, and Status `Open`;
   - ends with the line marked `[~]` instead of `[x]` when the gated rows cannot run yet.
     Tools that compare against references (`cargo xtask oracle`, `ui_diff.py`) must report a missing reference as `PENDING <name>` with exit code 3, never as a pass, and `cargo xtask ci` never runs them.
5. Owner decisions (D-OWN-1 to D-OWN-6): implement with the default in `reference/DECISIONS.md#owner-decisions`. Where the default turns a feature off, still implement it behind its Cargo feature or setting, with tests that run with the feature on wherever its dependencies build without the owner (an SDK that needs a licence acceptance gets an `OWNER-QUEUE.md` row instead), so the owner's answer only flips a default.
6. Insert the new lines into `CHECKLIST.md` directly below this Expand line, in order, in the same format as the P0 lines: checkbox `[ ]`, the ID, the title, and a link labelled `task` to the new task file.
7. Add the milestone's acceptance criteria as the last task (`Mxx-NN-milestone-verification.md`): run every acceptance check, update the parity matrix rows with evidence (owner-gated criteria as `Pending Qnn`), update `ARCHITECTURE.md`, `README.md`, and other affected docs.

## Cold-Read Review (do this before committing)

Read each task file as an executor that has only that file and `START-HERE.md`. Fix:

- any reference to unwritten context ("as above", "similar to Task 3", "etc.", "as needed", "appropriate", "choose", "TBD");
- any path or command that neither exists now nor is created by an earlier task;
- any step whose result cannot be checked by a command;
- any task over the size limit;
- any step that waits for the owner instead of queueing.

## Commit And Continue

1. Tick the Expand line.
2. `git add plans/livecraft-1.0/tasks/Mxx-* plans/livecraft-1.0/CHECKLIST.md` (and `reference/DECISIONS.md` or `OWNER-QUEUE.md` if changed).
3. Commit `Expand Mxx into N tasks` with one bullet per task title. Push.
4. Continue the run with the first new task. Do not end your turn.

## If The Milestone File Is Wrong

If the current code or the manual contradicts the milestone file (a decision cannot work, a section moved), do not improvise: correct the milestone file with the evidence in a short `Revision <date>` note at its end, then expand from the corrected file in the same commit. If the problem is a decision in `DECISIONS.md` or `TARGET-ARCHITECTURE.md`, change it minimally with the same revision note, add an `OWNER-QUEUE.md` row "Review revision of <file>" (non-blocking), and continue.
