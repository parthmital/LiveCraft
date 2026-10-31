use crate::{Beats, TimeSignatureMap};

/// Relative rounding slack, in grid steps or beats. A position this close past a grid line counts as on it.
const TOLERANCE: f64 = 1e-9;

/// A grid of musical divisions: none, every few bars, or a note value such as `1/16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    None,
    /// Lines every `n` bars, counted from bar 1.
    Bars(u32),
    /// Lines every `num / den` of a whole note, or two thirds of that when `triplet` is set.
    Fraction {
        num: u32,
        den: u32,
        triplet: bool,
    },
}

/// A clip's launch quantization, as the clip Quantize menu offers it (manual 16.4, page 363).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchQuantization {
    None,
    /// The Control Bar's global grid (manual 42.14, page 1025).
    Global,
    Grid(Grid),
}

impl LaunchQuantization {
    /// The menu in Live's order. The manual names only None and Global (page 363); the other
    /// entries come from the M02 milestone's decisions.
    pub const MENU: [LaunchQuantization; 15] = [
        LaunchQuantization::None,
        LaunchQuantization::Grid(Grid::Bars(8)),
        LaunchQuantization::Grid(Grid::Bars(4)),
        LaunchQuantization::Grid(Grid::Bars(2)),
        LaunchQuantization::Grid(Grid::Bars(1)),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 2,
            triplet: false,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 2,
            triplet: true,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 4,
            triplet: false,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 4,
            triplet: true,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 8,
            triplet: false,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 8,
            triplet: true,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 16,
            triplet: false,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 16,
            triplet: true,
        }),
        LaunchQuantization::Grid(Grid::Fraction {
            num: 1,
            den: 32,
            triplet: false,
        }),
        LaunchQuantization::Global,
    ];

    /// Replaces `Global` with the Control Bar's grid. Every other setting is returned unchanged.
    pub fn resolve(self, global: Grid) -> LaunchQuantization {
        match self {
            LaunchQuantization::Global => LaunchQuantization::Grid(global),
            other => other,
        }
    }
}

/// The first grid line at or after `now` for a clip launch, counted from the song start.
///
/// `Global` has no grid of its own, so resolve it with `LaunchQuantization::resolve` first.
/// An unresolved `Global` launches at `now`.
pub fn quantize_launch(now: Beats, q: LaunchQuantization, sig: &TimeSignatureMap) -> Beats {
    match q {
        LaunchQuantization::None | LaunchQuantization::Global => now,
        LaunchQuantization::Grid(grid) => next_grid_line(now, grid, sig),
    }
}

fn next_grid_line(now: Beats, grid: Grid, sig: &TimeSignatureMap) -> Beats {
    match grid {
        Grid::None => now,
        Grid::Bars(bars) => next_bar_line(now, bars, sig),
        Grid::Fraction { num, den, triplet } => {
            if num == 0 || den == 0 {
                return now;
            }
            // A note value is 4 num / den quarter notes. A triplet grid is two thirds of the straight one.
            let (num, den) = (f64::from(num), f64::from(den));
            let step = if triplet {
                8.0 * num / (3.0 * den)
            } else {
                4.0 * num / den
            };
            Beats(((now.0 / step) - TOLERANCE).ceil() * step)
        }
    }
}

/// The first bar line at or after `now`, where bar lines fall on every `bars` bars from bar 1.
fn next_bar_line(now: Beats, bars: u32, sig: &TimeSignatureMap) -> Beats {
    if bars == 0 {
        return now;
    }
    let here = sig.to_bar_beat_sixteenth(now).bar;
    let on_line = (sig.bar_start(here).0 - now.0).abs() < TOLERANCE;
    let candidate = i64::from(here) + i64::from(!on_line);
    let every = i64::from(bars);
    let target = (candidate + every - 1).div_euclid(every) * every;
    match i32::try_from(target) {
        Ok(bar) => sig.bar_start(bar),
        Err(_) => now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Beats = Beats(1.3);

    fn four_four() -> TimeSignatureMap {
        TimeSignatureMap::constant(4, 4).unwrap()
    }

    fn fraction(num: u32, den: u32, triplet: bool) -> Grid {
        Grid::Fraction { num, den, triplet }
    }

    fn assert_beats(actual: Beats, expected: f64) {
        assert!(
            (actual.0 - expected).abs() < 1e-9,
            "expected {expected}, got {}",
            actual.0
        );
    }

    #[test]
    fn menu_lists_live_entries_in_order() {
        let menu = LaunchQuantization::MENU;
        assert_eq!(menu.len(), 15);
        assert_eq!(menu[0], LaunchQuantization::None);
        assert_eq!(menu[1], LaunchQuantization::Grid(Grid::Bars(8)));
        assert_eq!(menu[4], LaunchQuantization::Grid(Grid::Bars(1)));
        assert_eq!(menu[5], LaunchQuantization::Grid(fraction(1, 2, false)));
        assert_eq!(menu[6], LaunchQuantization::Grid(fraction(1, 2, true)));
        assert_eq!(menu[13], LaunchQuantization::Grid(fraction(1, 32, false)));
        assert_eq!(menu[14], LaunchQuantization::Global);
    }

    #[test]
    fn every_menu_value_launches_on_its_next_line_from_1_3() {
        let sig = four_four();
        let expected = [
            1.3,
            32.0,
            16.0,
            8.0,
            4.0,
            2.0,
            4.0 / 3.0,
            2.0,
            4.0 / 3.0,
            1.5,
            4.0 / 3.0,
            1.5,
            4.0 / 3.0,
            1.375,
            1.3,
        ];
        for (q, next) in LaunchQuantization::MENU.into_iter().zip(expected) {
            assert_beats(quantize_launch(NOW, q, &sig), next);
        }
    }

    #[test]
    fn global_resolves_to_the_control_bar_grid() {
        let sig = four_four();
        let resolved = LaunchQuantization::Global.resolve(fraction(1, 4, false));
        assert_eq!(resolved, LaunchQuantization::Grid(fraction(1, 4, false)));
        assert_beats(quantize_launch(NOW, resolved, &sig), 2.0);
        assert_beats(quantize_launch(NOW, LaunchQuantization::Global, &sig), 1.3);
    }

    #[test]
    fn resolve_leaves_other_settings_unchanged() {
        let grid = fraction(1, 4, false);
        assert_eq!(
            LaunchQuantization::None.resolve(grid),
            LaunchQuantization::None
        );
        assert_eq!(
            LaunchQuantization::Grid(Grid::Bars(8)).resolve(grid),
            LaunchQuantization::Grid(Grid::Bars(8))
        );
    }

    #[test]
    fn a_position_on_a_line_launches_at_once() {
        let sig = four_four();
        let on_bar = Beats(4.0);
        assert_beats(
            quantize_launch(on_bar, LaunchQuantization::Grid(Grid::Bars(1)), &sig),
            4.0,
        );
        assert_beats(
            quantize_launch(on_bar, LaunchQuantization::Grid(Grid::Bars(2)), &sig),
            8.0,
        );
        assert_beats(
            quantize_launch(on_bar, LaunchQuantization::Grid(Grid::Bars(4)), &sig),
            16.0,
        );
        assert_beats(
            quantize_launch(
                Beats(2.0),
                LaunchQuantization::Grid(fraction(1, 2, false)),
                &sig,
            ),
            2.0,
        );
        assert_beats(
            quantize_launch(
                Beats(4.0 / 3.0),
                LaunchQuantization::Grid(fraction(1, 2, true)),
                &sig,
            ),
            4.0 / 3.0,
        );
    }

    #[test]
    fn bar_lines_follow_the_time_signature() {
        let six_eight = TimeSignatureMap::constant(6, 8).unwrap();
        assert_beats(
            quantize_launch(
                Beats(0.5),
                LaunchQuantization::Grid(Grid::Bars(1)),
                &six_eight,
            ),
            3.0,
        );

        let mut map = four_four();
        map.insert(2, 3, 4).unwrap();
        // Bar 2 is 3/4 and starts at beat 8, so the next bar lines are at 11 and then 14.
        assert_beats(
            quantize_launch(Beats(9.0), LaunchQuantization::Grid(Grid::Bars(1)), &map),
            11.0,
        );
        assert_beats(
            quantize_launch(Beats(9.0), LaunchQuantization::Grid(Grid::Bars(2)), &map),
            14.0,
        );
    }

    #[test]
    fn degenerate_grids_do_not_quantize() {
        let sig = four_four();
        for grid in [
            Grid::None,
            Grid::Bars(0),
            fraction(0, 4, false),
            fraction(1, 0, true),
        ] {
            assert_beats(
                quantize_launch(NOW, LaunchQuantization::Grid(grid), &sig),
                1.3,
            );
        }
    }
}
