use thiserror::Error;

use crate::Beats;

/// Errors from building a time signature map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TimeError {
    /// The numerator must be 1 to 99 and the denominator 1, 2, 4, 8, or 16 (manual 6.5, page 172).
    #[error("time signature {numerator}/{denominator} is not allowed")]
    InvalidTimeSignature { numerator: u8, denominator: u8 },
    /// Changes start at bar 0 or later.
    #[error("time signature change at bar {0} is before the song start")]
    NegativeBar(i32),
}

/// A time signature from the start of `bar` onward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Change {
    bar: i32,
    numerator: u8,
    denominator: u8,
}

impl Change {
    fn new(bar: i32, numerator: u8, denominator: u8) -> Result<Self, TimeError> {
        if (1..=99).contains(&numerator) && matches!(denominator, 1 | 2 | 4 | 8 | 16) {
            Ok(Self {
                bar,
                numerator,
                denominator,
            })
        } else {
            Err(TimeError::InvalidTimeSignature {
                numerator,
                denominator,
            })
        }
    }

    /// Quarter-note beats in one bar.
    fn bar_beats(&self) -> f64 {
        f64::from(self.numerator) * 4.0 / f64::from(self.denominator)
    }

    /// Sixteenths in one beat. A beat is the denominator's note value, and a sixteenth is a quarter of a quarter note.
    fn sixteenths_per_beat(&self) -> i32 {
        16 / i32::from(self.denominator)
    }

    /// Sixteenths in one bar.
    fn sixteenths_per_bar(&self) -> i64 {
        i64::from(self.numerator) * i64::from(self.sixteenths_per_beat())
    }
}

/// Time signature over song time, as changes at bar boundaries (manual 6.5, page 172).
///
/// The first change always sits at bar 0. Bars before bar 0 use the first change's signature,
/// so the count-in before the song start works.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSignatureMap {
    changes: Vec<Change>,
}

impl Default for TimeSignatureMap {
    /// 4/4 from the song start.
    fn default() -> Self {
        Self {
            changes: vec![Change {
                bar: 0,
                numerator: 4,
                denominator: 4,
            }],
        }
    }
}

impl TimeSignatureMap {
    /// A map with one time signature for the whole song.
    pub fn constant(numerator: u8, denominator: u8) -> Result<Self, TimeError> {
        Ok(Self {
            changes: vec![Change::new(0, numerator, denominator)?],
        })
    }

    /// The time signature in effect at `at` quarter-note beats, as (numerator, denominator).
    pub fn signature_at(&self, at: Beats) -> (u8, u8) {
        let (change, _) = self.change_at(at.0);
        (change.numerator, change.denominator)
    }

    /// Sets the time signature from `bar` onward, replacing a change already at `bar`.
    pub fn insert(&mut self, bar: i32, numerator: u8, denominator: u8) -> Result<(), TimeError> {
        if bar < 0 {
            return Err(TimeError::NegativeBar(bar));
        }
        let change = Change::new(bar, numerator, denominator)?;
        match self.changes.binary_search_by_key(&bar, |c| c.bar) {
            Ok(index) => self.changes[index] = change,
            Err(index) => self.changes.insert(index, change),
        }
        Ok(())
    }

    /// The first quarter-note position of `bar`, and the change in effect there.
    fn change_for_bar(&self, bar: i32) -> (Change, f64) {
        let (change, start) = self.last_change(|change, _| change.bar <= bar);
        (
            change,
            start + f64::from(bar - change.bar) * change.bar_beats(),
        )
    }

    /// The change in effect at `at` quarter-note beats, and the position where its first bar starts.
    fn change_at(&self, at: f64) -> (Change, f64) {
        self.last_change(|_, start| start <= at)
    }

    /// The last change that `reached` accepts, with the position where its first bar starts.
    /// The first change is the fallback, so every position has a change.
    fn last_change(&self, reached: impl Fn(&Change, f64) -> bool) -> (Change, f64) {
        let mut found = (self.changes[0], 0.0);
        for next in &self.changes[1..] {
            let (current, start) = found;
            let next_start = start + f64::from(next.bar - current.bar) * current.bar_beats();
            if !reached(next, next_start) {
                break;
            }
            found = (*next, next_start);
        }
        found
    }

    /// The first quarter-note position of `bar`.
    pub fn bar_start(&self, bar: i32) -> Beats {
        Beats(self.change_for_bar(bar).1)
    }

    /// Splits a position into bar, beat, and sixteenth, all zero-based.
    pub fn to_bar_beat_sixteenth(&self, at: Beats) -> BarBeatSixteenth {
        let (change, start) = self.change_at(at.0);
        let bars = ((at.0 - start) / change.bar_beats()).floor();
        let sixteenths_in_bar = (at.0 - start - bars * change.bar_beats()) * 4.0;
        let whole = sixteenths_in_bar
            .floor()
            .clamp(0.0, (change.sixteenths_per_bar() - 1) as f64) as i64;
        let per_beat = i64::from(change.sixteenths_per_beat());
        BarBeatSixteenth {
            bar: change.bar.saturating_add(bars as i32),
            beat: (whole / per_beat) as i32,
            sixteenth: (whole % per_beat) as i32,
            ticks: (sixteenths_in_bar - whole as f64).clamp(0.0, 1.0),
        }
    }

    /// The quarter-note position of a bar, beat, and sixteenth, all zero-based.
    pub fn from_bar_beat_sixteenth(&self, pos: BarBeatSixteenth) -> Beats {
        let (change, start) = self.change_for_bar(pos.bar);
        let sixteenths = f64::from(pos.beat) * f64::from(change.sixteenths_per_beat())
            + f64::from(pos.sixteenth)
            + pos.ticks;
        Beats(start + sixteenths / 4.0)
    }
}

/// A position as bar, beat, and sixteenth, all zero-based, with `ticks` as the part of a sixteenth
/// that is left over, from 0.0 up to 1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarBeatSixteenth {
    pub bar: i32,
    pub beat: i32,
    pub sixteenth: i32,
    pub ticks: f64,
}

impl BarBeatSixteenth {
    /// Live's position display, one-based, for example `1.1.1` at the song start.
    /// There is no bar 0: the bar before bar 1 is `-1` and the count-in starts at `-2.1.1` (manual 19, page 420).
    pub fn format_position(&self) -> String {
        let bar = if self.bar >= 0 {
            self.bar.saturating_add(1)
        } else {
            self.bar
        };
        format!(
            "{bar}.{}.{}",
            self.beat.saturating_add(1),
            self.sixteenth.saturating_add(1)
        )
    }

    /// Live's length display, zero-based, for example `0.0.0`.
    pub fn format_length(&self) -> String {
        format!("{}.{}.{}", self.bar, self.beat, self.sixteenth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::sample::select;

    #[test]
    fn signature_at_follows_a_mid_song_change() {
        let mut map = TimeSignatureMap::constant(4, 4).expect("4/4 is valid");
        map.insert(2, 7, 8).expect("7/8 is valid");
        assert_eq!(map.signature_at(Beats(0.0)), (4, 4));
        assert_eq!(map.signature_at(Beats(7.9)), (4, 4));
        assert_eq!(map.signature_at(Beats(8.0)), (7, 8));
    }

    fn position(bar: i32, beat: i32, sixteenth: i32) -> BarBeatSixteenth {
        BarBeatSixteenth {
            bar,
            beat,
            sixteenth,
            ticks: 0.0,
        }
    }

    #[test]
    fn four_four_positions_and_lengths() {
        let map = TimeSignatureMap::constant(4, 4).unwrap();
        assert_eq!(map.to_bar_beat_sixteenth(Beats(0.0)), position(0, 0, 0));
        assert_eq!(map.to_bar_beat_sixteenth(Beats(4.0)), position(1, 0, 0));
        assert_eq!(map.to_bar_beat_sixteenth(Beats(5.25)), position(1, 1, 1));
        assert_eq!(map.from_bar_beat_sixteenth(position(1, 1, 1)), Beats(5.25));
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(0.0)).format_position(),
            "1.1.1"
        );
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(5.25)).format_position(),
            "2.2.2"
        );
        assert_eq!(position(0, 0, 0).format_length(), "0.0.0");
        assert_eq!(position(1, 2, 3).format_length(), "1.2.3");
    }

    #[test]
    fn seven_eight_has_seven_eighth_note_beats() {
        let map = TimeSignatureMap::constant(7, 8).unwrap();
        assert_eq!(map.to_bar_beat_sixteenth(Beats(0.5)), position(0, 1, 0));
        assert_eq!(map.to_bar_beat_sixteenth(Beats(1.0)), position(0, 2, 0));
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(1.0)).format_position(),
            "1.3.1"
        );
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(1.25)).format_position(),
            "1.3.2"
        );
        // A 7/8 bar lasts 3.5 quarter notes.
        assert_eq!(map.to_bar_beat_sixteenth(Beats(3.5)), position(1, 0, 0));
        assert_eq!(map.from_bar_beat_sixteenth(position(1, 0, 0)), Beats(3.5));
    }

    #[test]
    fn six_eight_has_six_eighth_note_beats() {
        let map = TimeSignatureMap::constant(6, 8).unwrap();
        assert_eq!(map.to_bar_beat_sixteenth(Beats(0.5)), position(0, 1, 0));
        assert_eq!(map.to_bar_beat_sixteenth(Beats(0.75)), position(0, 1, 1));
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(1.5)).format_position(),
            "1.4.1"
        );
        // A 6/8 bar lasts 3 quarter notes.
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(3.0)).format_position(),
            "2.1.1"
        );
        assert_eq!(map.from_bar_beat_sixteenth(position(1, 0, 0)), Beats(3.0));
    }

    #[test]
    fn meter_change_mid_song_moves_the_bar_lengths() {
        let mut map = TimeSignatureMap::constant(4, 4).unwrap();
        map.insert(2, 3, 4).unwrap();
        // Bars 0 and 1 take 8 quarter notes, so bar 2 starts at beat 8 and is 3/4 long.
        assert_eq!(map.bar_start(2), Beats(8.0));
        assert_eq!(map.bar_start(3), Beats(11.0));
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(8.0)).format_position(),
            "3.1.1"
        );
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(10.5)).format_position(),
            "3.3.3"
        );
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(11.0)).format_position(),
            "4.1.1"
        );
        assert_eq!(map.from_bar_beat_sixteenth(position(2, 0, 0)), Beats(8.0));
    }

    #[test]
    fn bars_before_the_song_start_count_down_without_a_zero() {
        let map = TimeSignatureMap::constant(4, 4).unwrap();
        assert_eq!(map.bar_start(-1), Beats(-4.0));
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(-4.0)).format_position(),
            "-1.1.1"
        );
        assert_eq!(
            map.to_bar_beat_sixteenth(Beats(-8.0)).format_position(),
            "-2.1.1"
        );
    }

    #[test]
    fn rejects_signatures_outside_live_limits() {
        assert_eq!(
            TimeSignatureMap::constant(0, 4),
            Err(TimeError::InvalidTimeSignature {
                numerator: 0,
                denominator: 4
            })
        );
        assert!(TimeSignatureMap::constant(100, 4).is_err());
        assert!(TimeSignatureMap::constant(4, 3).is_err());
        assert!(TimeSignatureMap::constant(4, 32).is_err());
        assert!(TimeSignatureMap::constant(99, 16).is_ok());
        let mut map = TimeSignatureMap::constant(4, 4).unwrap();
        assert_eq!(map.insert(-1, 4, 4), Err(TimeError::NegativeBar(-1)));
        assert!(map.insert(4, 4, 5).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        #[test]
        fn bar_beat_sixteenth_round_trip(
            changes in vec_of_changes(),
            beats in -20.0f64..300.0,
        ) {
            let mut map = TimeSignatureMap::constant(4, 4).unwrap();
            for (bar, numerator, denominator) in changes {
                map.insert(bar, numerator, denominator).unwrap();
            }
            let pos = map.to_bar_beat_sixteenth(Beats(beats));
            let back = map.from_bar_beat_sixteenth(pos).0;
            prop_assert!((back - beats).abs() < 1e-9, "{beats} came back as {back} from {pos:?}");
        }

        #[test]
        fn fields_stay_inside_their_bar(
            changes in vec_of_changes(),
            beats in -20.0f64..300.0,
        ) {
            let mut map = TimeSignatureMap::constant(4, 4).unwrap();
            for (bar, numerator, denominator) in changes {
                map.insert(bar, numerator, denominator).unwrap();
            }
            let pos = map.to_bar_beat_sixteenth(Beats(beats));
            let (change, _) = map.change_for_bar(pos.bar);
            prop_assert!(pos.beat >= 0 && pos.beat < i32::from(change.numerator));
            prop_assert!(pos.sixteenth >= 0 && pos.sixteenth < change.sixteenths_per_beat());
            prop_assert!((0.0..=1.0).contains(&pos.ticks));
        }
    }

    fn vec_of_changes() -> impl Strategy<Value = Vec<(i32, u8, u8)>> {
        proptest::collection::vec((0i32..40, 1u8..=99, select(vec![1u8, 2, 4, 8, 16])), 0..6)
    }
}
