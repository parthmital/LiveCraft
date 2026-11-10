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

## Review (2026-11-10)

All M08 items (1 to 8, 10, plus Q12 queueing for item 9) were completed locally and committed together per milestone policy.

| Item | Summary |
| ---- | ------- |
| 1 | `VirtualDriver` switched to `Engine::process_offline` (deterministic sample-derived clock, no wall clock, metronome silenced); `OfflineBouncer` drives `VirtualDriver` at fixed 512-frame blocks with `skip_frames` for `render_start`. |
| 2 | `RenderOptions`, `RenderedTrack`, and `apps/livecraft/src/cli/render.rs` mirror Live 12.4 Export Audio/Video options (`--track`, `--selected-tracks`, `--start-bars`/`--start-beats`, `--duration-bars`/`--length-beats`, `--include-returns-main`, `--render-as-loop`, `--mono`, `--normalize`, `--analysis-file`, `--sample-rate`, `--source-sample-rate`, `--no-pcm`, `--format`, `--bits`, `--dither`, `--mp3`, `--no-report`). |
| 3 | `StemTarget` (`Main`, `DryNode`, `WetNode`) and topological Group summing in `CompiledGraph` support `Main`, `AllIndividualTracks` (skipping empty MIDI tracks per manual p. 127), `SelectedTracks`, and single-track stems with or without Return/Main effects. |
| 4 | Unclamped 32-bit IEEE float WAV/AIFC export; `DitherMode` (`No Dither`, `Rectangular`, `Triangular`, `POW-r 1`, `POW-r 2`, `POW-r 3`) with seeded PCG32 PRNG and `32768.0` / `8388608.0` scaling in `livecraft-dsp::dither` applied strictly below 32-bit. |
| 5 | Pure-Rust AIFF/AIFC (`fl32`) writer, `flacenc` 0.5.1 (Apache-2.0, 16/24-bit), and `rusty_mp3` 1.0.5 (Apache-2.0, 320 kbps CBR MP3) in `livecraft-formats::audio_encode`, recorded in `RESEARCH.md` and `DECISIONS.md` D32. |
| 6 | Two-pass `Render as Loop` using `DeviceProcessor::tail_samples()` on `ReverbEffect`, `DelayEffect`, `ChorusEffect`; `(L+R)*0.5` mono fold; peak-to-1.0 normalization; `rubato::Fft` downsampling when `source_sample_rate != sample_rate`; `.lcanalysis` JSON sidecars. |
| 7 | `RenderReport` JSON (`.report.json`) with `livecraft_version`, `git_commit` (`build.rs`), serialized `settings`, `sha2` 0.11.1 lowercase hex SHA-256, and BS.1770 metrics (B13). |
| 8 | `livecraft-dsp::metrics` (RMS, 4x oversampled BS.1770-4 true peak, BS.1770-4 integrated/short-term/momentary LUFS, 28-band `realfft` 1/3-octave energy, null residual) and `cargo xtask oracle` in `xtask/src/oracle.rs` plus four LiveCraft `.als` sets in `fixtures/oracle/`. |
| 9 | Queued as `Q12` in `OWNER-QUEUE.md` (`Status: Open`). |
| 10 | Verification: `cargo xtask ci` (510 passed), `rt-checks` allocation tests, `cargo xtask oracle` skip-when-missing, and analytical self-consistency tests on all four oracle `.als` sets (`<= -110 dBFS`). |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf` via `plans/livecraft-1.0/tools/manual.py`:

- 5.1.3.1, page 127: "All Individual Tracks — The post-fader signal at the output of each individual track, including Group Tracks, return tracks and the Main track. Note that only tracks that output audio are exported. If the Set contains a MIDI track without an instrument or any audio effects, it is not included in the render. If a MIDI track contains audio effects but no instrument, the file is exported but contains a silent audio signal. All exported files will have the same length"
- 5.1.3.2, page 128: "Include Return and Main Effects — When exporting individual or selected tracks, you can enable this option to also include the signal from any return tracks used by the tracks, as well as any effects used on the Main track."
- 5.1.3.2, page 128: "When rendered as a loop, the export process consists of two passes. The first pass does not actually write a file to disk, but processes the effect so that its total output length is calculated. The audio is then written to disk during the second pass, and the effect tail from the first pass is wrapped into the beginning of the rendered file."
- 5.1.3.2, page 128: "If you select a sample rate that is lower than the one used in the Set, the audio is first exported at the current Project-specific sample rate. The resulting file is then downsampled in a second step"
- 5.1.3.3, page 129: "File Type — Use the chooser to select the file type used for the export. WAV, AIFF, and FLAC are the available options." "Bit Depth — Use the chooser to select the bit depth for the rendered audio: 16, 24, or 32." "Dither Options — If rendering at a bit depth lower than 32-bit, you can select one of the following dithering options: No Dither, Rectangular, Triangular, POW-r 1, POW-r 2, or POW-r 3." "Triangular is selected by default"
- 5.1.3.3, page 129-130: "Encode MP3 — Enable this option to render a CBR 320 kbps MP3 file." "Note that if both the Encode PCM and Encode MP3 options are deactivated, the Export button in the dialog will be grayed out, as it is not possible to render without creating a corresponding audio file."
- 39.2.1, page 990: "Rendering to a file with the same bit depth as the original results in complete phase cancellation."

### Implemented options vs remaining gaps

| Live 12.4 option / area | Status | Milestone for gap |
| ----------------------- | ------ | ----------------- |
| Rendered Track (`Main`, `All Individual Tracks`, `Selected Tracks Only`, individual track) | Implemented (`--track`, `--selected-tracks`) | UI dialog chooser in M14/M40 |
| Render Start & Render Length | Implemented (`--start-bars`/`--start-beats`, `--duration-bars`/`--length-beats`) | Arrangement selection integration & UI in M16/M40 |
| Include Return and Main Effects, Render as Loop, Convert to Mono, Normalize, Sample Rate | Implemented (`--include-returns-main`, `--render-as-loop`, `--mono`, `--normalize`, `--sample-rate`, `--source-sample-rate`) | UI dialog in M14/M40 |
| Create Analysis File | Implemented as LiveCraft `.lcanalysis` JSON sidecar (`--analysis-file`) | Shared sample cache in M11 |
| Encode PCM (`WAV`, `AIFF`, `FLAC`; `16`, `24`, `32` float; `No Dither`, `Rectangular`, `Triangular`, `POW-r 1/2/3`), Encode MP3 (320 kbps CBR) | Implemented (`--no-pcm`, `--format`, `--bits`, `--dither`, `--mp3`) | UI dialog in M14/M40 |
| Video Rendering Options (`Create Video`, `Video Encoder`, `Encoder Settings`) | Gap | M39 (Video) |
| Real-Time Rendering waiting period for External Audio Effect / External Instrument | Gap | M34/M38 |
| Live Clip (`.alc`) drag export | Gap | M10 (Live Projects and file management) |

### Evidence

- `cargo xtask ci` passed (510 passed, 0 failed, 1 ignored `ten_minutes_of_playback_allocate_nothing`), log `.agent-local/logs/2026-11-10_m08-ci.log`.
- `cargo test -p livecraft-engine --no-default-features --features rt-checks` passed (zero audio-thread allocations).
- `cargo xtask oracle` ran cleanly (`0 passed, 4 skipped, 0 failed` due to absent `.agent-local/reference/oracle/*.wav`).
- Analytical oracle self-consistency test `bouncer::tests::oracle_fixtures_render_deterministically_and_match_analytical_linear_math` passed (`<= -110 dBFS` residual across the four committed `.als` sets).

### Criteria

| Criterion | Met | Evidence |
| --------- | --- | -------- |
| Two renders of the same set are byte-identical | Yes | `bouncer::tests::two_offline_renders_are_byte_identical_across_wav_aiff_flac_and_mp3` (plus `launch.rs::a_follow_action_render_is_byte_identical_for_a_stored_seed`) |
| Oracle linear-processing sets null to at or below -90 dBFS against Live's renders | Partly (owner Q12) | Harness in `xtask/src/oracle.rs` (`cargo xtask oracle`), 4 committed sets in `fixtures/oracle/`, Live 12.4 reference WAVs queued in `OWNER-QUEUE.md` `Q12` |
| `livecraft render --help` lists every option from Live's dialog that is implemented; the review lists gaps with milestones | Yes | `cli_tests::render_help_documents_all_live_export_audio_options` and the gaps table above |

### Owner rows

- `Q12`: owner exports the four `fixtures/oracle/*.als` sets in Live 12.4 Suite into `.agent-local/reference/oracle/*.wav` and runs `cargo xtask oracle`. Line stays `[~]` until Q12 arrives.

### Not verified

- Live 12.4 owner reference renders (`.agent-local/reference/oracle/*.wav`) are absent, so Live-vs-LiveCraft null residual numbers await `Q12`.
- 10-minute `long_playback` `--ignored` release stress run was not re-run (only `two_seconds_of_playback_allocate_nothing` ran under `rt-checks`).
