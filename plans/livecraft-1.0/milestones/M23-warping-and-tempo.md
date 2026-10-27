# M23: Warping and tempo

Purpose: scope, decisions, and acceptance criteria for audio warping. Read when: expanding or reviewing M23.
Phase: 4. Depends on: M22. Manual: 9 Audio Clips, Tempo, and Warping (234 to 251: 9.1 tempo including tempo nudge and tap, 9.2 warping with warp markers, transients, auto-warp, saving warp settings, 9.3 warp modes 9.3.1 Beats, 9.3.2 Tones, 9.3.3 Texture, 9.3.4 Re-Pitch, 9.3.5 Complex and Complex Pro), 8.4.1 Warp Controls (215). Parity rows: 9.1 to 9.3.

## Goal

Audio clips warp to the set tempo with Live's warp markers, transient detection, auto-warp, and all six warp modes with their parameters, sounding equivalent within the time-stretch thresholds.

## Decisions

- New crate `livecraft-warp` (depends on dsp; `LAYERS` updated).
- Warp map: piecewise-linear beat-time to sample-time mapping from warp markers, as `.als` stores it (read in M09, preserved).
- Transient detection: spectral flux onset detector with adaptive threshold; parameters tuned against HUMAN oracle files (owner exports Live's detected transients by saving the clip's warp markers in an `.als`; tests compare positions within 5 ms).
- Tempo detection for auto-warp: onset autocorrelation plus comb filter bank; compared with Live's detected tempo on the oracle set.
- Modes: Complex and Complex Pro use `signalsmith-stretch` 0.1.3 (Complex Pro with formant preservation per the manual's Formants and Envelope controls, implemented with the library's tonality limit and formant options; deviations recorded); Beats is transient slicing with Live's Preserve, Transient Loop Mode, and Transient Envelope parameters; Tones and Texture are granular with grain size and flux from the manual; Re-Pitch is variable-rate resampling via `rubato`.
- Processing runs in the engine per clip with preallocated buffers; heavy analysis runs as jobs.

## Task Outline

1. Create `livecraft-warp`; warp map model and tests.
2. Warp markers UI in the Sample Editor (create, move, delete, quantize).
3. Transient detection and HUMAN oracle comparison.
4. Tempo detection and auto-warp; tempo nudge and tap (9.1).
5. Re-Pitch mode.
6. Beats mode with all parameters.
7. Tones and Texture modes.
8. Complex and Complex Pro via `signalsmith-stretch`.
9. Engine integration with no allocation; RAM and streaming sources.
10. Oracle renders for each mode at 3 tempo ratios (HUMAN); comparisons.
11. Milestone verification.

## Acceptance Criteria

- Each warp mode meets the time-stretch thresholds in `QUALITY-GATES.md#audio` on the oracle set.
- Transient positions within 5 ms of Live's for at least 95 percent of onsets on the oracle set.
- Parity rows 9.1 to 9.3 Done with evidence.
