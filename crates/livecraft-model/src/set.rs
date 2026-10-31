use crate::device::{Device, DeviceType};
use crate::scale::ScaleConfig;
use crate::scene::Scene;
use crate::track::{Track, TrackType};
use crate::tuning::TuningSystem;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transport {
    pub bpm: f32,
    pub time_signature: (u8, u8),
    pub is_playing: bool,
    pub is_recording: bool,
    pub playhead_beats: f64,
    pub loop_enabled: bool,
    pub loop_start_beats: f64,
    pub loop_length_beats: f64,
    pub metronome_enabled: bool,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            time_signature: (4, 4),
            is_playing: false,
            is_recording: false,
            playhead_beats: 0.0,
            loop_enabled: false,
            loop_start_beats: 0.0,
            loop_length_beats: 16.0,
            metronome_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: String,
    pub creator: String,
    pub name: String,
    pub transport: Transport,
    #[serde(default)]
    pub scale: ScaleConfig,
    #[serde(default)]
    pub tuning: Option<TuningSystem>,
    pub tracks: Vec<Track>,
    pub return_tracks: Vec<Track>,
    pub master_track: Track,
    pub scenes: Vec<Scene>,
}

impl Project {
    pub fn new_default(name: impl Into<String>) -> Self {
        let num_slots = 8;
        let mut tracks = vec![
            Track::new(1, "1-MIDI", TrackType::Midi, num_slots),
            Track::new(2, "2-MIDI", TrackType::Midi, num_slots),
            Track::new(3, "3-Audio", TrackType::Audio, num_slots),
            Track::new(4, "4-Audio", TrackType::Audio, num_slots),
        ];

        // Give default instrument to Track 1
        tracks[0]
            .device_chain
            .push(Device::new(101, "Operator", DeviceType::Operator));
        // Give Compressor and EQ Eight to Track 3
        tracks[2]
            .device_chain
            .push(Device::new(102, "EQ Eight", DeviceType::EqEight));

        let return_tracks = vec![
            {
                let mut trk = Track::new(5, "A-Reverb", TrackType::Return, num_slots);
                trk.device_chain
                    .push(Device::new(201, "Reverb", DeviceType::Reverb));
                trk
            },
            {
                let mut trk = Track::new(6, "B-Delay", TrackType::Return, num_slots);
                trk.device_chain
                    .push(Device::new(202, "Delay", DeviceType::Delay));
                trk
            },
        ];

        let mut master_track = Track::new(7, "Master", TrackType::Master, 0);
        master_track
            .device_chain
            .push(Device::new(301, "Limiter", DeviceType::Limiter));

        let scenes = (1..=num_slots)
            .map(|i| Scene::new(i as u64, format!("{}", i)))
            .collect();

        Self {
            schema_version: "LiveCraft 1.0 (Ableton Live 12.0 compatible)".to_string(),
            creator: "LiveCraft Engine".to_string(),
            name: name.into(),
            transport: Transport::default(),
            scale: ScaleConfig::default(),
            tuning: None,
            tracks,
            return_tracks,
            master_track,
            scenes,
        }
    }
}
