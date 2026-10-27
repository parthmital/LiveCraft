# M01: Workspace restructure into target crates

Purpose: scope, decisions, and acceptance criteria for moving today's four crates into the target layout. Read when: expanding or reviewing M01.
Phase: 1. Depends on: Phase 0 complete. Manual: none. Parity rows: none.

## Goal

The code lives in the crates named by `reference/TARGET-ARCHITECTURE.md`, behaviour is unchanged (proven by characterisation tests written first), and `cargo xtask layering` enforces the new dependency rules.

## Scope

In: create `livecraft-model`, `livecraft-formats`, `livecraft-devices`, `livecraft-engine`, `livecraft-ui`, `livecraft-testkit`, and `apps/livecraft`; move code into them; delete `livecraft-core`, `livecraft-gui`, `livecraft-cli` once empty; keep `livecraft-dsp` for primitives only.
Out: crates with no code yet (`livecraft-time` M02, `livecraft-session` M04, `livecraft-warp` M23, `livecraft-library` M24, `livecraft-control` M38, `livecraft-server` M42, plug-in crates M37). Any behaviour change or API redesign (M02 to M05).

## Decisions

- Moves use `git mv` so history follows files. Each task lists the old-to-new import path table it rewrites.
- File destinations:

  | From                                                                                                                   | To                                                                                              |
  | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
  | `crates/livecraft-core/src/model.rs`, `command.rs`                                                                     | `crates/livecraft-model/src/` (command moves again in M04)                                      |
  | `crates/livecraft-core/src/als.rs`, `theme.rs`                                                                         | `crates/livecraft-formats/src/`                                                                 |
  | `crates/livecraft-dsp/src/biquad.rs`                                                                                   | stays in `crates/livecraft-dsp/src/`                                                            |
  | `crates/livecraft-dsp/src/{autofilter,autopan,chorus,compressor,delay,eq3,eq8,gate,limiter,redux,reverb,saturator}.rs` | `crates/livecraft-devices/src/effects/`                                                         |
  | `crates/livecraft-dsp/src/{operator,simpler,drumrack}.rs`                                                              | `crates/livecraft-devices/src/instruments/`                                                     |
  | `crates/livecraft-dsp/src/processor.rs`                                                                                | `crates/livecraft-devices/src/processor.rs`                                                     |
  | `crates/livecraft-dsp/src/{engine,graph,bouncer,audio_clip}.rs`                                                        | `crates/livecraft-engine/src/`                                                                  |
  | `crates/livecraft-gui/src/*`                                                                                           | `crates/livecraft-ui/src/`                                                                      |
  | `crates/livecraft-cli/src/main.rs`, `tests/`                                                                           | `apps/livecraft/src/main.rs`, `apps/livecraft/tests/` (package `livecraft`, binary `livecraft`) |

- Tests move with their code; `src/tests.rs` blocks are split per destination crate by test subject.
- `LAYERS` in `xtask/src/main.rs` becomes: dsp [], model [], formats [model], devices [model, dsp], engine [model, dsp, devices, formats], ui [model, formats, engine], testkit [any, dev only, not listed as normal], livecraft [model, formats, engine, ui, devices, dsp], xtask []. (UI depending on engine is temporary until M04 introduces `livecraft-session`; M04 removes it.)
- `livecraft-testkit` gets the first shared helper: `render_project_summary(project: &Project, bars: u32, sample_rate: u32) -> RenderSummary { rms_l, rms_r, peak_l, peak_r, frames }` used by characterisation tests.

## Task Outline

1. Characterisation tests: render the default project and the CLI `new` demo for 4 bars at 44.1 kHz; assert frames, peaks, and RMS against values recorded now (tolerance 1e-6). Assert `livecraft inspect --json` output of the demo set equals a committed `insta` snapshot.
2. Create `livecraft-testkit` with `RenderSummary` and move the helper from task 1 into it.
3. Create `livecraft-model`; move `model.rs` and `command.rs`; re-export from `livecraft-core` temporarily so other crates compile; move model tests.
4. Create `livecraft-formats`; move `als.rs` and `theme.rs`; temporary re-export from core; move tests.
5. Remove `livecraft-core` (switch all imports to model and formats; delete the crate).
6. Create `livecraft-devices`; move effects, instruments, processor; temporary re-exports from dsp; move tests.
7. Create `livecraft-engine`; move engine, graph, bouncer, audio_clip; move tests; reduce `livecraft-dsp` to `biquad.rs` and remove its re-exports.
8. Rename `livecraft-gui` to `livecraft-ui` (directory, package, imports).
9. Move `livecraft-cli` to `apps/livecraft` (package `livecraft`); update CI artifact path if needed, `.cargo/config.toml` alias, README.
10. Update `LAYERS`, `ARCHITECTURE.md` (module map, dependency rules), `AGENTS.md` commands (`cargo test -p <new names>`), README structure; milestone verification.

## Acceptance Criteria

- `cargo xtask ci` passes; the characterisation tests from task 1 pass unchanged at the end.
- `cargo metadata` lists exactly: livecraft-model, livecraft-formats, livecraft-dsp, livecraft-devices, livecraft-engine, livecraft-ui, livecraft-testkit, livecraft, xtask.
- `grep -rn "livecraft_core\|livecraft_gui\|livecraft_cli" --include=*.rs .` returns nothing.
- `ARCHITECTURE.md` module map matches the new layout.
