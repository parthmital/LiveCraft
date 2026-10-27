# M12: UI reference capture and fidelity tooling

Purpose: scope, decisions, and acceptance criteria for building the reference library and comparison tools. Read when: expanding or reviewing M12.
Phase: 3. Depends on: M11. Manual: chapters 3 to 31 and 41 (every screenshot); `reference/UI-FIDELITY.md`. Parity rows: none directly; every UI row depends on it.

## Goal

The owner has captured Live 12.4 in every state the plan needs, under the fixed protocol; tools measure tokens from captures and compare LiveCraft captures against them region by region; LiveCraft can capture any state headlessly at a fixed size.

## Decisions

- Capture list (`plans/livecraft-1.0/reference/capture-list.md`, created by task 1) names every state as `<area>__<state>__<theme>`, its LiveCraft-authored `.als` fixture, the steps to reach it, and the regions to compare (rectangles in pixels at 1920x1080).
- LiveCraft headless capture: `livecraft gui --screenshot out.png --size 1920x1080 --state <state-id> --theme <theme>` where `--state` loads a fixture and applies a scripted list of UI intents (open menus, select items, hover) defined in `crates/livecraft-ui/src/capture_states.rs`.
- Tools (Python, repo-local `.venv`, Pillow and NumPy from `tools/requirements.txt`):
  - `tools/ui_measure.py`: sample colours at points, measure run lengths (borders, gaps), text bounding boxes; writes JSON.
  - `tools/ui_diff.py`: per-region bounding boxes, colour deltas, SSIM (implemented in NumPy, 7x7 Gaussian window, grey), text alignment via column and row profiles; HTML report into `.agent-local/output/ui-diff/`.
- Reference files live only in `.agent-local/reference/live/` (never committed). Fixtures (`.als`) are committed under `fixtures/ui/`.
- egui snapshot tests (`egui_kittest` 0.37.0 with `wgpu` and `snapshot` features) render each state for regression detection in CI; Live comparison stays local.

## Task Outline

1. Write `capture-list.md` from manual chapters 3 to 31 and 41 (states, fixtures, regions, steps). At least 150 states.
2. Create the LiveCraft-authored fixtures for the first 30 states (main window, Control Bar, Browser, Session, Arrangement, Mixer, Clip View, Device View basics).
3. HUMAN: owner sets up Live per `UI-FIDELITY.md#capture-protocol` and captures the first 30 states with sidecars.
4. `--size` and `--state` options and `capture_states.rs`.
5. `tools/ui_measure.py` with unit tests on synthetic images.
6. `tools/ui_diff.py` with unit tests on synthetic images (shifted box, colour change, blur).
7. `egui_kittest` snapshot test harness in `livecraft-ui` running the 30 states in CI.
8. HUMAN: remaining states captured in batches as later milestones need them (each later UI milestone lists which states it needs; this task only documents the procedure).
9. Milestone verification: diff report generated for the 30 states (failing is expected now; the report establishes the baseline).

## Acceptance Criteria

- 30 states captured and diffed; the baseline report path is recorded in the review.
- Tool unit tests pass; `cargo xtask ci` runs the egui snapshot tests.
- No Live capture is tracked by Git (`git ls-files .agent-local` is empty).
