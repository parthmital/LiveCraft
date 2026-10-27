# Task P0-10: Upgrade egui and eframe to 0.37 with the wgpu renderer

Depends on: P0-09. Size: 13 files, about 450 changed lines, nearly all by two validated scripts. Milestone: P0.

## Goal

The GUI builds on egui and eframe 0.37.0, renders with wgpu (DX12 by default on Windows), and looks the same as before in screenshots.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-09 is `[x]` or `[~]` and P0-10 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.
4. Capture "before" screenshots: `cargo dev -- gui --screenshot .agent-local/output/p0-10/before-session.png --view session` and the same with `--view arrangement` and `before-arrangement.png`.

## Context

- Decision D12: egui and eframe 0.37 with wgpu; DX12 default on Windows because Vulkan crashed (segmentation fault) on the reference machine on 2026-10-27, while DX12 and GL worked. `WGPU_BACKEND=vulkan|dx12|gl` still overrides.
- Do not enable eframe's `accesskit` feature yet: on 2026-10-27 it pulled in `zvariant`, `zbus_names`, `zvariant_derive`, which cargo-deny flags as unmaintained (RUSTSEC-2026-0370 to 0372). Accessibility is milestone M40.
- Breaking changes from 0.29 to 0.37 that affect this code (egui CHANGELOG): `Rounding` renamed `CornerRadius` with integer fields; `rect_stroke` needs a `StrokeKind`; `SidePanel` and `TopBottomPanel` replaced by `Panel`; panels take the parent `Ui` (`.show(ui, ...)`); `exact_height` and `exact_width` became `exact_size`; `SelectableLabel` became `Button::selectable`; `Ui::id` became `scope_id`; `allocate_new_ui` became `scope_builder`; `Context::style` and `set_style` became `global_style` and `set_global_style`; `eframe::App::update(ctx)` became `App::ui(ui)`; screenshots use `Context::request_screenshot(callback)` instead of an `Event::Screenshot`; `Margin::same` takes `i8`.
- `plans/livecraft-1.0/tools/egui_037_migrate.py` applies the mechanical renames; `plans/livecraft-1.0/tools/egui_037_finish.py` rewrites the head of `impl eframe::App` in `app.rs` (new `ui` entry point and callback screenshot, saved by a new `save_screenshot_and_exit` function) and switches `lib.rs` to wgpu with a `wgpu_options()` function. Both were validated on 2026-10-27: after them, clippy was clean, all tests passed, and screenshots of both views were identical in layout to the glow build.
- `StrokeKind::Middle` keeps 0.29's stroke placement (centred on the edge).

## Steps

1. In `Cargo.toml` `[workspace.dependencies]`, replace the `eframe` line with:

   ```toml
   eframe = { version = "0.37.0", default-features = false, features = ["default_fonts", "wgpu"] }
   ```

   and `egui = "0.29"` with `egui = "0.37.0"`.

2. Run `./.venv/Scripts/python plans/livecraft-1.0/tools/egui_037_migrate.py crates/livecraft-gui/src`. Expected counts (from the dry run; small differences are fine): about 110 `Rounding::same`, 29 `Rounding`, 46 `rect_stroke StrokeKind`, 4 `Panel exact_size`, 5 `.show(ctx`, 4 `SelectableLabel`, 3 `ui.id()`.

3. Run `./.venv/Scripts/python plans/livecraft-1.0/tools/egui_037_finish.py crates/livecraft-gui`. Expected: `rewrote App::ui head in ...app.rs` and `switched ...lib.rs to wgpu with DX12 default on Windows`.

4. Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`. Expected: clean. If errors remain, fix each using this table (only in `crates/livecraft-gui/src`):

   | Error mentions                             | Fix                                                                 |
   | ------------------------------------------ | ------------------------------------------------------------------- |
   | `expected u8, found floating-point number` | write the literal as an integer (`2.0` becomes `2`)                 |
   | `expected i8, found floating-point number` | same, for `Margin`                                                  |
   | deprecated `show_inside`                   | rename to `show`                                                    |
   | deprecated `CentralPanel::show` with `ctx` | pass the parent `ui` instead of `ctx` as the first argument         |
   | `no method named X` on `Context`           | look up the 0.37 name with `cargo doc -p egui --no-deps` and rename |

5. In `README.md`:
   - Tech Stack row `egui`, `eframe`: version cell `0.37 (wgpu renderer)`.
   - Quick Start table, `livecraft gui` row: replace `update GPU drivers (the current renderer needs OpenGL 3.3)` with `update GPU drivers; on Windows set WGPU_BACKEND=gl to try OpenGL`.
   - Troubleshooting: add the row `| GUI crashes at start on Windows | GPU driver problem in the selected wgpu backend | WGPU_BACKEND=gl cargo dev | Default is DX12; try gl, or update drivers |`.

6. In `AGENTS.md`, find:

   ```text
   - **GUI screenshot needs a display and OpenGL 3.3**: `--screenshot` opens a real window for one frame.
   ```

   Replace it with:

   ```text
   - **GUI screenshot needs a display and a GPU**: `--screenshot` opens a real window and saves frame 5. wgpu uses DX12 on Windows by default (Vulkan crashed on the reference machine); override with `WGPU_BACKEND=gl` or `vulkan`.
   ```

## Do Not

- Change files other than: `Cargo.toml`, `Cargo.lock`, files under `crates/livecraft-gui/src/`, `README.md`, `AGENTS.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not enable `accesskit` or `glow`. Do not restyle anything; this task only ports.

## Verify

| Command                                                                                                          | Expected result                                                                                                            |
| ---------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `cargo tree -i egui --depth 0`                                                                                   | `egui v0.37.0` (or a later 0.37 patch)                                                                                     |
| `cargo xtask ci`                                                                                                 | `xtask: all checks passed`, 49 tests                                                                                       |
| `cargo dev -- gui --screenshot .agent-local/output/p0-10/after-session.png --view session`                       | `Screenshot successfully saved`                                                                                            |
| `cargo dev -- gui --screenshot .agent-local/output/p0-10/after-arrangement.png --view arrangement --theme light` | `Screenshot successfully saved`                                                                                            |
| Open before and after PNGs side by side                                                                          | same panels, controls, and text in the same places; small spacing differences from egui's new default style are acceptable |

## Commit And Push

1. Tick P0-10 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml Cargo.lock crates/livecraft-gui/src README.md AGENTS.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Upgrade egui and eframe to 0.37 with the wgpu renderer

   - Apply the 0.29 to 0.37 API renames across livecraft-gui
   - Move the app to App::ui and callback-based screenshots
   - Render with wgpu, defaulting to DX12 on Windows unless WGPU_BACKEND is set
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A script exits with `markers not found` or `already migrated`: run `git checkout -- crates/livecraft-gui` and log a blocker with the message.
- The screenshot command crashes: retry with `WGPU_BACKEND=gl`. If that works, note it in the commit body and continue; if not, log a blocker.
- Other failures: fix within the listed files, at most 3 attempts. Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: discard code changes (`git checkout -- .`), append under Blockers in `CHECKLIST.md`: `P0-10, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
