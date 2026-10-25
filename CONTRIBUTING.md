# Contributing to LiveCraft

Thank you for your interest in contributing to LiveCraft. LiveCraft is a clean-room, native Rust digital audio workstation striving for full compatibility and workflow parity with Ableton Live 12.

## Architectural Principles

1. 100% Native Pure Rust: Zero Electron or web view dependencies. The entire stack (engine, graph, parser, CLI, GUI) is written in Rust.
2. Real-Time Audio Safety: The audio callback thread must never allocate memory, block on mutexes, or perform disk or network I/O. Inter-thread communication must use atomic primitives or lock-free ring buffers.
3. Clean Document and Command Model: Pure-data structures with stable IDs and undo/redo coverage for all user actions.
4. Determinism: Offline rendering must produce sample-accurate, reproducible output.

## Development Environment Setup

### Prerequisites

- Rust 1.80+ (stable toolchain)
- Cargo components: `rustfmt` and `clippy`
- On Linux: `libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev libx11-dev`

### Setup Steps

```bash
# Clone the repository
git clone https://github.com/storytold/livecraft.git
cd livecraft

# Run full test suite (44 tests across core, dsp, gui, and cli)
cargo test --workspace

# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Build release binary
cargo build --release
```

## Pull Request Guidelines

1. Every pull request must pass the automated local and CI pipeline:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo build --release`
2. Audio code changes must maintain zero-allocation guarantees in real-time callbacks.
3. Add unit or integration tests for all newly introduced DSP units, commands, or parser features.
4. Update affected documentation across [README.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/README.md), [ARCHITECTURE.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ARCHITECTURE.md), and [ROADMAP.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/ROADMAP.md).
5. Keep commits focused and logically structured.

## Submitting Issues

- Use the bug report template for unintended behaviour, audio glitches, or crashes.
- Use the feature request template for proposing Ableton Live 12 parity additions or DSP devices.
