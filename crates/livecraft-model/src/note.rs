use serde::{Deserialize, Serialize};

/// A MIDI note with the properties Live 12 edits in the note editor (manual 10.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiNote {
    pub pitch: u8,               // 0 - 127 (MIDI 60 is C3 in Live's naming)
    pub start_beat: f64,         // Start time in quarter-note beats
    pub duration_beats: f64,     // Duration in quarter-note beats
    pub velocity: f32,           // 1 - 127; fractional values as Live stores them
    pub release_velocity: f32,   // 0 - 127, the Note Off velocity (manual 10.5, page 281)
    pub probability: f32,        // 0.0 - 1.0, the chance that the note plays (page 282)
    pub velocity_deviation: f32, // Range for a random velocity (page 280)
    pub mute: bool,
}

impl MidiNote {
    /// A note with Live's defaults for every property the constructor does not take.
    pub fn new(pitch: u8, start_beat: f64, duration_beats: f64, velocity: u8) -> Self {
        Self {
            pitch,
            start_beat,
            duration_beats,
            velocity: f32::from(velocity),
            release_velocity: 64.0,
            probability: 1.0,
            velocity_deviation: 0.0,
            mute: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiClipData {
    pub notes: Vec<MidiNote>,
}
