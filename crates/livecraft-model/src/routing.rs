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

/// Where a MIDI track sends its notes (manual 17.3, External MIDI In/Out, page 375).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiOutput {
    /// No external MIDI output.
    #[default]
    None,
    /// An output port of the system, by its position in the list of output ports (`list_midi_ports`),
    /// and a MIDI channel from 0 to 15. The channel is zero-based, as in the MIDI status byte; Live
    /// shows channels 1 to 16.
    External { port: u8, channel: u8 },
}

/// The routing of one track. The default is no input, output to the Main track, and no MIDI output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routing {
    pub audio_from: AudioInput,
    pub audio_to: AudioOutput,
    /// Where a MIDI track sends its notes. The `.als` writer does not write it yet (M09).
    #[serde(default)]
    pub midi_to: MidiOutput,
}

impl Routing {
    /// True when the routing is the default, which the JSON export leaves out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_routing_has_no_midi_output() {
        let routing = Routing::default();
        assert_eq!(routing.midi_to, MidiOutput::None);
        assert!(routing.is_default());
    }

    #[test]
    fn a_midi_output_makes_the_routing_differ_from_the_default() {
        let routing = Routing {
            midi_to: MidiOutput::External {
                port: 1,
                channel: 4,
            },
            ..Routing::default()
        };
        assert!(!routing.is_default());
    }

    #[test]
    fn a_routing_saved_before_the_midi_output_existed_loads_with_none() {
        let mut value = serde_json::to_value(Routing::default()).expect("serialize a routing");
        value
            .as_object_mut()
            .expect("a routing is an object")
            .remove("midi_to");
        let routing: Routing = serde_json::from_value(value).expect("load an older routing");
        assert_eq!(routing.midi_to, MidiOutput::None);
    }

    #[test]
    fn a_midi_output_survives_a_json_round_trip() {
        let routing = Routing {
            midi_to: MidiOutput::External {
                port: 2,
                channel: 15,
            },
            ..Routing::default()
        };
        let json = serde_json::to_string(&routing).expect("serialize a routing");
        let loaded: Routing = serde_json::from_str(&json).expect("load the routing");
        assert_eq!(loaded, routing);
    }
}
