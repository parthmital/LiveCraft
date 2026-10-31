use crate::automation::AutomationLane;
use crate::clip::{ArrangementClip, ClipSlot};
use crate::color::ABLETON_PALETTE;
use crate::device::Device;
use crate::ids::{LaneId, TrackId};
use crate::routing::Routing;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Audio,
    Midi,
    /// Sums the tracks inside it. Group tracks have mixer controls and effects, but no clips (manual 18.3, page 400).
    Group,
    Return,
    /// The Main track (manual 18.4, page 402). Serialised as `Master` until the owner decides the export name (OWNER-QUEUE Q04).
    #[serde(rename = "Master")]
    Main,
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
    pub id: TrackId,
    pub name: String,
    pub track_type: TrackType,
    /// The group track that contains this track. Groups nest, so a group can have a parent group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<TrackId>,
    /// Where the audio comes from and goes to (manual 17). Left out of the JSON export while default.
    #[serde(default, skip_serializing_if = "Routing::is_default")]
    pub routing: Routing,
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
    pub fn new(
        id: TrackId,
        name: impl Into<String>,
        track_type: TrackType,
        num_slots: usize,
    ) -> Self {
        let default_color = match track_type {
            TrackType::Audio => ABLETON_PALETTE[10],  // Cyan
            TrackType::Midi => ABLETON_PALETTE[2],    // Amber
            TrackType::Return => ABLETON_PALETTE[14], // Purple
            TrackType::Group | TrackType::Main => "#888888",
        };

        // Group tracks hold no clips (manual 18.3, page 400). Return and Main tracks cannot play
        // clips (manual 18.4, page 402). None of them gets clip slots, whatever the caller asks for.
        let num_slots = if matches!(
            track_type,
            TrackType::Group | TrackType::Return | TrackType::Main
        ) {
            0
        } else {
            num_slots
        };
        let clip_slots = (0..num_slots)
            .map(|slot_idx| ClipSlot::empty((id.0 * 1000) + slot_idx as u64))
            .collect();

        Self {
            id,
            name: name.into(),
            track_type,
            parent: None,
            routing: Routing::default(),
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

    pub fn lane_mut(&mut self, id: LaneId) -> Option<&mut AutomationLane> {
        self.automation_lanes.iter_mut().find(|lane| lane.id == id)
    }
}
