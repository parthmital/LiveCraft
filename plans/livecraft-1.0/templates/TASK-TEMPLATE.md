````markdown
# Task <ID>: <title>

Depends on: <previous task ID>. Size: <N files, about M changed lines>. Milestone: <Mxx or P0>.

## Goal

<One or two sentences: the observable outcome.>

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty (line-ending-only files excepted).
2. In `plans/livecraft-1.0/CHECKLIST.md`, no line above <ID> is `[ ]`, and <ID> is `[ ]`.
3. Run `cargo xtask ci`. Expected: passes (<test count after the previous task> tests).

## Context

<What exists now, quoted with paths and line numbers. Every decision this task depends on, stated as fact with its source (DECISIONS.md ID, manual page). Manual sentences this task implements, quoted with page numbers. Conventions to copy, with a short example from the repository.>

## Steps

1. Create `<path>` with exactly this content:

   ```rust
   <complete file>
   ```

2. In `<path>`, find this unique text:

   ```rust
   <anchor lines copied from the file>
   ```

   Replace it with:

   ```rust
   <new lines>
   ```

3. Run `<command>`. Expected: <result>.

## Do Not

- Change files other than: `<path>`, `<path>`, `plans/livecraft-1.0/CHECKLIST.md`.
- <Task-specific traps.>

## Verify

| Command             | Expected result               |
| ------------------- | ----------------------------- |
| `cargo xtask ci`    | exits 0; <N> tests pass       |
| `<behaviour check>` | <exact output or observation> |

## Commit And Push

1. Tick <ID> in `plans/livecraft-1.0/CHECKLIST.md` (`[~]` if an Owner-gated row could not run; see `prompts/EXPAND-MILESTONE.md` step 4).
2. `git add <each changed path>` (never `git add -A` or `git add .`).
3. Commit:

   ```text
   <subject>

   - <change>
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A Verify command fails: read the whole error, fix it within the files listed under "Do Not", rerun every Verify command. At most 3 attempts, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
- Still failing, or the fix needs another file after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: do not commit code. Append under Blockers in `CHECKLIST.md`: `<ID>, <date>: <what failed>`, the failing command, and the last 20 lines of its output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
````
