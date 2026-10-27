# M02: Musical time foundation

Purpose: scope, decisions, and acceptance criteria for the `livecraft-time` crate. Read when: expanding or reviewing M02.
Phase: 1. Depends on: M01. Manual: 6.5 Time Signature Changes (172 to 173), 9.1 Tempo (234 to 237), 6.10 Using the Editing Grid (180), 16.4 Clip Launch Quantization (363), 3.1 The Control Bar (35). Parity rows: none directly; every timing feature depends on it.

## Goal

One tested crate owns musical time: positions, tempo and time-signature maps, beat and sample conversion, grid and quantization values, and Live's position formatting.

## Decisions

- `Beats(f64)`: quarter-note beats from the song start, matching `.als`. `Samples(i64)`: sample frames from the song start.
- `TempoMap`: ordered breakpoints `(Beats, bpm: f64, ramp_to_next: bool)`, at least one point; Live allows tempo automation with linear ramps in beats. Conversion beat to seconds integrates exactly for constant and linear segments (closed form for linear tempo: t = integral of 60/bpm(b) db, logarithmic for linear bpm ramps).
- Tempo range 20 to 999 BPM (Live's Control Bar range; confirm on manual page 35 when the milestone starts and record the quote).
- `TimeSignatureMap`: changes `(bar index, numerator 1..99, denominator in {1,2,4,8,16})` at bar boundaries, as in manual 6.5.
- `BarBeatSixteenth { bar: i32, beat: i32, sixteenth: i32, ticks: f64 }` with `format_position()` producing Live's `1. 1. 1` style, one-based, and `format_length()` producing `0. 0. 0` style, zero-based. Exact string formats come from reference captures in M12; until then from the manual's Control Bar description.
- `Grid` (`None`, `Bars(u32)`, `Fraction { num: u32, den: u32 }`, `Triplet` flag) and `LaunchQuantization` with Live's menu entries: None, 8 Bars, 4 Bars, 2 Bars, 1 Bar, 1/2, 1/2T, 1/4, 1/4T, 1/8, 1/8T, 1/16, 1/16T, 1/32, plus Global for clips. The exact list is quoted from manual 16.4 when the milestone starts.
- No dependency on any other workspace crate. `proptest` 1.12.0 as dev-dependency.

## Interfaces

```rust
pub struct TempoMap { /* private */ }
impl TempoMap {
    pub fn constant(bpm: f64) -> Self;
    pub fn insert(&mut self, at: Beats, bpm: f64, ramp_to_next: bool);
    pub fn bpm_at(&self, at: Beats) -> f64;
    pub fn beats_to_seconds(&self, at: Beats) -> f64;
    pub fn seconds_to_beats(&self, seconds: f64) -> Beats;
    pub fn beats_to_samples(&self, at: Beats, sample_rate: f64) -> f64;
    pub fn samples_to_beats(&self, at: f64, sample_rate: f64) -> Beats;
}
pub struct TimeSignatureMap { /* private */ }
impl TimeSignatureMap {
    pub fn constant(numerator: u8, denominator: u8) -> Self;
    pub fn to_bar_beat_sixteenth(&self, at: Beats) -> BarBeatSixteenth;
    pub fn from_bar_beat_sixteenth(&self, pos: BarBeatSixteenth) -> Beats;
    pub fn bar_start(&self, bar: i32) -> Beats;
}
pub fn quantize_launch(now: Beats, q: LaunchQuantization, sig: &TimeSignatureMap) -> Beats; // next boundary at or after now
```

## Task Outline

1. Create the crate with `Beats`, `Samples`, arithmetic, and unit tests; add to `LAYERS`.
2. `TempoMap` with constant and step changes; property test: `seconds_to_beats(beats_to_seconds(b)) == b` within 1e-9.
3. Linear tempo ramps with closed-form integration; tests against numeric integration.
4. `TimeSignatureMap` and bar.beat.sixteenth conversion and formatting; tests for 4/4, 7/8, 6/8, and a change mid-song.
5. `Grid`, `LaunchQuantization`, `quantize_launch`; tests for every menu value.
6. Replace the ad-hoc beat and tempo maths in `livecraft-engine` (`graph.rs` playhead advance) and `livecraft-model` with the new types where it is a pure substitution; characterisation tests from M01 still pass.
7. Milestone verification and docs.

## Acceptance Criteria

- Property tests for round trips pass with 10,000 cases.
- `grep -rn "60.0 /" crates/livecraft-engine/src` shows no remaining hand-written beat-to-second conversions outside `livecraft-time`.
- M01 characterisation tests pass.
