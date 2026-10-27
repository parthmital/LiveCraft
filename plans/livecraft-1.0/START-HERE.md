# LiveCraft 1.0 Implementation Plan

Goal: take LiveCraft from today's pre-alpha prototype to version 1.0, a native Rust DAW that a Live 12.4 Suite user cannot tell apart from Live in interface, workflows, sound, and behaviour, and that then exceeds Live with the improvements in `reference/BEYOND-LIVE.md`.

## How This Plan Works

- `PLAN.md` explains the phases, milestones, order, and exit criteria. Read it once.
- `CHECKLIST.md` is the only record of progress. Every line is one unit of work, done in order.
- Two kinds of checklist lines:
  - **Task** (`tasks/<id>.md`): a self-contained instruction file. Do exactly what it says.
  - **Expand** (`milestones/Mxx-*.md`): turn that milestone into task files with `prompts/EXPAND-MILESTONE.md`, add them to `CHECKLIST.md` directly below the Expand line, commit, and stop. The next session executes the new tasks.
- Lines marked **HUMAN** need the repository owner (for example capturing screenshots from the installed Live). The executor stops and asks; it never fakes the result.
- `reference/` holds the decisions every task relies on. Tasks repeat what they need, so the executor reads reference files only when a task or the expansion prompt says so.

## Folder Map

```text
plans/livecraft-1.0/
  START-HERE.md           this file: executor rules, project facts, prompts per tool
  CHECKLIST.md            ordered work lines and the Blockers log (single source of progress)
  PLAN.md                 phases, milestones, exit criteria
  tasks/                  self-contained task files (P0-xx now; Mxx-NN added by expansions)
  milestones/             one spec per milestone M01 to M46 (scope, decisions, outline, acceptance)
  reference/
    TARGET-ARCHITECTURE.md  crates, threads, engine, model, UI, file-format decisions
    DECISIONS.md            every plan-wide decision and the owner decisions still open
    QUALITY-GATES.md        definition of done, test levels, audio and performance thresholds
    UI-FIDELITY.md          how visual parity is captured, measured, and judged
    PARITY-MATRIX.md        every Live 12.4 manual section and device, its milestone, its status
    BEYOND-LIVE.md          verified improvements over Live and their milestones
  prompts/
    EXPAND-MILESTONE.md     procedure for Expand lines
    OWNER-GUIDE.md          HUMAN work and owner decisions
  templates/TASK-TEMPLATE.md
  tools/                  validated helper scripts (manual queries, migrations) and requirements.txt
  files/                  validated files that tasks copy verbatim (xtask source, deny.toml, CI workflow)
```

## Rules For The Executor

1. Work on exactly one checklist line per session: the first unticked one. Read only `START-HERE.md`, `CHECKLIST.md`, and that line's file, plus the files it names.
2. Follow the steps exactly and in order. Do not refactor, rename, reformat, upgrade, or "improve" anything the task does not name.
3. A line is done only when every command in its Verify table gives the expected result, its box in `CHECKLIST.md` is ticked, and the commit is pushed.
4. Never edit or delete tests to make them pass, skip a check, add a dependency the task does not name, add `#[allow(...)]` to silence a lint, or use `--no-verify` or `--force`.
5. When stuck, follow the task's "If Something Fails" section: log the blocker under Blockers in `CHECKLIST.md`, commit only that file, push, and stop.
6. Ignore files that show as changed only because of line endings: `git diff --ignore-cr-at-eol --stat` does not list them.
7. Never commit Ableton assets, screenshots of Live, manual page renders, or anything under `.agent-local/` (see `AGENTS.md#clean-room-rules`).
8. Before adding or upgrading a dependency, a task names the exact version. Use `cargo add <crate>@<version> -p <package>`; never edit `Cargo.lock` by hand.
9. A HUMAN line: print what the owner must do (copied from the task), then stop without ticking it. The owner ticks it.

## Project Facts

- Stack: Rust stable 1.99.1 (pinned by Task P0-01 in `rust-toolchain.toml`), Cargo workspace, egui and eframe for the GUI, cpal for audio, hound for WAV, quick-xml and flate2 for `.als`, clap for the CLI.
- Platform of the reference machine: Windows 11, Git Bash and PowerShell available. Ableton Live 12 Suite 12.4 is installed at `C:\ProgramData\Ableton\Live 12 Suite` (used only as a black-box reference by HUMAN tasks).
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

Copy these exactly. `NEXT` and `DONE` are defined first; the tool sections show where to paste them.

**NEXT**

```text
Read plans/livecraft-1.0/START-HERE.md and follow its rules. Do the first unticked line in plans/livecraft-1.0/CHECKLIST.md, exactly as its file says, and nothing else. If the line is an Expand line, follow plans/livecraft-1.0/prompts/EXPAND-MILESTONE.md. If it is a HUMAN line, print the owner instructions from its file and stop.
```

**DONE**

```text
Done when every command in that file's Verify table gives its expected result, its box in plans/livecraft-1.0/CHECKLIST.md is ticked, and the commit is pushed with git status --short --branch clean and not ahead of origin, or when a blocker is logged in CHECKLIST.md and pushed.
```

### Antigravity (primary executor: Gemini 3.8 Flash High)

1. Start a new conversation for every checklist line and select Gemini 3.8 Flash High.
2. Send `/goal ` followed by NEXT, a space, and DONE. Goal mode stops the agent from ending its turn before the evidence exists.
3. When a line stays blocked after its allowed attempts: on a paid plan send `/boost Fix the blocker logged for the current line in plans/livecraft-1.0/CHECKLIST.md, then finish that line exactly as its file says.` followed by DONE. Otherwise use the Blocked Task prompt with a stronger model.
4. For Expand lines, prefer a stronger model if available (for example a Gemini Pro model, or Claude), because expansion is design work; Flash is acceptable if nothing else is available.
5. Do not use `/plan` (the plan exists) or parallel agents on this branch (the checklist is sequential).

### Claude Code

1. `/clear`, then `/model` to pick the model.
2. Send `/goal ` + NEXT + DONE + ` Stop after 40 turns.`

### Codex, OpenCode, Gemini CLI, Cursor, and others

- Codex 0.128.0 or later: new session, `/goal ` + NEXT + DONE.
- OpenCode: `/new`, Build agent, then NEXT + DONE as a plain message; if it stops early, `Continue the current line. ` + DONE.
- Any other tool: new session, NEXT + DONE, in its autonomous mode if it has one.

### Whole Phase In One Session

Only for a strong executor with a goal mode, never for Expand or HUMAN lines:

```text
/goal Complete every unticked Task line in plans/livecraft-1.0/CHECKLIST.md in order, one at a time, following plans/livecraft-1.0/START-HERE.md, stopping before the first Expand or HUMAN line or at the first logged blocker. Each task is done as its file defines, with its own commit and push.
```

### Blocked Task

```text
The current line in plans/livecraft-1.0/CHECKLIST.md is blocked; the entry under Blockers has the error. Find the root cause, fix it, finish the line exactly as its file says, correct the task file if its instructions were wrong, and remove the blocker entry in the same commit.
```

Then DONE.

### Milestone Review (after the last task of each milestone)

```text
Review every commit since the milestone's Expand line against plans/livecraft-1.0/milestones/<milestone file> and plans/livecraft-1.0/reference/QUALITY-GATES.md. List each acceptance criterion as met or not met with evidence (command output or file and line). Fix nothing; append the list to the milestone file under "Review" and commit.
```
