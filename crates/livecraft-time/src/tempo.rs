use crate::Beats;

/// Lowest tempo the conversions use. Lower tempos are floored to it so that seconds per beat stay finite.
pub const MIN_BPM: f64 = 20.0;

/// Highest tempo Live's Tempo control accepts (manual 7.2.1, page 191). Conversions do not clamp to it.
pub const MAX_BPM: f64 = 999.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    at: Beats,
    bpm: f64,
}

/// Tempo over song time: breakpoints in beats, each holding its tempo until the next one.
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
            }],
        }
    }

    /// Sets the tempo from `at` onward, replacing any breakpoint already at `at`.
    pub fn insert(&mut self, at: Beats, bpm: f64) {
        debug_assert!(at.0.is_finite(), "tempo breakpoints need finite positions");
        // Adding 0.0 turns -0.0 into 0.0, so one position has one key.
        let point = Point {
            at: Beats(at.0 + 0.0),
            bpm: bpm.max(MIN_BPM),
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
        self.points[reached.saturating_sub(1)].bpm
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
            return Beats(first.at.0 + (seconds - reached) * first.bpm / 60.0);
        }
        for pair in self.points.windows(2) {
            let (step_start, step_end) = (pair[0], pair[1]);
            let step_seconds = (step_end.at.0 - step_start.at.0) * 60.0 / step_start.bpm;
            if seconds < reached + step_seconds {
                return Beats(step_start.at.0 + (seconds - reached) * step_start.bpm / 60.0);
            }
            reached += step_seconds;
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
            .map(|(start, end, bpm)| {
                let (lo, hi) = (from.max(start), to.min(end));
                if lo < hi { (hi - lo) * 60.0 / bpm } else { 0.0 }
            })
            .sum()
    }

    /// Constant-tempo pieces that cover every beat, as (start, end, bpm).
    fn pieces(&self) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
        let first = self.points[0];
        let last = self.points[self.points.len() - 1];
        std::iter::once((f64::NEG_INFINITY, first.at.0, first.bpm))
            .chain(
                self.points
                    .windows(2)
                    .map(|pair| (pair[0].at.0, pair[1].at.0, pair[0].bpm)),
            )
            .chain(std::iter::once((last.at.0, f64::INFINITY, last.bpm)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

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
        map.insert(Beats(4.0), 60.0);
        assert_eq!(map.bpm_at(Beats(3.5)), 120.0);
        assert_eq!(map.bpm_at(Beats(4.0)), 60.0);
        // Four beats at 120 BPM take 2 s; four more at 60 BPM take 4 s.
        assert_eq!(map.beats_to_seconds(Beats(8.0)), 6.0);
        assert_eq!(map.seconds_to_beats(6.0), Beats(8.0));
    }

    #[test]
    fn insert_replaces_a_breakpoint_at_the_same_beat() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(4.0), 60.0);
        map.insert(Beats(4.0), 90.0);
        assert_eq!(map.bpm_at(Beats(5.0)), 90.0);
        assert_eq!(map.points.len(), 2);
    }

    #[test]
    fn insert_keeps_breakpoints_in_beat_order() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(8.0), 60.0);
        map.insert(Beats(4.0), 90.0);
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
    fn sample_conversions_use_the_sample_rate() {
        let mut map = TempoMap::constant(120.0);
        map.insert(Beats(4.0), 60.0);
        assert_eq!(map.beats_to_samples(Beats(4.0), 44_100.0), 88_200.0);
        assert_eq!(map.samples_to_beats(88_200.0, 44_100.0), Beats(4.0));
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
                map.insert(Beats(at), bpm);
            }
            let back = map.seconds_to_beats(map.beats_to_seconds(Beats(beats))).0;
            prop_assert!((back - beats).abs() < 1e-9, "{beats} came back as {back}");
        }
    }
}
