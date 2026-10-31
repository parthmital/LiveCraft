//! Musical time: positions and lengths in beats and sample frames.
//!
//! Beats are quarter-note beats from the song start, as `.als` stores them.

mod grid;
mod meter;
mod tempo;
mod units;

pub use grid::{Grid, LaunchQuantization, quantize_launch};
pub use meter::{BarBeatSixteenth, TimeError, TimeSignatureMap};
pub use tempo::{
    MAX_BPM, MIN_BPM, TempoMap, beats_per_sample, samples_per_beat, seconds_for_beats,
    seconds_per_beat,
};
pub use units::{Beats, Samples};
