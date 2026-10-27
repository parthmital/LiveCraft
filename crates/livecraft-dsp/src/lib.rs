pub mod audio_clip;
pub mod bouncer;
pub mod engine;
pub mod graph;

pub use audio_clip::*;
pub use bouncer::*;
pub use engine::*;
pub use graph::*;

pub use livecraft_devices::biquad::*;
pub use livecraft_devices::effects::*;
pub use livecraft_devices::instruments::*;
pub use livecraft_devices::processor::*;
pub use livecraft_devices::{biquad, effects, instruments, processor};

pub mod autofilter {
    pub use livecraft_devices::effects::autofilter::*;
}
pub mod autopan {
    pub use livecraft_devices::effects::autopan::*;
}
pub mod chorus {
    pub use livecraft_devices::effects::chorus::*;
}
pub mod compressor {
    pub use livecraft_devices::effects::compressor::*;
}
pub mod delay {
    pub use livecraft_devices::effects::delay::*;
}
pub mod drumrack {
    pub use livecraft_devices::instruments::drumrack::*;
}
pub mod eq3 {
    pub use livecraft_devices::effects::eq3::*;
}
pub mod eq8 {
    pub use livecraft_devices::effects::eq8::*;
}
pub mod gate {
    pub use livecraft_devices::effects::gate::*;
}
pub mod limiter {
    pub use livecraft_devices::effects::limiter::*;
}
pub mod operator {
    pub use livecraft_devices::instruments::operator::*;
}
pub mod redux {
    pub use livecraft_devices::effects::redux::*;
}
pub mod reverb {
    pub use livecraft_devices::effects::reverb::*;
}
pub mod saturator {
    pub use livecraft_devices::effects::saturator::*;
}
pub mod simpler {
    pub use livecraft_devices::instruments::simpler::*;
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests;
