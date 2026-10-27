# Quality Gates

Purpose: how every change is tested and what "done" means for code, features, and parity. Read when: writing or expanding any task, or reviewing a milestone.
Last verified: 2026-10-27.

## Definition Of Done (every task)

1. `cargo xtask ci` passes.
2. New behaviour has tests written in the same commit; bug fixes have a test that failed before the fix.
3. Docs affected by the change are updated in the same commit (`AGENTS.md`, `ARCHITECTURE.md`, `DESIGN.md`, `SECURITY.md`, `README.md`, `docs/`, the parity matrix).
4. One commit, pushed, checklist ticked.

## Definition Of Done (every Live feature)

A parity matrix row moves to Done only when all apply:

1. Behaviour matches the manual section cited in the row; the task quotes the manual sentences it implements.
2. Exposed as named commands in the registry (from M04 on), usable from the CLI and control server.
3. Undo and redo work for every edit it makes, proven by a test that applies, undoes, and compares the model.
4. Saved to and loaded from `.als` losslessly (from M09 on), proven by a round-trip test.
5. UI matches reference captures within `UI-FIDELITY.md` thresholds (from M12 on).
6. Audio-producing features pass the audio gates below.
7. Keyboard shortcuts from `docs/keyboard-shortcuts.md` (or its generated replacement) work.

## Test Levels

| Level            | Tool                                                    | Where                                                                                       | Runs in CI                                                                         |
| ---------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Unit             | `#[test]`                                               | `src/**/tests.rs` or inline `mod tests`                                                     | Yes                                                                                |
| Property         | `proptest` 1.12.0                                       | time maths, model invariants, command undo, parsers                                         | Yes                                                                                |
| Integration      | `crates/<crate>/tests/*.rs`                             | crate public APIs, CLI end to end                                                           | Yes                                                                                |
| Golden audio     | `livecraft-testkit` golden compare                      | `crates/livecraft-testkit/golden/` (LiveCraft's own renders, small WAV or hash)             | Yes                                                                                |
| UI interaction   | `egui_kittest` 0.37.0                                   | `crates/livecraft-ui/tests/`                                                                | Yes (Windows and Linux with wgpu software adapter if available; else Windows only) |
| UI snapshot      | `egui_kittest` snapshots                                | `crates/livecraft-ui/tests/snapshots/` (LiveCraft renders only)                             | Yes                                                                                |
| Reference parity | `plans/livecraft-1.0/tools/ui_diff.py` (created in M12) | compares LiveCraft captures with owner captures of Live in `.agent-local/reference/live/`   | No, local only (Live captures are never committed)                                 |
| Audio oracle     | `cargo xtask oracle` (created in M08)                   | compares LiveCraft renders with owner renders from Live in `.agent-local/reference/oracle/` | No, local only                                                                     |
| Real-time safety | `assert_no_alloc` around engine processing              | engine tests and a debug-build smoke test                                                   | Yes                                                                                |
| Benchmarks       | `criterion` 0.8.2                                       | `crates/<crate>/benches/`                                                                   | No; run in M41 and before releases                                                 |

Local-only gates skip with a printed notice when their reference files are absent, and never fail CI for that reason. Milestone reviews run them on the reference machine.

## Audio

- Determinism: rendering the same set twice gives byte-identical WAV output on the same machine and build.
- Neutral operations (manual chapter 39): unity gain, centred pan, a bypassed device, and an empty device chain pass input to output bit-exactly in 32-bit float. Each has a test.
- Oracle null test for a device or feature: render the same `.als` (written by LiveCraft, opened and rendered by the owner in Live with the export settings in M08) and subtract. Pass thresholds:
  - Linear, time-invariant processing (gain, pan, EQ, filters at static settings, sends, mixing): residual RMS at or below -90 dBFS relative to the reference.
  - Nonlinear or modulated processing (saturation, compression, LFOs, reverbs, synth voices): no sample-level null is expected. Pass when the 1/3-octave band energy difference is within 1.0 dB in every band from 31.5 Hz to 16 kHz, integrated loudness (ITU-R BS.1770) within 0.5 LU, and peak within 0.5 dB.
  - Time-stretch and pitch-shift: band difference within 2.0 dB, onset positions within 2 ms.
  - Randomised devices (Random, Humanize, Euclid seeds): compare statistics over 1000 events, not individual events.
- MIDI timing (manual chapter 40): events render at the exact sample computed from the tempo map; tests assert sample positions.
- Output safety: no NaN or infinity reaches the driver or a render (Task P0-16).

## Performance

Reference machine: the owner's Windows PC. Measured with `criterion` and the engine's CPU meter.

| Budget                                                                                            | Target                                                                                 |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Audio callback, 64 audio tracks each with EQ Eight, Compressor, Reverb, 48 kHz, 128-sample buffer | under 50 percent of one buffer period on one core; under 25 percent with M41 multicore |
| Dropouts in a 10-minute playback of the 64-track benchmark set                                    | zero                                                                                   |
| GUI frame time with the 64-track set in Session View                                              | under 8 ms (120 fps capable)                                                           |
| Cold start to interactive empty set                                                               | under 1.5 s                                                                            |
| Open Live 12.4's default template                                                                 | under 300 ms                                                                           |
| Offline render speed of the 64-track set                                                          | at least 10x real time                                                                 |
| Memory for an empty set                                                                           | under 200 MB                                                                           |

## Security

- Every file parser has size limits and a test with an oversized or malformed input (Tasks P0-14, P0-15 set the pattern).
- Every parser gets a `proptest` that feeds arbitrary bytes and asserts no panic.
- `cargo-deny` advisories, licences, bans, and sources pass in CI.
- Plug-ins run out of process (M37); a crashing plug-in test proves the engine survives.

## Milestone Review

After the last task of a milestone, a review appended to the milestone file records each acceptance criterion as met or not met with evidence. Unmet criteria become new tasks before the next milestone starts.
