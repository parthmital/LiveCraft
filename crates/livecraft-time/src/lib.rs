//! Musical time: positions and lengths in beats and sample frames.
//!
//! Beats are quarter-note beats from the song start, as `.als` stores them.

mod tempo;
mod units;

pub use tempo::{MAX_BPM, MIN_BPM, TempoMap};
pub use units::{Beats, Samples};
