pub mod audio_clip;
pub mod autofilter;
pub mod autopan;
pub mod biquad;
pub mod bouncer;
pub mod chorus;
pub mod compressor;
pub mod delay;
pub mod drumrack;
pub mod engine;
pub mod eq3;
pub mod eq8;
pub mod gate;
pub mod graph;
pub mod limiter;
pub mod operator;
pub mod processor;
pub mod redux;
pub mod reverb;
pub mod saturator;
pub mod simpler;

pub use audio_clip::*;
pub use autofilter::*;
pub use autopan::*;
pub use biquad::*;
pub use chorus::*;
pub use compressor::*;
pub use delay::*;
pub use drumrack::*;
pub use eq3::*;
pub use eq8::*;
pub use gate::*;
pub use limiter::*;
pub use operator::*;
pub use processor::*;
pub use redux::*;
pub use reverb::*;
pub use saturator::*;
pub mod instruments {
    pub use crate::drumrack::*;
    pub use crate::operator::*;
    pub use crate::simpler::*;
}
pub use bouncer::*;
pub use engine::*;
pub use graph::*;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests;
