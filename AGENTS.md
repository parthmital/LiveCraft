# Agent Guide

Purpose: rules and commands every agent task in this repository needs. Read when: always.
Last verified: 2026-10-27.

## Commands

- Format check: `cargo fmt --all -- --check` (fix: `cargo fmt --all`)
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Test all: `cargo test --workspace`
- Test one crate: `cargo test -p livecraft-core` (also `livecraft-dsp`, `livecraft-gui`, `livecraft-cli`)
- Test one test: `cargo test -p livecraft-dsp test_eq8_processing`
- Release build: `cargo build --release --workspace`
- Toolchain: pinned to Rust 1.99.1 by `rust-toolchain.toml`; crate metadata comes from `[workspace.package]` in `Cargo.toml`.
- Run: `cargo run --bin livecraft -- <subcommand>`; GUI frame capture: `cargo run --bin livecraft -- gui --screenshot out.png --view session --theme dark`
- Query the Live 12 manual: see `docs/live-manual.md`.

Run format check, lint, test, and release build before every commit. Baseline on 2026-10-27: all clean, 44 tests pass.

## Implementation Plan

- Work comes from `plans/livecraft-1.0/`. Read its `START-HERE.md` before any plan task; `CHECKLIST.md` is the only source of task status.
- One prompt runs the whole checklist (`START-HERE.md#prompts`); one commit per task, never skip or reorder lines. Owner-only work goes to `plans/livecraft-1.0/OWNER-QUEUE.md` and never stops the run.

## Conventions

- Crate direction: `livecraft-core` depends on nothing internal; `livecraft-dsp` on core; `livecraft-gui` on core and dsp; `livecraft-cli` on all. Never add a reverse edge.
- Tests live in a `#[cfg(test)] mod tests` block of the module they test, or in `crates/<crate>/tests/` for cross-module behaviour. Every new behaviour gets a test in the same commit.
- Errors: library code returns `Result` with `thiserror` enums or `anyhow::Result`; no `unwrap()` or `expect()` outside tests.
- Logging: library crates use `log::error!`, `log::warn!`, `log::info!`, `log::debug!`; never `eprintln!` or `println!`. Only `crates/livecraft-cli` prints to the terminal. Never log on the audio thread.
- Audio callback code (`write_audio*` in `engine.rs` and everything it calls) must not allocate, lock, log, or do I/O. See `ARCHITECTURE.md#known-violations` for where today's code breaks this.
- Live terms are used exactly as the manual spells them (Session View, Clip View, Track Activator, Main track).
- MIDI note names follow Live: note 60 is C3.

## Clean-Room Rules

- Reproduce Live's behaviour and layout from the manual and from observing a legitimately installed Live. Never copy, decompile, or disassemble Ableton binaries, and never copy code from Ableton.
- Never commit Ableton assets: fonts, icons, bitmaps, theme files (`.ask`), Core Library samples or presets, Packs, or screenshots of Live or manual pages. Keep such references in Git-ignored `.agent-local/`.
- Draw every icon in code. Use only fonts with an open licence that permits redistribution.
- Never use "Ableton" or "Live" as product branding; only in compatibility statements.

## Gotchas

- **`docs/ableton-live-12-reference-manual.pdf` missing**: it is Git-ignored (108 MB). Ask the owner for it; do not download from unofficial sources.
- **Manual screenshots are tiled into many small images**: render whole pages to PNG instead of extracting images (`plans/livecraft-1.0/tools/render_manual_pages.py`).
- **`test_parse_real_ableton_template` passes without asserting anything**: it only asserts when Live 12 Suite is installed at `C:\ProgramData\Ableton\Live 12 Suite`.
- **GUI screenshot needs a display and OpenGL 3.3**: `--screenshot` opens a real window for one frame.
- **Line-ending-only diffs**: `.gitattributes` sets `text=auto`; use `git diff --ignore-cr-at-eol --stat`.
- **Bash tool on Windows**: write scripts, JSON, and regexes to files instead of passing backslashes through the shell.

## Docs

- `README.md`: overview, setup, CLI reference, documentation index.
- `ARCHITECTURE.md`: crate responsibilities, data flow, known violations. Read before moving code.
- `DESIGN.md`: UI fidelity rules and where tokens live. Read before UI work.
- `SECURITY.md`: threat model and open findings. Read before touching file parsing, plug-ins, or networking.
- `RESEARCH.md`: library and approach decisions with sources. Read before adding a dependency.
- `docs/live-manual.md`: manual page map and query tools. Read before implementing any Live feature.
- `docs/keyboard-shortcuts.md`: Live 12 shortcuts. Read before binding keys.
