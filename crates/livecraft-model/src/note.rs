use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiNote {
    pub pitch: u8,           // 0 - 127 (60 = Middle C)
    pub start_beat: f64,     // Start time in beats (quarter notes)
    pub duration_beats: f64, // Duration in beats
    pub velocity: u8,        // 0 - 127
    pub muted: bool,
}

impl MidiNote {
    pub fn new(pitch: u8, start_beat: f64, duration_beats: f64, velocity: u8) -> Self {
        Self {
            pitch,
            start_beat,
            duration_beats,
            velocity,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiClipData {
    pub notes: Vec<MidiNote>,
}
