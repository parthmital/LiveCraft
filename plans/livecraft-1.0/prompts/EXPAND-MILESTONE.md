# Expand A Milestone Into Tasks

Purpose: the exact procedure for an Expand line in `CHECKLIST.md`. Read when: the first unticked line is `Expand Mxx`.

You are the planner for this milestone, not the implementer. You write task files; you change no product code in this session.

## Inputs To Read (in this order)

1. `plans/livecraft-1.0/START-HERE.md` and `CHECKLIST.md`.
2. The milestone file `plans/livecraft-1.0/milestones/Mxx-*.md`.
3. `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`, `DECISIONS.md`, `QUALITY-GATES.md`; `UI-FIDELITY.md` if the milestone has UI.
4. The rows for this milestone in `reference/PARITY-MATRIX.md`.
5. The manual sections the milestone lists: run `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py section "<number> <title>"` for each. Read them fully.
6. The current code the milestone touches. Use `ARCHITECTURE.md` to find it; read the files, do not guess their contents.
7. `templates/TASK-TEMPLATE.md`.

## Procedure

1. Run `cargo xtask ci` and record the result (test count) for the tasks' "Before You Start".
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
4. HUMAN work (owner captures, renders in Live, opening files in Live): write it as its own task file whose steps are instructions for the owner, with exact file names and where to put them, and mark its checklist line `HUMAN`.
5. Insert the new lines into `CHECKLIST.md` directly below this Expand line, in order, in the same format as existing lines.
6. Add the milestone's acceptance criteria as the last task (`Mxx-NN-milestone-verification.md`): run every acceptance check, update the parity matrix rows with evidence, update `ARCHITECTURE.md`, `README.md`, and other affected docs.

## Cold-Read Review (do this before committing)

Read each task file as an executor that has only that file and `START-HERE.md`. Fix:

- any reference to unwritten context ("as above", "similar to Task 3", "etc.", "as needed", "appropriate", "choose", "TBD");
- any path or command that neither exists now nor is created by an earlier task;
- any step whose result cannot be checked by a command;
- any task over the size limit.

## Commit

1. Tick the Expand line.
2. `git add plans/livecraft-1.0/tasks/Mxx-* plans/livecraft-1.0/CHECKLIST.md` (and `reference/DECISIONS.md` if changed).
3. Commit `Expand Mxx into N tasks` with one bullet per task title. Push. Stop.

## If The Milestone File Is Wrong

If the current code or the manual contradicts the milestone file (a decision cannot work, a section moved), do not improvise: correct the milestone file with the evidence in a short "Revision <date>" note at its end, then expand from the corrected file in the same commit. If the problem is a decision in `DECISIONS.md` or `TARGET-ARCHITECTURE.md`, log a blocker instead and stop; those change only with the owner or a stronger model.
