# LiveCraft 1.0 Implementation Plan

Goal: take LiveCraft from today's pre-alpha prototype to version 1.0, a native Rust DAW that a Live 12.4 Suite user cannot tell apart from Live in interface, workflows, sound, and behaviour, and that then exceeds Live with the improvements in `reference/BEYOND-LIVE.md`.

## How This Plan Works

- One prompt runs the whole plan, from the first unticked line to the release. The executor does every line in order, commits and pushes each, and moves straight on to the next. It does not stop between lines, tasks, milestones, or phases.
- `CHECKLIST.md` is the only record of progress. Line states: `[ ]` to do, `[x]` done, `[~]` done except owner input queued in `OWNER-QUEUE.md`.
- Three kinds of checklist lines:
  - **Task** (`tasks/<id>.md`): a self-contained instruction file. Do exactly what it says.
  - **Expand** (`milestones/Mxx-*.md`): turn that milestone into task files with `prompts/EXPAND-MILESTONE.md`, insert them directly below the Expand line, commit, push, then continue with the first new task.
  - **Reference pass** (`tasks/REFERENCE-PASS.md`): finish `[~]` lines whose owner input has arrived.
- Owner-only work (captures and renders from the installed Live, measurements, legal advice, signing certificates) never stops the run. The executor does everything a model can, adds a row to `OWNER-QUEUE.md`, marks the line `[~]`, and continues. The owner fills the queue whenever convenient, even while the run is going. The executor never fakes owner input.
- Owner decisions D-OWN-1 to D-OWN-6 have defaults in `reference/DECISIONS.md#owner-decisions`. The executor uses the default; the owner may change it at any time.
- `reference/` holds the decisions every task relies on. Tasks repeat what they need, so the executor reads reference files only when a task or the expansion procedure says so.

## Folder Map

```text
plans/livecraft-1.0/
  START-HERE.md           this file: executor rules, unblock procedure, project facts, the run prompt
  CHECKLIST.md            ordered work lines and the Blockers log (single source of progress)
  OWNER-QUEUE.md          owner-only requests the executor queues without stopping
  PLAN.md                 phases, milestones, exit criteria
  tasks/                  self-contained task files (P0-xx now; Mxx-NN added by expansions; REFERENCE-PASS.md)
  milestones/             one spec per milestone M01 to M46 (scope, decisions, outline, acceptance)
  reference/
    TARGET-ARCHITECTURE.md  crates, threads, engine, model, UI, file-format decisions
    DECISIONS.md            every plan-wide decision and the owner decisions with their defaults
    QUALITY-GATES.md        definition of done, test levels, audio and performance thresholds
    UI-FIDELITY.md          how visual parity is captured, measured, and judged
    PARITY-MATRIX.md        every Live 12.4 manual section and device, its milestone, its status
    BEYOND-LIVE.md          verified improvements over Live and their milestones
  prompts/
    EXPAND-MILESTONE.md     procedure for Expand lines
    OWNER-GUIDE.md          how the owner runs the plan and fills the queue
  templates/TASK-TEMPLATE.md
  tools/                  helper scripts (manual queries, migrations) and requirements.txt
  files/                  files that tasks copy verbatim (xtask source, deny.toml, CI workflow)
```

## Rules For The Executor

1. Run the checklist in one go: take the first `[ ]` line, do it, commit, push, then take the next `[ ]` line. Never end your turn between lines. For each line read only `START-HERE.md`, that line's file, and the files it names.
2. Follow the steps exactly and in order. Do not refactor, rename, reformat, upgrade, or "improve" anything the task does not name.
3. A line is done only when every command in its Verify table gives the expected result, its box in `CHECKLIST.md` is changed, and the commit is pushed.
4. Never edit or delete tests to make them pass, skip a check, add a dependency the task does not name, add `#[allow(...)]` to silence a lint, or use `--no-verify` or `--force`.
5. Owner-only work: do every step a model can, append the request to `OWNER-QUEUE.md` (exact steps, where the result goes, which check it unblocks), mark the line `[~]`, commit, push, continue. Where a check compares against an owner reference that does not exist yet, implement with the fallback the task gives and record the check as pending; never mark it passed.
6. When a task says "log a blocker" or "stop", first run the Unblock Procedure below. Only its step 4 logs a blocker and ends the run.
7. Keep context small: after each push, rely on the files, not your memory of earlier tasks. Re-read a file before editing it.
8. Ignore files that show as changed only because of line endings: `git diff --ignore-cr-at-eol --stat` does not list them.
9. Never commit Ableton assets, screenshots of Live, manual page renders, or anything under `.agent-local/` (see `AGENTS.md#clean-room-rules`).
10. Before adding or upgrading a dependency, a task names the exact version. Use `cargo add <crate>@<version> -p <package>`; never edit `Cargo.lock` by hand.
11. When no `[ ]` line is left, run `tasks/REFERENCE-PASS.md` once more. The run is complete when no `[ ]` line remains and every remaining `[~]` line waits on an `Open` row in `OWNER-QUEUE.md`.

## Unblock Procedure

1. Read the whole error and the code it names. Write the root cause in one sentence.
2. If the task file is wrong (an anchor no longer matches, an API changed, an expected value is wrong, a version is unavailable), correct the task file and add `Revision <date>: <what changed and why>` at its end. If a milestone or `reference/` decision cannot work, change it minimally, add the same revision note there, and add an `OWNER-QUEUE.md` row "Review revision of <file>" (it does not block).
3. Fix within the task's file list, plus files the root cause proves must change. At most 3 more attempts, rerunning every Verify command each time.
4. Still failing: discard code changes (`git checkout -- .` for tracked files), append under Blockers in `CHECKLIST.md`: `<line ID>, <date>: <root cause>`, the failing command, and the last 20 lines of its output. Commit only `CHECKLIST.md`, push, and end the run.

## Project Facts

- Stack: Rust stable 1.99.1 (pinned in `rust-toolchain.toml` by Task P0-01), Cargo workspace, egui and eframe for the GUI, cpal for audio, hound for WAV, quick-xml and flate2 for `.als`, clap for the CLI.
- Platform of the reference machine: Windows 11, Git Bash and PowerShell available. Ableton Live 12 Suite 12.4 is installed at `C:\ProgramData\Ableton\Live 12 Suite` (used only as a black-box reference by the owner).
- Python helper tools: create the repo-local environment once with `python -m venv .venv` then `./.venv/Scripts/python -m pip install -r plans/livecraft-1.0/tools/requirements.txt` (macOS or Linux: `./.venv/bin/python`). Never install Python packages globally.
- Checks before every commit, until Task P0-07 adds the one-command runner:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo build --release --workspace`
- After Task P0-07: `cargo xtask ci` runs all of them plus the layering and docs checks. After Task P0-12 it also runs `cargo-deny` (`--no-deny` skips it).
- Baseline on 2026-10-27 (commit `1735fa8`): format clean, clippy clean, 44 tests pass (livecraft-cli 4, livecraft-core 13, livecraft-dsp 25, livecraft-gui 2), release build succeeds, `livecraft gui --screenshot` works.
- Branch: `main`. Remote: `origin` (`https://github.com/parthmital/LiveCraft.git`).
- Commit format: imperative subject under 72 characters, blank line, `- ` bullets, then the attribution line if your tool adds one. Example:

  ```text
  Pin toolchain and share package metadata

  - Add rust-toolchain.toml pinning Rust 1.99.1 with rustfmt and clippy
  - Move version, edition, licence, and repository into workspace.package
  ```

- Manual: `docs/ableton-live-12-reference-manual.pdf` (Live 12.4 edition, 1039 pages, Git-ignored). Query it with `plans/livecraft-1.0/tools/manual.py`; `docs/live-manual.md` lists chapters and pages.

## Prompts

`RUN` and `DONE` are defined first; the tool sections show where to paste them. The prompt is idempotent: it always resumes at the first `[ ]` line, so if a conversation ends early (context limit, quota, crash, a blocker you fixed), start a new conversation and paste the same prompt.

**RUN**

```text
Read plans/livecraft-1.0/START-HERE.md and follow its rules. Work through plans/livecraft-1.0/CHECKLIST.md from the first unticked line to the last, in order, without stopping between lines: do each Task line exactly as its file says, expand each Expand line with plans/livecraft-1.0/prompts/EXPAND-MILESTONE.md and continue straight into the tasks it adds, and do each Reference pass line with plans/livecraft-1.0/tasks/REFERENCE-PASS.md. Owner-only work goes to plans/livecraft-1.0/OWNER-QUEUE.md and never stops the run. Run the Unblock Procedure in START-HERE.md before logging any blocker.
```

**DONE**

```text
Done when no line in plans/livecraft-1.0/CHECKLIST.md is [ ], every [~] line waits on an Open row in plans/livecraft-1.0/OWNER-QUEUE.md, and the last commit is pushed with git status --short --branch clean and not ahead of origin; or when a blocker remains after the Unblock Procedure and is logged in CHECKLIST.md and pushed.
```

### Antigravity (primary executor: Gemini 3.8 Flash High)

1. New conversation, select Gemini 3.8 Flash High.
2. Send `/goal ` followed by RUN, a space, and DONE. Goal mode stops the agent from ending its turn before the evidence exists, so it keeps going line after line, through every Expand line, to the release.
3. If the conversation ends before DONE holds (context or quota limit): new conversation, same prompt.
4. If the run ends on a logged blocker: on a paid plan send `/boost Fix the blocker logged in plans/livecraft-1.0/CHECKLIST.md as the Unblock Procedure in plans/livecraft-1.0/START-HERE.md says, remove the blocker entry, then continue the run.` followed by RUN and DONE. Otherwise run the Blocked Run prompt with a stronger model once, then paste the `/goal` prompt again.
5. Do not use `/plan` (the plan exists) or parallel agents on this branch (the checklist is sequential).

### Claude Code

1. `/model` to pick the model; use auto mode so check commands run unattended.
2. Send `/goal ` + RUN + DONE.

### Codex, OpenCode, Gemini CLI, Cursor, and others

- Codex 0.128.0 or later: new session, `/goal ` + RUN + DONE.
- OpenCode: `/new`, Build agent, then RUN + DONE as a plain message; whenever it stops before DONE holds, send `Continue. ` + RUN + DONE.
- Any other tool: new session, RUN + DONE, in its autonomous mode if it has one; paste again whenever it stops early.

### Blocked Run

For a stronger model, only when the run ended on a blocker:

```text
The run of plans/livecraft-1.0/CHECKLIST.md ended on the blocker logged under Blockers. Find the root cause, fix it, finish that line exactly as its file says, correct the task file with a dated revision note if its instructions were wrong, remove the blocker entry, commit, and push. Do only that line, then stop.
```

Then paste the run prompt again with the primary executor.

### Milestone Review (optional, for the owner)

```text
Review every commit since the milestone's Expand line against plans/livecraft-1.0/milestones/<milestone file> and plans/livecraft-1.0/reference/QUALITY-GATES.md. List each acceptance criterion as met, pending owner input, or not met, with evidence (command output or file and line). Fix nothing; append the list to the milestone file under "Review" and commit.
```
