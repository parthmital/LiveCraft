use crate::automation::AutomationLane;
use crate::color::ABLETON_PALETTE;
use crate::note::MidiClipData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarpMode {
    Beats,
    Tones,
    Texture,
    RePitch,
    Complex,
    ComplexPro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMarker {
    pub sample_time_sec: f64,
    pub beat_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClipData {
    pub file_path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub warp_mode: WarpMode,
    pub is_warped: bool,
    pub original_bpm: f64,
    pub warp_markers: Vec<WarpMarker>,
    pub gain_db: f32,
    pub pitch_cents: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipPayload {
    Midi(MidiClipData),
    Audio(AudioClipData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub length_beats: f64,
    pub loop_start_beat: f64,
    pub loop_end_beat: f64,
    pub is_looping: bool,
    pub start_marker_beat: f64,
    pub end_marker_beat: f64,
    pub payload: ClipPayload,
    #[serde(default)]
    pub automation_lanes: Vec<AutomationLane>,
}

impl Clip {
    pub fn new_midi(id: u64, name: impl Into<String>, length_beats: f64) -> Self {
        Self {
            id,
            name: name.into(),
            color: ABLETON_PALETTE[2].to_string(), // Amber/Orange
            length_beats,
            loop_start_beat: 0.0,
            loop_end_beat: length_beats,
            is_looping: true,
            start_marker_beat: 0.0,
            end_marker_beat: length_beats,
            payload: ClipPayload::Midi(MidiClipData { notes: Vec::new() }),
            automation_lanes: Vec::new(),
        }
    }

    pub fn new_audio(
        id: u64,
        name: impl Into<String>,
        file_path: impl Into<String>,
        length_beats: f64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            color: ABLETON_PALETTE[10].to_string(), // Cyan
            length_beats,
            loop_start_beat: 0.0,
            loop_end_beat: length_beats,
            is_looping: true,
            start_marker_beat: 0.0,
            end_marker_beat: length_beats,
            payload: ClipPayload::Audio(AudioClipData {
                file_path: file_path.into(),
                sample_rate: 44100,
                channels: 2,
                warp_mode: WarpMode::Beats,
                is_warped: true,
                original_bpm: 120.0,
                warp_markers: Vec::new(),
                gain_db: 0.0,
                pitch_cents: 0,
            }),
            automation_lanes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSlot {
    pub id: u64,
    pub clip: Option<Clip>,
    pub has_stop_button: bool,
}

impl ClipSlot {
    pub fn empty(id: u64) -> Self {
        Self {
            id,
            clip: None,
            has_stop_button: true,
        }
    }

    pub fn with_clip(id: u64, clip: Clip) -> Self {
        Self {
            id,
            clip: Some(clip),
            has_stop_button: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementClip {
    pub id: u64,
    pub clip: Clip,
    pub start_beat: f64,
    pub length_beats: f64,
}

impl ArrangementClip {
    pub fn new(id: u64, clip: Clip, start_beat: f64, length_beats: f64) -> Self {
        Self {
            id,
            clip,
            start_beat,
            length_beats,
        }
    }

    pub fn split_at(&self, split_beat: f64, new_id: u64) -> Option<(Self, Self)> {
        if split_beat <= self.start_beat || split_beat >= self.start_beat + self.length_beats {
            return None;
        }
        let first_len = split_beat - self.start_beat;
        let second_len = self.length_beats - first_len;

        let mut first = self.clone();
        first.length_beats = first_len;
        first.clip.length_beats = first_len;
        first.clip.end_marker_beat = first.clip.start_marker_beat + first_len;

        let mut second = self.clone();
        second.id = new_id;
        second.clip.id = new_id;
        second.start_beat = split_beat;
        second.length_beats = second_len;
        second.clip.length_beats = second_len;
        second.clip.start_marker_beat += first_len;
        second.clip.end_marker_beat = second.clip.start_marker_beat + second_len;

        // Adjust internal markers & payload
        if let (ClipPayload::Midi(m1), ClipPayload::Midi(m2)) =
            (&mut first.clip.payload, &mut second.clip.payload)
        {
            let rel_split = first_len;
            m1.notes.retain(|n| n.start_beat < rel_split);
            for n in &mut m1.notes {
                if n.start_beat + n.duration_beats > rel_split {
                    n.duration_beats = (rel_split - n.start_beat).max(0.01);
                }
            }
            m2.notes
                .retain(|n| n.start_beat + n.duration_beats > rel_split);
            for n in &mut m2.notes {
                if n.start_beat < rel_split {
                    let cut = rel_split - n.start_beat;
                    n.duration_beats = (n.duration_beats - cut).max(0.01);
                    n.start_beat = 0.0;
                } else {
                    n.start_beat -= rel_split;
                }
            }
        }
        Some((first, second))
    }

    pub fn duplicate(&self, new_id: u64, offset_beats: f64) -> Self {
        let mut dup = self.clone();
        dup.id = new_id;
        dup.clip.id = new_id;
        dup.start_beat += offset_beats;
        dup
    }

    pub fn trim_start(&mut self, delta_beats: f64) {
        let actual_delta = delta_beats.min(self.length_beats - 0.01);
        self.start_beat += actual_delta;
        self.length_beats -= actual_delta;
        self.clip.length_beats = self.length_beats;
        self.clip.start_marker_beat += actual_delta;
        if let ClipPayload::Midi(ref mut mdata) = self.clip.payload {
            if actual_delta > 0.0 {
                mdata
                    .notes
                    .retain(|n| n.start_beat + n.duration_beats > actual_delta);
                for n in &mut mdata.notes {
                    if n.start_beat < actual_delta {
                        let cut = actual_delta - n.start_beat;
                        n.duration_beats = (n.duration_beats - cut).max(0.01);
                        n.start_beat = 0.0;
                    } else {
                        n.start_beat -= actual_delta;
                    }
                }
            } else {
                for n in &mut mdata.notes {
                    n.start_beat -= actual_delta;
                }
            }
        }
    }

    pub fn trim_end(&mut self, delta_beats: f64) {
        let new_len = (self.length_beats + delta_beats).max(0.01);
        self.length_beats = new_len;
        self.clip.length_beats = new_len;
        self.clip.end_marker_beat = self.clip.start_marker_beat + new_len;
        if let ClipPayload::Midi(ref mut mdata) = self.clip.payload {
            mdata.notes.retain(|n| n.start_beat < new_len);
            for n in &mut mdata.notes {
                if n.start_beat + n.duration_beats > new_len {
                    n.duration_beats = (new_len - n.start_beat).max(0.01);
                }
            }
        }
    }
}
