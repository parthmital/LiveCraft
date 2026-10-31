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

## Review (2026-10-31)

Evidence for every Outline item and Acceptance Criterion. Commits: `ecc3b5d` (item 1), `9b31f29` (item 2), `7a7a672` (item 3), `e442169` (item 4), `2c8bef0` (item 5), `12e6ef8` (item 6), and the commit that adds this review (item 7).

### Manual quotes

Read with `plans/livecraft-1.0/tools/manual.py` from `docs/ableton-live-12-reference-manual.pdf`. Page renders stayed in `.agent-local/reference/manual/` and are not committed.

- Tempo range, the Decisions check. The milestone pointed to page 35, which names the tempo controls but gives no range. Page 191 (section 7.2.1) states it: "Any tempo can be used, as long as it is within the range allowed by Live's Tempo control (20-999 BPM). Any time signature can be used, provided it has a numerator between 1 and 99 and a denominator with a beat value of 1, 2, 4, 8 or 16."
- Time signature markers, page 172 (section 6.5): "Any time signature with a one- or two-digit numerator and a denominator of 1, 2, 4, 8 or 16 can be used as a value for a marker."
- Time signature markers, page 173: "Time signature markers are not quantized; they may be placed anywhere in the timeline, and their positioning is only constrained by the editing grid."
- Position display, page 169: "The Arrangement Position fields show the play position in bars-beats-sixteenths." Page 189 returns the fields to "1.1.1".
- Count-in, page 420: "The count-in runs from negative bars-beats-sixteenths (beginning at -2.1.1., for example, with a Count-In setting of 2 bars) up to 1.1.1., at which point recording commences."
- Grid, page 180 (section 6.10): the shortcut "toggles triplets mode; this would, for instance, change the grid from eighth notes to eighth note triplets".
- Clip Quantization chooser, page 363 (section 16.4): "To disable clip quantization, choose "None."" and "To use the Control Bar's Global Quantization setting, choose "Global.""
- Global Quantization, page 1025 (section 42.14): Sixteenth-Note, Eighth-Note, Quarter-Note, 1-Bar, and Quantization Off, on Ctrl or Cmd 6, 7, 8, 9, and 0.
- Triplet naming, page 419: "Eighth Note Triplet Quantization", a Record Quantization menu entry.

Launch quantization menu: not verified. Page 363 names only "None" and "Global" for the clip chooser, and its figure shows the closed chooser set to Global. The other 13 entries in the milestone list (8 Bars to 1/32) do not appear in the manual text. Page 1025 supports the 1/16, 1/8, 1/4, and 1 Bar values only as Global shortcuts. Logged as Q03 in `OWNER-QUEUE.md`, and M02 stays `[~]` until it is answered.

### Evidence

- `cargo xtask ci` passes on the final tree. Logs: `.agent-local/logs/2026-10-31_m02-06-ci.log` (commit 6) and the log for this commit.
- `cargo test --workspace`: 89 passed, 0 failed. The baseline was 55. Per crate: livecraft 6 (characterisation 2, cli 4), livecraft-model 9, livecraft-formats 6, livecraft-dsp 1, livecraft-devices 18 (17 unit, 1 integration in `tests/tempo_sync.rs`), livecraft-engine 10, livecraft-time 33, livecraft-ui 2, xtask 4.
- Property tests: four properties in two `proptest!` blocks configured with `ProptestConfig::with_cases(10_000)`: `seconds_round_trip_through_step_changes`, `seconds_round_trip_through_ramps_and_steps`, `bar_beat_sixteenth_round_trip`, and `fields_stay_inside_their_bar`. All pass.
- `cargo test -p livecraft --test characterisation`: 2 passed. `apps/livecraft/tests/characterisation.rs` and its snapshot are unchanged since `e764c73`.
- `grep -rn "60.0 /" crates/livecraft-engine/src`: no matches.

### Criteria

| Criterion                                                                                        | Met | Evidence                                            |
| ------------------------------------------------------------------------------------------------ | --- | --------------------------------------------------- |
| Property tests for round trips pass with 10,000 cases                                            | Yes | Four properties with `with_cases(10_000)`, all pass |
| `grep -rn "60.0 /" crates/livecraft-engine/src` shows no hand-written beat-to-second conversions | Yes | No matches                                          |
| M01 characterisation tests pass                                                                  | Yes | 2 passed, test file and snapshot unchanged          |

All Outline items are done and the three Acceptance Criteria hold. The checklist line stays `[~]` because the launch menu check (Q03) needs owner input.

### Deviations from the milestone

1. Position string: the manual writes `1.1.1` (pages 169, 189, 420), so `format_position` gives `1.1.1` and `format_length` gives `0.0.0`. The milestone example `1. 1. 1` is not used. M12 confirms the format against captures.
2. `TimeSignatureMap::constant` and `insert` return `Result` with `TimeError`, where the Interfaces block showed `-> Self`. Library code must not panic on bad input (AGENTS.md).
3. `TimeSignatureMap::insert` takes a bar index from 0. Changes before the song start are rejected. Bars before bar 0 use the first signature, so the count-in works.
4. `TempoMap` floors tempo at `MIN_BPM` and does not reject values above `MAX_BPM`. Range rejection stays with the UI, which clamps in `crates/livecraft-ui/src/transport.rs`.
5. `BarBeatSixteenth.ticks` is the fraction of a sixteenth left over, from 0.0 to 1.0. The manual does not define a ticks unit in these sections, and the field is not displayed.
6. `Grid::Fraction` carries `triplet: bool`. `LaunchQuantization` is `None`, `Global`, or `Grid(Grid)`, and `resolve(global)` replaces `Global`. `quantize_launch` treats an unresolved `Global` as no quantization, and its doc comment says so.
7. Time signature changes sit at bar boundaries, as the Decisions say. Manual page 173 says markers are not quantized and can sit anywhere. This is not implemented here and should be revisited in M03 or M07.
8. `livecraft-time` depends on the workspace `thiserror` for `TimeError`.
9. `beats_per_sample` is not floored, so a zero tempo still stops the playhead. `seconds_per_beat`, `samples_per_beat`, and `seconds_for_beats` floor at 20 BPM, as the clip and delay code did before. The bouncer had no floor, so a zero tempo gave an infinite render length (reasoned from the code, not run). Only renders below 20 BPM change.
10. Scope beyond the named crates: `livecraft-devices` (delay) and `livecraft-ui` (transport clamps) also use the crate, following the Goal. `livecraft-model` has no beat or second conversions, so it gets no dependency. Its `Transport.bpm` and `time_signature` are plain data in the serialised snapshot, and converting them belongs to M03.
11. The characterisation tests compare renders with a 1e-6 tolerance, not bit for bit. Bit identity of the substitutions follows from unchanged operation order. The delay path is tested at 200,000 tempos against the old f32 expression.

### Open items

- Q03 (owner): confirm the 15-entry clip Quantize menu against installed Live 12.4 Suite. Needed before M02 becomes `[x]`.
- M12: confirm the position string format, `1.1.1` here against the milestone's `1. 1. 1` example.
- M03: convert `Transport.bpm` and `time_signature` to the new types, and decide whether time signature changes move off bar boundaries (manual page 173).
