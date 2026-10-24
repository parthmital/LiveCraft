use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standard Ableton Live 70-color palette (hex RGB values)
pub const ABLETON_PALETTE: &[&str] = &[
    "#ff4848", "#ff764c", "#ffa844", "#ffd03c", "#faff40", "#b9ff40", "#7bff40", "#40ff63",
    "#40ffa7", "#40ffec", "#40d2ff", "#408eff", "#4c5eff", "#814cff", "#b94cff", "#ff4ce6",
    "#ff4c99", "#d83838", "#d86038", "#d88a38", "#d8aa38", "#d0d838", "#93d838", "#61d838",
    "#38d855", "#38d88e", "#38d8c8", "#38aad8", "#3870d8", "#3d38d8", "#6638d8", "#9338d8",
    "#d838b9", "#d8387a", "#aa2828", "#aa4928", "#aa6c28", "#aa8528", "#a4aa28", "#74aa28",
    "#4daa28", "#28aa3f", "#28aa6f", "#28aa9c", "#2885aa", "#2857aa", "#2d28aa", "#4f28aa",
    "#7428aa", "#aa2892", "#aa285e", "#761818", "#763118", "#764918", "#765c18", "#717618",
    "#4f7618", "#337618", "#187629", "#18764c", "#18766c", "#185c76", "#183b76", "#1e1876",
    "#361876", "#4f1876", "#761864", "#76183f", "#ffffff", "#000000",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Audio,
    Midi,
    Return,
    Master,
}

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
pub struct MidiClipData {
    pub notes: Vec<MidiNote>,
}

impl MidiClipData {
    pub fn quantize(&mut self, grid_division: f64) {
        if grid_division <= 0.0 {
            return;
        }
        for note in &mut self.notes {
            note.start_beat = (note.start_beat / grid_division).round() * grid_division;
            note.duration_beats =
                ((note.duration_beats / grid_division).round() * grid_division).max(grid_division);
        }
    }

    pub fn transpose(&mut self, semitones: i8, scale: Option<&ScaleConfig>) {
        for note in &mut self.notes {
            let mut new_pitch = (note.pitch as i16 + semitones as i16).clamp(0, 127) as u8;
            if let Some(sc) = scale {
                if sc.is_active {
                    new_pitch = sc.snap_to_scale(new_pitch);
                }
            }
            note.pitch = new_pitch;
        }
    }

    pub fn strum(&mut self, spread_beats: f64, up: bool) {
        if self.notes.len() < 2 {
            return;
        }
        self.notes.sort_by(|a, b| {
            a.start_beat
                .partial_cmp(&b.start_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.pitch.cmp(&b.pitch))
        });

        let threshold = 0.03125;
        let mut group_start = 0;
        while group_start < self.notes.len() {
            let base_time = self.notes[group_start].start_beat;
            let mut group_end = group_start + 1;
            while group_end < self.notes.len()
                && (self.notes[group_end].start_beat - base_time).abs() < threshold
            {
                group_end += 1;
            }

            let group_len = group_end - group_start;
            if group_len > 1 {
                for i in 0..group_len {
                    let idx = group_start + i;
                    let offset = if up {
                        i as f64 * (spread_beats / group_len as f64)
                    } else {
                        (group_len - 1 - i) as f64 * (spread_beats / group_len as f64)
                    };
                    self.notes[idx].start_beat += offset;
                }
            }
            group_start = group_end;
        }
    }

    pub fn humanize(&mut self, timing_spread_beats: f64, velocity_spread: i8, seed: u64) {
        let mut state = seed.wrapping_add(1);
        for note in &mut self.notes {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let time_rand = ((state >> 33) as f64 / 2147483648.0) * 2.0 - 1.0;
            note.start_beat = (note.start_beat + time_rand * timing_spread_beats).max(0.0);

            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let vel_rand = ((state >> 33) as f64 / 2147483648.0) * 2.0 - 1.0;
            let delta_v = (vel_rand * velocity_spread as f64) as i16;
            note.velocity = (note.velocity as i16 + delta_v).clamp(1, 127) as u8;
        }
    }

    pub fn invert_pitches(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        let sum: u32 = self.notes.iter().map(|n| n.pitch as u32).sum();
        let avg_pitch = (sum / self.notes.len() as u32) as i16;
        for note in &mut self.notes {
            let diff = note.pitch as i16 - avg_pitch;
            note.pitch = (avg_pitch - diff).clamp(0, 127) as u8;
        }
    }

    pub fn reverse_time(&mut self, total_length_beats: f64) {
        let max_time = total_length_beats.max(
            self.notes
                .iter()
                .map(|n| n.start_beat + n.duration_beats)
                .fold(0.0f64, f64::max),
        );
        for note in &mut self.notes {
            let note_end = note.start_beat + note.duration_beats;
            note.start_beat = (max_time - note_end).max(0.0);
        }
        self.notes.sort_by(|a, b| {
            a.start_beat
                .partial_cmp(&b.start_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn velocity_ramp(&mut self, start_vel: u8, end_vel: u8) {
        if self.notes.is_empty() {
            return;
        }
        let min_start = self
            .notes
            .iter()
            .map(|n| n.start_beat)
            .fold(f64::INFINITY, f64::min);
        let max_start = self
            .notes
            .iter()
            .map(|n| n.start_beat)
            .fold(0.0f64, f64::max);
        let span = (max_start - min_start).max(0.001);

        for note in &mut self.notes {
            let norm = (note.start_beat - min_start) / span;
            let vel = (start_vel as f64 + (end_vel as f64 - start_vel as f64) * norm).round();
            note.velocity = (vel as i16).clamp(1, 127) as u8;
        }
    }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleType {
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
}

impl ScaleType {
    pub const ALL: &'static [ScaleType] = &[
        ScaleType::Major,
        ScaleType::Minor,
        ScaleType::HarmonicMinor,
        ScaleType::MelodicMinor,
        ScaleType::Dorian,
        ScaleType::Phrygian,
        ScaleType::Lydian,
        ScaleType::Mixolydian,
        ScaleType::Locrian,
        ScaleType::MajorPentatonic,
        ScaleType::MinorPentatonic,
        ScaleType::Blues,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ScaleType::Major => "Major",
            ScaleType::Minor => "Minor",
            ScaleType::HarmonicMinor => "Harmonic Minor",
            ScaleType::MelodicMinor => "Melodic Minor",
            ScaleType::Dorian => "Dorian",
            ScaleType::Phrygian => "Phrygian",
            ScaleType::Lydian => "Lydian",
            ScaleType::Mixolydian => "Mixolydian",
            ScaleType::Locrian => "Locrian",
            ScaleType::MajorPentatonic => "Major Pentatonic",
            ScaleType::MinorPentatonic => "Minor Pentatonic",
            ScaleType::Blues => "Blues",
        }
    }

    pub fn intervals(&self) -> &'static [u8] {
        match self {
            ScaleType::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleType::Minor => &[0, 2, 3, 5, 7, 8, 10],
            ScaleType::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleType::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            ScaleType::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleType::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleType::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleType::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleType::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            ScaleType::MajorPentatonic => &[0, 2, 4, 7, 9],
            ScaleType::MinorPentatonic => &[0, 3, 5, 7, 10],
            ScaleType::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }
}

pub const NOTE_NAMES: &[&str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleConfig {
    pub root_note: u8, // 0 = C, 1 = C#, ..., 11 = B
    pub scale_type: ScaleType,
    pub is_active: bool,
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self {
            root_note: 0, // C
            scale_type: ScaleType::Minor,
            is_active: false,
        }
    }
}

impl ScaleConfig {
    pub fn new(root_note: u8, scale_type: ScaleType) -> Self {
        Self {
            root_note: root_note % 12,
            scale_type,
            is_active: true,
        }
    }

    pub fn is_in_scale(&self, pitch: u8) -> bool {
        if !self.is_active {
            return true;
        }
        let semitone_from_root = (pitch + 12 - (self.root_note % 12)) % 12;
        self.scale_type.intervals().contains(&semitone_from_root)
    }

    pub fn snap_to_scale(&self, pitch: u8) -> u8 {
        if !self.is_active || self.is_in_scale(pitch) {
            return pitch;
        }
        let semitone_from_root = (pitch + 12 - (self.root_note % 12)) % 12;
        let intervals = self.scale_type.intervals();
        let mut min_diff = 127;
        let mut best_interval = intervals[0];
        for &int in intervals {
            let diff = (semitone_from_root as i16 - int as i16).abs();
            if diff < min_diff {
                min_diff = diff;
                best_interval = int;
            }
        }
        let octave = (pitch as i16 - semitone_from_root as i16) / 12;
        (octave * 12 + self.root_note as i16 + best_interval as i16).clamp(0, 127) as u8
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    EqEight,
    Delay,
    Reverb,
    Compressor,
    Limiter,
    Utility,
    Operator,
    Simpler,
    Saturator,
    DrumRack,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: u64,
    pub name: String,
    pub device_type: DeviceType,
    pub is_enabled: bool,
    pub parameters: HashMap<String, f32>,
}

impl Device {
    pub fn new(id: u64, name: impl Into<String>, device_type: DeviceType) -> Self {
        let mut parameters = HashMap::new();
        match &device_type {
            DeviceType::EqEight => {
                // 8 bands: freq, gain, q
                for i in 1..=8 {
                    parameters.insert(format!("band_{}_freq", i), 100.0 * (2.0f32.powi(i - 1)));
                    parameters.insert(format!("band_{}_gain", i), 0.0);
                    parameters.insert(format!("band_{}_q", i), 0.707);
                    parameters.insert(format!("band_{}_enabled", i), 1.0);
                }
            }
            DeviceType::Delay => {
                parameters.insert("time_ms".to_string(), 375.0); // 1/8th note at 120 BPM
                parameters.insert("feedback".to_string(), 0.35);
                parameters.insert("dry_wet".to_string(), 0.30);
                parameters.insert("ping_pong".to_string(), 1.0);
            }
            DeviceType::Reverb => {
                parameters.insert("decay_time_sec".to_string(), 2.2);
                parameters.insert("room_size".to_string(), 0.65);
                parameters.insert("damping".to_string(), 0.4);
                parameters.insert("dry_wet".to_string(), 0.25);
                parameters.insert("stereo_width".to_string(), 1.0);
            }
            DeviceType::Compressor => {
                parameters.insert("threshold_db".to_string(), -18.0);
                parameters.insert("ratio".to_string(), 4.0);
                parameters.insert("attack_ms".to_string(), 15.0);
                parameters.insert("release_ms".to_string(), 100.0);
                parameters.insert("makeup_gain_db".to_string(), 2.0);
            }
            DeviceType::Limiter => {
                parameters.insert("ceiling_db".to_string(), -0.3);
                parameters.insert("gain_db".to_string(), 0.0);
                parameters.insert("release_ms".to_string(), 150.0);
            }
            DeviceType::Utility => {
                parameters.insert("gain_db".to_string(), 0.0);
                parameters.insert("pan".to_string(), 0.0);
                parameters.insert("width".to_string(), 1.0); // 1.0 = 100%
                parameters.insert("mute".to_string(), 0.0);
            }
            DeviceType::Operator => {
                parameters.insert("osc1_waveform".to_string(), 0.0); // 0=Sine, 1=Saw, 2=Square, 3=Triangle
                parameters.insert("osc1_level".to_string(), 1.0);
                parameters.insert("osc1_coarse".to_string(), 1.0);
                parameters.insert("osc2_waveform".to_string(), 1.0); // Saw
                parameters.insert("osc2_level".to_string(), 0.5);
                parameters.insert("osc2_coarse".to_string(), 2.0);
                parameters.insert("filter_cutoff".to_string(), 2400.0);
                parameters.insert("filter_resonance".to_string(), 0.3);
                parameters.insert("attack_ms".to_string(), 5.0);
                parameters.insert("decay_ms".to_string(), 120.0);
                parameters.insert("sustain".to_string(), 0.7);
                parameters.insert("release_ms".to_string(), 300.0);
            }
            DeviceType::Simpler => {
                parameters.insert("playback_mode".to_string(), 0.0); // 0=Classic, 1=1-Shot, 2=Slice
                parameters.insert("gain_db".to_string(), 0.0);
                parameters.insert("attack_ms".to_string(), 2.0);
                parameters.insert("decay_ms".to_string(), 50.0);
                parameters.insert("sustain".to_string(), 1.0);
                parameters.insert("release_ms".to_string(), 100.0);
                parameters.insert("filter_cutoff".to_string(), 20000.0);
            }
            DeviceType::Saturator => {
                parameters.insert("drive_db".to_string(), 6.0);
                parameters.insert("curve_type".to_string(), 0.0); // 0=Soft Sine, 1=Hard Clip, 2=Analog Warmth, 3=Foldback
                parameters.insert("output_db".to_string(), -2.0);
                parameters.insert("dry_wet".to_string(), 1.0);
                parameters.insert("tone".to_string(), 0.5);
            }
            _ => {}
        }

        Self {
            id,
            name: name.into(),
            device_type,
            is_enabled: true,
            parameters,
        }
    }

    pub fn get_param(&self, key: &str) -> f32 {
        self.parameters.get(key).copied().unwrap_or(0.0)
    }

    pub fn set_param(&mut self, key: impl Into<String>, value: f32) {
        self.parameters.insert(key.into(), value);
    }
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
    pub sends: Vec<f32>, // Send levels to Return tracks (0.0 to 1.0)
    pub clip_slots: Vec<ClipSlot>,
    #[serde(default)]
    pub arrangement_clips: Vec<ArrangementClip>,
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
            sends: vec![0.0, 0.0], // Sends A and B
            clip_slots,
            arrangement_clips: Vec::new(),
            device_chain: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: u64,
    pub name: String,
    pub tempo: Option<f32>,
    pub time_signature: Option<(u8, u8)>,
    pub color: String,
}

impl Scene {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tempo: None,
            time_signature: None,
            color: "#464646".to_string(),
        }
    }
}

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
            tracks,
            return_tracks,
            master_track,
            scenes,
        }
    }
}
