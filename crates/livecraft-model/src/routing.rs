//! Routing descriptions: where an audio track's signal comes from and where it goes (manual 17).
//!
//! These types only describe routing. No audio flows through them until the engine reads them (M20).

use crate::ids::TrackId;
use serde::{Deserialize, Serialize};

/// The point where another track's signal is tapped (manual 17.5.1, page 381).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapPoint {
    PreFx,
    PostFx,
    PostMixer,
}

/// Where an audio track receives its signal (manual 17.2, page 375; 17.4, page 380; 17.5, page 381).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioInput {
    /// No input.
    #[default]
    None,
    /// An audio interface input channel, chosen with "Ext. In" (page 375).
    ExternalIn { channel: u16 },
    /// The Main output, routed to the track for resampling (page 380).
    Resampling,
    /// Another track's signal, tapped at one point (page 381).
    Track { track: TrackId, tap: TapPoint },
}

/// Where an audio track sends its signal (manual 17.5, page 381).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioOutput {
    #[default]
    Main,
    Track(TrackId),
}

/// The audio routing of one track. The default is no input and output to the Main track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routing {
    pub audio_from: AudioInput,
    pub audio_to: AudioOutput,
}

impl Routing {
    /// True when the routing is the default, which the JSON export leaves out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
