//! Musical time: positions and lengths in beats and sample frames.
//!
//! Beats are quarter-note beats from the song start, as `.als` stores them.

mod units;

pub use units::{Beats, Samples};
