//! What the audio thread publishes for the session and the GUI (M05 item 9, TARGET-ARCHITECTURE
//! "Engine Decisions").
//!
//! The audio thread writes one `EngineSnapshot` per driver callback into a `triple_buffer`. The writer
//! never waits for the reader, and the reader always gets the newest complete snapshot. The snapshot
//! holds fixed-size arrays only, so writing it neither allocates nor locks.

use crate::graph::TrackMeter;

/// Most nodes (tracks, returns, and Main) a snapshot holds. `EngineHandle::submit_graph` refuses a
/// graph with more.
pub const MAX_NODES: usize = 256;

/// The transport at the end of the last published block.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Transport {
    pub playing: bool,
    /// The song position in beats.
    pub playhead_beats: f64,
    pub bpm: f32,
}

/// What the clip lane of a node is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ClipState {
    /// The launched session slot, if any.
    pub active_slot: Option<usize>,
    /// True when a clip played in the last block.
    pub playing: bool,
}

/// The level and clip state of one node.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeSnapshot {
    pub meter: TrackMeter,
    pub clip: ClipState,
}

/// One block's view of the engine.
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    pub transport: Transport,
    /// The nodes of the running graph in node order (tracks, returns, then Main). Only the first
    /// `node_count` entries are meaningful.
    pub nodes: [NodeSnapshot; MAX_NODES],
    pub node_count: usize,
    /// The time the last block took, as a share of its buffer period. 1.0 means the whole period.
    pub cpu_load: f32,
}

impl Default for EngineSnapshot {
    fn default() -> Self {
        Self {
            transport: Transport::default(),
            nodes: std::array::from_fn(|_| NodeSnapshot::default()),
            node_count: 0,
            cpu_load: 0.0,
        }
    }
}

/// The share of a buffer period that rendering `frames` frames took.
pub fn cpu_load(elapsed_seconds: f32, frames: usize, sample_rate: f32) -> f32 {
    if frames == 0 || sample_rate <= 0.0 {
        return 0.0;
    }
    elapsed_seconds / (frames as f32 / sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_load_is_the_time_spent_per_buffer_period() {
        // 5 ms of work for a 441-frame buffer at 44.1 kHz, whose period is 10 ms.
        assert!((cpu_load(0.005, 441, 44100.0) - 0.5).abs() < 1e-6);
        assert_eq!(cpu_load(0.005, 0, 44100.0), 0.0);
    }
}
