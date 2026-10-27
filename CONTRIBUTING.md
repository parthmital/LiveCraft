# Contributing

Purpose: how outside contributors propose changes. Read when: preparing a contribution.
Last verified: 2026-10-27.

## Setup

Follow `README.md#quick-start` and `README.md#prerequisites`. Read `AGENTS.md`, which holds the commands, conventions, and clean-room rules that apply to every change.

## Workflow

- Pick an unticked task from `plans/livecraft-1.0/CHECKLIST.md`, or open an issue first for anything outside the plan.
- Branch from `main`. One logical change per pull request.
- Commit messages: a short imperative subject line, a blank line, then `- ` bullets describing each change.
- Before pushing, run every check in `AGENTS.md#commands` and make them pass. Do not disable lints or tests.
- Add tests for every behaviour change, and update the docs the change affects in the same pull request.
- Never commit Ableton assets, screenshots, or proprietary files (`AGENTS.md#clean-room-rules`).
- Report security issues privately (`SECURITY.md#reporting`).
