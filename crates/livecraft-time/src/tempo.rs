use crate::Beats;

/// Lowest tempo the conversions use. Lower tempos are floored to it so that seconds per beat stay finite.
pub const MIN_BPM: f64 = 20.0;

/// Highest tempo Live's Tempo control accepts (manual 7.2.1, page 191). Conversions do not clamp to it.
pub const MAX_BPM: f64 = 999.0;

/// Seconds in one beat at `bpm`, with the tempo floored at `MIN_BPM`.
pub fn seconds_per_beat(bpm: f64) -> f64 {
    60.0 / bpm.max(MIN_BPM)
}

/// Sample frames in one beat at `bpm`, with the tempo floored at `MIN_BPM`.
pub fn samples_per_beat(bpm: f64, sample_rate: f64) -> f64 {
    seconds_per_beat(bpm) * sample_rate
}

/// Beats that pass in one sample frame at `bpm`. The tempo is not floored, so a zero tempo stops the playhead.
pub fn beats_per_sample(bpm: f64, sample_rate: f64) -> f64 {
    (bpm / 60.0) / sample_rate
}

/// Seconds for `beats` at `bpm`, with the tempo floored at `MIN_BPM`.
pub fn seconds_for_beats(beats: f64, bpm: f64) -> f64 {
    (beats / bpm.max(MIN_BPM)) * 60.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    at: Beats,
    bpm: f64,
    ramp_to_next: bool,
}

/// A stretch of song time whose tempo is constant, or changes linearly in beats.
///
/// Segments that run to infinity at either end always have equal start and end tempos.
#[derive(Debug, Clone, Copy)]
struct Segment {
    start: f64,
    end: f64,
    start_bpm: f64,
    end_bpm: f64,
}

impl Segment {
    /// The stretch from breakpoint `from` to breakpoint `to`. It ramps only when `from` asks for it.
    fn between(from: Point, to: Point) -> Self {
        Self {
            start: from.at.0,
            end: to.at.0,
            start_bpm: from.bpm,
            end_bpm: if from.ramp_to_next { to.bpm } else { from.bpm },
        }
    }

    /// Change in bpm per beat. Only meaningful for a finite segment that ramps.
    fn slope(&self) -> f64 {
        (self.end_bpm - self.start_bpm) / (self.end - self.start)
    }

    /// The tempo at `at`, which must lie in the segment.
    fn bpm_at(&self, at: f64) -> f64 {
        if self.start_bpm == self.end_bpm {
            self.start_bpm
        } else {
            self.start_bpm + self.slope() * (at - self.start)
        }
    }

    /// Seconds from `from` to `to`, both inside the segment and in order.
    ///
    /// A ramp is linear in beats, so the integral of 60 / bpm has the closed form (60 / k) ln(b1 / b0),
    /// with k the slope over the stretch. `ln_1p` keeps small ramps accurate.
    fn seconds(&self, from: f64, to: f64) -> f64 {
        let (b0, b1) = (self.bpm_at(from), self.bpm_at(to));
        let span = b1 - b0;
        let length = to - from;
        if span == 0.0 {
            60.0 * length / b0
        } else {
            60.0 * length * (span / b0).ln_1p() / span
        }
    }

    /// The beat reached `elapsed` seconds after the start of this finite segment.
    ///
    /// Inverts `seconds`: with b0 the start tempo and k the slope, the beat is
    /// start + b0 (e^(k elapsed / 60) - 1) / k.
    fn beat_after(&self, elapsed: f64) -> f64 {
        let k = if self.start_bpm == self.end_bpm {
            0.0
        } else {
            self.slope()
        };
        let offset = if k == 0.0 {
            elapsed * self.start_bpm / 60.0
        } else {
            self.start_bpm * (k * elapsed / 60.0).exp_m1() / k
        };
        self.start + offset
    }
}

/// Tempo over song time: breakpoints in beats. Each breakpoint holds its tempo until the next one,
/// or ramps linearly in beats to the next one's tempo.
///
/// The map is never empty. Breakpoint positions are finite, unique, and kept in order.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    points: Vec<Point>,
}

impl TempoMap {
    /// A map with one tempo for the whole song.
    pub fn constant(bpm: f64) -> Self {
        Self {
            points: vec![Point {
                at: Beats(0.0),
                bpm: bpm.max(MIN_BPM),
                ramp_to_next: false,
            }],
        }
    }

    /// Sets the tempo from `at` onward, replacing any breakpoint already at `at`.
    ///
    /// With `ramp_to_next`, the tempo changes linearly in beats up to the next breakpoint;
    /// otherwise it steps at `at`. The last breakpoint holds its tempo forever.
    pub fn insert(&mut self, at: Beats, bpm: f64, ramp_to_next: bool) {
        debug_assert!(at.0.is_finite(), "tempo breakpoints need finite positions");
        // Adding 0.0 turns -0.0 into 0.0, so one position has one key.
        let point = Point {
            at: Beats(at.0 + 0.0),
            bpm: bpm.max(MIN_BPM),
            ramp_to_next,
        };
        match self
            .points
            .binary_search_by(|p| p.at.0.total_cmp(&point.at.0))
        {
            Ok(index) => self.points[index] = point,
            Err(index) => self.points.insert(index, point),
        }
    }

    /// The tempo in effect at `at`.
    pub fn bpm_at(&self, at: Beats) -> f64 {
        let reached = self.points.partition_point(|p| p.at.0 <= at.0);
        if reached == 0 {
            return self.points[0].bpm;
        }
        let previous = self.points[reached - 1];
        match self.points.get(reached) {
            Some(&next) => Segment::between(previous, next).bpm_at(at.0),
            None => previous.bpm,
        }
    }

    /// Seconds from the song start to `at`. Beats before zero count backwards.
    pub fn beats_to_seconds(&self, at: Beats) -> f64 {
        if at.0 >= 0.0 {
            self.seconds_between(0.0, at.0)
        } else {
            -self.seconds_between(at.0, 0.0)
        }
    }

    /// Beats from the song start to `seconds`, the inverse of `beats_to_seconds`.
    pub fn seconds_to_beats(&self, seconds: f64) -> Beats {
        let first = self.points[0];
        let mut reached = self.beats_to_seconds(first.at);
        if seconds < reached {
            // Before the first breakpoint the first tempo holds, as a constant.
            return Beats(first.at.0 + (seconds - reached) * first.bpm / 60.0);
        }
        for pair in self.points.windows(2) {
            let segment = Segment::between(pair[0], pair[1]);
            let length = segment.seconds(segment.start, segment.end);
            if seconds < reached + length {
                return Beats(segment.beat_after(seconds - reached));
            }
            reached += length;
        }
        let last = self.points[self.points.len() - 1];
        Beats(last.at.0 + (seconds - reached) * last.bpm / 60.0)
    }

    /// Sample frames from the song start to `at`, possibly fractional.
    pub fn beats_to_samples(&self, at: Beats, sample_rate: f64) -> f64 {
        self.beats_to_seconds(at) * sample_rate
    }

    /// Beats from the song start to `at` sample frames, the inverse of `beats_to_samples`.
    pub fn samples_to_beats(&self, at: f64, sample_rate: f64) -> Beats {
        self.seconds_to_beats(at / sample_rate)
    }

    /// Seconds between `from` and `to`, with `from <= to`.
    fn seconds_between(&self, from: f64, to: f64) -> f64 {
        self.pieces()
            .map(|segment| {
                let (lo, hi) = (from.max(segment.start), to.min(segment.end));
                if lo < hi {
                    segment.seconds(lo, hi)
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// Segments that cover every beat, in order, from the open start to the open end.
    fn pieces(&self) -> impl Iterator<Item = Segment> + '_ {
        let first = self.points[0];
        let last = self.points[self.points.len() - 1];
        std::iter::once(Segment {
            start: f64::NEG_INFINITY,
            end: first.at.0,
            start_bpm: first.bpm,
            end_bpm: first.bpm,
        })
        .chain(
            self.points
                .windows(2)
                .map(|pair| Segment::between(pair[0], pair[1])),
        )
        .chain(std::iter::once(Segment {
            start: last.at.0,
            end: f64::INFINITY,
            start_bpm: last.bpm,
            end_bpm: last.bpm,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

    /// Simpson's rule over `bpm_at`, independent of the closed form. Only for continuous tempos.
    fn numeric_seconds(map: &TempoMap, from: f64, to: f64) -> f64 {
        const STEPS: usize = 200_000;
        let h = (to - from) / STEPS as f64;
        let f = |x: f64| 60.0 / map.bpm_at(Beats(x));
        let mut sum = f(from) + f(to);
        for i in 1..STEPS {
            let x = from + h * i as f64;
            sum += if i % 2 == 1 { 4.0 * f(x) } else { 2.0 * f(x) };
        }
        sum * h / 3.0
    }

    #[test]
    fn constant_tempo_converts_beats_and_seconds() {
        let map = TempoMap::constant(120.0);
        assert_eq!(map.bpm_at(Beats(10.0)), 120.0);
        assert_eq!(map.beats_to_seconds(Beats(4.0)), 2.0);
        assert_eq!(map.seconds_to_beats(2.0), Beats(4.0));
        assert_eq!(map.beats_to_seconds(Beats(-2.0)), -1.0);
    }

    #[test]
    fn step_change_applies_from_its_beat() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(4.0), 60.0, false);
        assert_eq!(map.bpm_at(Beats(3.5)), 120.0);
        assert_eq!(map.bpm_at(Beats(4.0)), 60.0);
        // Four beats at 120 BPM take 2 s; four more at 60 BPM take 4 s.
        assert_eq!(map.beats_to_seconds(Beats(8.0)), 6.0);
        assert_eq!(map.seconds_to_beats(6.0), Beats(8.0));
    }

    #[test]
    fn insert_replaces_a_breakpoint_at_the_same_beat() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(4.0), 60.0, false);
        map.insert(Beats(4.0), 90.0, false);
        assert_eq!(map.bpm_at(Beats(5.0)), 90.0);
        assert_eq!(map.points.len(), 2);
    }

    #[test]
    fn insert_keeps_breakpoints_in_beat_order() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(8.0), 60.0, false);
        map.insert(Beats(4.0), 90.0, false);
        assert_eq!(map.bpm_at(Beats(5.0)), 90.0);
        assert_eq!(map.bpm_at(Beats(9.0)), 60.0);
    }

    #[test]
    fn tempo_is_floored_at_the_minimum() {
        assert_eq!(TempoMap::constant(0.0).bpm_at(Beats(0.0)), MIN_BPM);
        assert_eq!(TempoMap::constant(f64::NAN).bpm_at(Beats(0.0)), MIN_BPM);
        assert_eq!(TempoMap::constant(0.0).beats_to_seconds(Beats(1.0)), 3.0);
    }

    #[test]
    fn free_conversions_match_the_engine_arithmetic() {
        assert_eq!(seconds_per_beat(120.0), 0.5);
        assert_eq!(seconds_per_beat(0.0), 3.0);
        assert_eq!(samples_per_beat(120.0, 44_100.0), 22_050.0);
        assert_eq!(seconds_for_beats(4.0, 120.0), 2.0);
        assert_eq!(seconds_for_beats(4.0, 0.0), 12.0);
        assert_eq!(beats_per_sample(120.0, 44_100.0), 2.0 / 44_100.0);
        assert_eq!(beats_per_sample(0.0, 44_100.0), 0.0);
    }

    #[test]
    fn sample_conversions_use_the_sample_rate() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(4.0), 60.0, false);
        assert_eq!(map.beats_to_samples(Beats(4.0), 44_100.0), 88_200.0);
        assert_eq!(map.samples_to_beats(88_200.0, 44_100.0), Beats(4.0));
    }

    #[test]
    fn linear_ramp_interpolates_bpm_in_beats() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(0.0), 120.0, true);
        map.insert(Beats(4.0), 60.0, false);
        assert_eq!(map.bpm_at(Beats(1.0)), 105.0);
        assert_eq!(map.bpm_at(Beats(2.0)), 90.0);
        assert_eq!(map.bpm_at(Beats(4.0)), 60.0);
    }

    #[test]
    fn ramp_seconds_match_the_closed_form() {
        // Tempo 120 - 15 b over [0, 4]: the integral of 60 / bpm is 4 ln 2 s.
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(0.0), 120.0, true);
        map.insert(Beats(4.0), 60.0, false);
        assert!((map.beats_to_seconds(Beats(4.0)) - 4.0 * std::f64::consts::LN_2).abs() < 1e-12);
    }

    #[test]
    fn ramp_matches_numeric_integration() {
        let mut fast = TempoMap::constant(20.0);
        fast.insert(Beats(0.0), 20.0, true);
        fast.insert(Beats(1.0), 999.0, false);

        let mut slow = TempoMap::constant(160.0);
        slow.insert(Beats(0.0), 160.0, true);
        slow.insert(Beats(4.0), 40.0, true);
        slow.insert(Beats(6.0), 40.0, false);

        for map in [&fast, &slow] {
            for beats in [0.5, 1.0, 2.5, 4.0, 7.0] {
                let closed = map.beats_to_seconds(Beats(beats));
                let numeric = numeric_seconds(map, 0.0, beats);
                assert!(
                    (closed - numeric).abs() < 1e-9,
                    "{beats} beats: closed {closed}, numeric {numeric}"
                );
            }
        }
    }

    #[test]
    fn ramp_then_step_continues_from_the_ramp_end() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(0.0), 120.0, true);
        map.insert(Beats(4.0), 60.0, false);
        map.insert(Beats(8.0), 90.0, false);
        // The ramp takes 4 ln 2 s, then four beats at 60 BPM take 4 s.
        let expected = 4.0 * std::f64::consts::LN_2 + 4.0;
        assert!((map.beats_to_seconds(Beats(8.0)) - expected).abs() < 1e-12);
        assert!((map.seconds_to_beats(expected) - Beats(8.0)).0.abs() < 1e-12);
    }

    #[test]
    fn ramp_between_equal_tempos_is_constant() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(0.0), 120.0, true);
        map.insert(Beats(4.0), 120.0, false);
        assert_eq!(map.beats_to_seconds(Beats(4.0)), 2.0);
        assert_eq!(map.seconds_to_beats(2.0), Beats(4.0));
    }

    #[test]
    fn seconds_invert_inside_a_ramp() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(0.0), 120.0, true);
        map.insert(Beats(4.0), 60.0, false);
        // Two beats into the ramp: 4 ln(4 / 3) s.
        let seconds = 4.0 * (4.0_f64 / 3.0).ln();
        assert!((map.seconds_to_beats(seconds).0 - 2.0).abs() < 1e-12);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        #[test]
        fn seconds_round_trip_through_step_changes(
            steps in vec((0.0f64..500.0, 20.0f64..999.0), 0..8),
            beats in -50.0f64..600.0,
        ) {
            let mut map = TempoMap::constant(120.0);
            for (at, bpm) in steps {
                map.insert(Beats(at), bpm, false);
            }
            let back = map.seconds_to_beats(map.beats_to_seconds(Beats(beats))).0;
            prop_assert!((back - beats).abs() < 1e-9, "{beats} came back as {back}");
        }

        #[test]
        fn seconds_round_trip_through_ramps_and_steps(
            changes in vec((0.0f64..500.0, 20.0f64..999.0, any::<bool>()), 0..8),
            beats in -50.0f64..600.0,
        ) {
            let mut map = TempoMap::constant(120.0);
            for (at, bpm, ramp) in changes {
                map.insert(Beats(at), bpm, ramp);
            }
            let back = map.seconds_to_beats(map.beats_to_seconds(Beats(beats))).0;
            prop_assert!((back - beats).abs() < 1e-9, "{beats} came back as {back}");
        }
    }
}
