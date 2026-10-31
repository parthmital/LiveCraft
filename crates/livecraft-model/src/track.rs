use crate::automation::AutomationLane;
use crate::clip::{ArrangementClip, ClipSlot};
use crate::color::ABLETON_PALETTE;
use crate::device::Device;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Audio,
    Midi,
    Return,
    Master,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MonitoringMode {
    In,
    #[default]
    Auto,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: u64,
    pub name: String,
    pub track_type: TrackType,
    pub color: String,
    pub volume_db: f32, // 0.0 dB default, -inf to +6 dB
    pub pan: f32,       // -1.0 (Left) to +1.0 (Right), 0.0 = Center
    pub is_muted: bool,
    pub is_soloed: bool,
    pub is_armed: bool,
    #[serde(default)]
    pub monitoring: MonitoringMode,
    pub sends: Vec<f32>, // Send levels to Return tracks (0.0 to 1.0)
    pub clip_slots: Vec<ClipSlot>,
    #[serde(default)]
    pub arrangement_clips: Vec<ArrangementClip>,
    #[serde(default)]
    pub automation_lanes: Vec<AutomationLane>,
    pub device_chain: Vec<Device>,
}

impl Track {
    pub fn new(id: u64, name: impl Into<String>, track_type: TrackType, num_slots: usize) -> Self {
        let default_color = match track_type {
            TrackType::Audio => ABLETON_PALETTE[10],  // Cyan
            TrackType::Midi => ABLETON_PALETTE[2],    // Amber
            TrackType::Return => ABLETON_PALETTE[14], // Purple
            TrackType::Master => "#888888",
        };

        let clip_slots = (0..num_slots)
            .map(|slot_idx| ClipSlot::empty((id * 1000) + slot_idx as u64))
            .collect();

        Self {
            id,
            name: name.into(),
            track_type,
            color: default_color.to_string(),
            volume_db: 0.0,
            pan: 0.0,
            is_muted: false,
            is_soloed: false,
            is_armed: false,
            monitoring: MonitoringMode::Auto,
            sends: vec![0.0, 0.0], // Sends A and B
            clip_slots,
            arrangement_clips: Vec::new(),
            automation_lanes: Vec::new(),
            device_chain: Vec::new(),
        }
    }
}
