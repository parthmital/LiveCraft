use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A position or length in quarter-note beats from the song start.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Beats(pub f64);

/// A position or length in sample frames from the song start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Samples(pub i64);

macro_rules! arithmetic {
    ($ty:ident) => {
        impl Add for $ty {
            type Output = $ty;
            fn add(self, rhs: $ty) -> $ty {
                $ty(self.0 + rhs.0)
            }
        }

        impl Sub for $ty {
            type Output = $ty;
            fn sub(self, rhs: $ty) -> $ty {
                $ty(self.0 - rhs.0)
            }
        }

        impl AddAssign for $ty {
            fn add_assign(&mut self, rhs: $ty) {
                self.0 += rhs.0;
            }
        }

        impl SubAssign for $ty {
            fn sub_assign(&mut self, rhs: $ty) {
                self.0 -= rhs.0;
            }
        }
    };
}

arithmetic!(Beats);
arithmetic!(Samples);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_add_and_subtract() {
        assert_eq!(Beats(1.5) + Beats(2.25), Beats(3.75));
        assert_eq!(Beats(4.0) - Beats(1.0), Beats(3.0));
        let mut position = Beats(0.0);
        position += Beats(0.5);
        position -= Beats(0.25);
        assert_eq!(position, Beats(0.25));
    }

    #[test]
    fn samples_add_and_subtract_past_the_song_start() {
        assert_eq!(Samples(44_100) + Samples(22_050), Samples(66_150));
        assert_eq!(Samples(0) - Samples(1), Samples(-1));
        let mut frames = Samples(10);
        frames += Samples(5);
        frames -= Samples(20);
        assert_eq!(frames, Samples(-5));
    }

    #[test]
    fn beats_order_by_position() {
        assert!(Beats(-0.5) < Beats(0.0));
        assert!(Beats(3.0) > Beats(2.999));
        assert_eq!(Beats::default(), Beats(0.0));
    }
}
