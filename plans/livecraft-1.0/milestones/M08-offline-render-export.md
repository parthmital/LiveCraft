# M08: Offline render and export

Purpose: scope, decisions, and acceptance criteria for Export Audio/Video parity and the headless render CLI. Read when: expanding or reviewing M08.
Phase: 1. Depends on: M07. Manual: 3.21 Saving and Exporting (60 to 61), 5.4 Live Sets (export sections, 135 to 141), 20 Bounce to Audio (425 to 430, render path only; UI in M36), 39.3 Non-Neutral Operations (dither, sample-rate conversion, 993 to 995). Parity rows: 3.21. Beyond: B03, B13.

## Goal

Renders use the exact real-time engine through the `VirtualDriver`, offer Live's export options, are deterministic, and come with a render report; the oracle harness compares LiveCraft renders with Live renders made by the owner.

## Decisions

- Export options follow Live's Export Audio/Video dialog as documented: rendered track (Main, all individual tracks, selected tracks), render start and length, include return and main effects, render as loop, convert to mono, normalize, create analysis file (produces LiveCraft's own analysis format), sample rate, encode PCM with file type WAV, AIFF, FLAC; bit depth 16, 24, 32 (float); dither options; encode MP3. The exact option list is quoted from the manual when the milestone starts; any option the manual lists that this milestone cannot do is listed as a gap with its milestone.
- 32-bit float renders are not clamped (fixes today's bouncer clamp); integer renders dither only when selected; dither algorithms: rectangular, triangular, and noise-shaped options named as Live names them (quote page 993 to 995).
- Encoders: WAV via `hound`; AIFF and FLAC writers in-house or via crates approved by cargo-deny (decide when the milestone starts with research; record in DECISIONS); MP3 via an LGPL-free encoder only if one passes cargo-deny, otherwise listed as a gap for M45.
- Determinism: fixed block size 512, no wall clock, seeded randomness from the set; render report JSON next to the file: settings, LiveCraft version, commit, SHA-256 of the output (B13).
- Oracle harness: `cargo xtask oracle <name>` renders `fixtures/oracle/<name>.als` (LiveCraft-authored sets, committed) and compares with `.agent-local/reference/oracle/<name>.wav` (owner renders from Live, never committed) using the metrics in `QUALITY-GATES.md#audio`; prints a table; skips when the reference is missing.

## Task Outline

1. `VirtualDriver` and offline render loop on the real engine; byte-identical double render test.
2. Render options type mirroring Live's dialog; CLI flags for each (`livecraft render`).
3. Individual and selected track rendering (stems) through the same path.
4. WAV writer for 16, 24, 32 float, no clamping for float; dither options.
5. AIFF and FLAC writers (research and DECISIONS row in the task).
6. Normalize, render as loop, mono conversion, include returns and main.
7. Render report JSON with SHA-256 (`sha2` 0.11.1).
8. Oracle harness in xtask plus metric functions (RMS, 1/3-octave bands, LUFS, true peak) in `livecraft-dsp` with unit tests against known signals.
9. HUMAN: owner renders the oracle sets in Live 12.4 with the documented settings into `.agent-local/reference/oracle/` (task lists each set and setting).
10. Milestone verification: oracle results for the basic sets (sine at unity, panned sine, two tracks summed, send to return at -6 dB) meet the linear thresholds.

## Acceptance Criteria

- Two renders of the same set are byte-identical.
- Oracle linear-processing sets null to at or below -90 dBFS against Live's renders (recorded table in the review).
- `livecraft render --help` lists every option from Live's dialog that is implemented; the review lists gaps with milestones.
