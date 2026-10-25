# Task Specification: task-027

## 1. Metadata

- **ID:** `task-027`
- **Phase:** Phase 5: Headless Workflows, Testing, CI & Documentation
- **Title:** Zero Audio Allocations Assertion & Full CI Suite Pass
- **Dependencies:** `task-026`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 38 (Audio Thread Safety) & Coding Standards

## 2. Context & Problem Statement

Ableton Live 12 Chapter 38 demands that audio callbacks NEVER allocate heap memory (`malloc`, `free`, `Vec::new()`, `String::new()`) or acquire blocking mutexes. Furthermore, all 4 crates in the LiveCraft workspace must compile with zero warnings under strict clippy and formatting checks.

## 3. Scope of Work

1. Audio Callback Heap Allocator Audit:
   - Verify that steady-state audio rendering loop in `crates/livecraft-dsp/src/graph.rs` and `engine.rs` contains zero heap allocations.
   - Run allocation tracing or code review asserting that scratch buffers are pre-allocated at initialization.
2. Workspace-wide linting and formatting:
   - `cargo clippy --workspace --all-targets -- -D warnings` must pass with 0 errors and 0 warnings.
   - `cargo fmt --all -- --check` must pass with 0 diffs.
3. Full test suite execution across all crates.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/graph.rs`
- `crates/livecraft-dsp/src/engine.rs`

## 5. Files to Modify

- All crates as needed to fix clippy warnings or formatting.

## 6. What NOT to Touch

- Do NOT disable clippy warnings with `#![allow(warnings)]` or `#[allow(all)]`. Every warning must be properly resolved.

## 7. Step-by-Step Implementation Instructions

1. Run `cargo clippy --workspace --all-targets -- -D warnings`.
2. Inspect and fix any reported lints, dead code warnings, or unnecessary clones.
3. Run `cargo fmt --all`.
4. Run `cargo test --workspace`.

## 8. Verification Command

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

## 9. Acceptance Criteria

- [ ] Zero audio callback heap allocations during audio stream playback.
- [ ] `cargo clippy` passes clean across the entire workspace with `-D warnings`.
- [ ] `cargo fmt` passes clean.
- [ ] All workspace tests pass.

## 10. Failure Handling Protocol

If clippy warns on complex types or function signatures:

- Refactor type into a named struct or type alias rather than suppressing the warning.
