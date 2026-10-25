# Task Specification: task-026

## 1. Metadata

- **ID:** `task-026`
- **Phase:** Phase 5: Headless Workflows, Testing, CI & Documentation
- **Title:** Automated Visual Regression Test Harness & Screenshot Diffing
- **Dependencies:** `task-025`
- **Manual Grounding:** Ableton Live 12 Reference Manual Visual Specification

## 2. Context & Problem Statement

To guarantee that the LiveCraft GUI matches Ableton Live 12 without regressions, the repository requires an automated visual regression harness. This harness captures headless screenshots across views and themes and calculates visual fidelity metrics against reference images.

## 3. Scope of Work

1. Headless screenshot automation in `livecraft-cli`:
   - `livecraft gui --screenshot <path> --view session --theme dark`
   - `livecraft gui --screenshot <path> --view arrangement --theme light`
   - `livecraft gui --screenshot <path> --view session --tab 1` (Piano Roll / Clip View)
2. Integrate visual comparison script `.agent-local/scripts/compare_ui.py`:
   - Calculate Mean Absolute Error (MAE) and generate side-by-side comparison images.
   - Assert visual consistency across GUI updates.

## 4. Files to Inspect

- `crates/livecraft-cli/src/main.rs` (GUI subcommand)
- `.agent-local/scripts/compare_ui.py`

## 5. Files to Modify

- `crates/livecraft-cli/src/main.rs`
- `crates/livecraft-gui/src/app.rs`

## 6. What NOT to Touch

- Do NOT touch DSP algorithms or core model.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/app.rs`:
   - Ensure `screenshot_path` triggers on frame 2 (after full layout calculation and texture caching) so rendering is complete.
   - Save high-DPI image via `image::save_buffer` or egui screenshot capture.
2. Verify screenshot generation:
   ```bash
   cargo run --bin livecraft -- gui --screenshot target/test_shot.png --view session
   ```
3. Run `python .agent-local/scripts/compare_ui.py --view session` and verify comparison artifact generation.

## 8. Verification Command

```bash
cargo run --bin livecraft -- gui --screenshot target/test_shot.png --view session
```

## 9. Acceptance Criteria

- [ ] Headless screenshot capture executes in under 2 seconds and saves valid PNG.
- [ ] Both Dark Neutral and Light Ableton themes capture cleanly.
- [ ] Visual diff artifacts are stored in `.agent-local/output/comparisons/`.

## 10. Failure Handling Protocol

If window fails to initialize headlessly on Linux CI:

- Use software OpenGL backend (`LIBGL_ALWAYS_SOFTWARE=1`) or virtual display (`xvfb-run`).
