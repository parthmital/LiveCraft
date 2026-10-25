# Task Specification: task-025

## 1. Metadata

- **ID:** `task-025`
- **Phase:** Phase 5: Headless Workflows, Testing, CI & Documentation
- **Title:** Headless CLI Subcommand Polish & Faster-Than-Realtime Bouncer
- **Dependencies:** `task-024`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 20 (Bounce to Audio) & Architecture Manual (Dual-Surface Execution)

## 2. Context & Problem Statement

A core pillar of LiveCraft's architectural superiority over Ableton Live is headless execution and scriptable automation. All DAW operations must be executable without launching the desktop GUI:

- `info`: Metadata and set inspection.
- `inspect`: Deep JSON track, clip, and device inspection.
- `new`: Template project generation.
- `render`: Offline faster-than-realtime (~133x) bounce to 16, 24, or 32-bit float WAV.
- `stems`: Multi-track stem export bouncing each track into an isolated stem file.
- `play`: Real-time playback in the terminal with live ASCII VU meters.
- `convert`: Lossless bidirectional `.als` <-> JSON translation.

## 3. Scope of Work

1. Audit and polish all CLI subcommands in `crates/livecraft-cli/src/main.rs`.
2. Verify that `livecraft stems` exports all individual player tracks plus return tracks to an output directory.
3. Verify that `livecraft render` produces a bit-accurate WAV audio file with clean headers and no clipping.
4. Expand integration test suite in `crates/livecraft-cli/tests/cli_tests.rs`.

## 4. Files to Inspect

- `crates/livecraft-cli/src/main.rs`
- `crates/livecraft-cli/tests/cli_tests.rs`
- `crates/livecraft-dsp/src/bouncer.rs`

## 5. Files to Modify

- `crates/livecraft-cli/src/main.rs`
- `crates/livecraft-cli/tests/cli_tests.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-cli/src/main.rs`:
   - Verify error reporting: invalid files or missing paths return human-readable error messages and non-zero exit codes.
   - For `stems`: ensure track name sanitization (e.g. replacing slashes with underscores) so file creation succeeds across Windows, macOS, and Linux.
   - For `render`: ensure `--duration-bars`, `--sample-rate`, and `--bits` arguments are passed accurately to `OfflineBouncer`.
2. In `crates/livecraft-cli/tests/cli_tests.rs`:
   - Add integration tests covering `livecraft info`, `livecraft inspect --json`, `livecraft stems`, and `livecraft convert`.
3. Run `cargo test -p livecraft-cli --test cli_tests`.

## 8. Verification Command

```bash
cargo test -p livecraft-cli --test cli_tests
```

## 9. Acceptance Criteria

- [ ] All 8 CLI subcommands execute cleanly and deterministically.
- [ ] Offline rendering executes faster than real-time (~100x+) to 24-bit WAV.
- [ ] Multi-track stems export isolated WAV files for every track.

## 10. Failure Handling Protocol

If WAV header writing fails:

- Check `hound::WavSpec` sample format configuration (16-bit PCM vs 24-bit PCM vs 32-bit Float).
