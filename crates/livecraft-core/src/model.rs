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
            if let Some(sc) = scale
                && sc.is_active
            {
                new_pitch = sc.snap_to_scale(new_pitch);
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

    pub fn chop(&mut self, subdivisions: u32) {
        if subdivisions <= 1 || self.notes.is_empty() {
            return;
        }
        let mut new_notes = Vec::with_capacity(self.notes.len() * subdivisions as usize);
        for note in &self.notes {
            let slice_dur = note.duration_beats / subdivisions as f64;
            for s in 0..subdivisions {
                new_notes.push(MidiNote::new(
                    note.pitch,
                    note.start_beat + s as f64 * slice_dur,
                    slice_dur,
                    note.velocity,
                ));
            }
        }
        self.notes = new_notes;
    }

    pub fn ornament(&mut self, grace_offset_beats: f64, velocity_scale: f32) {
        if self.notes.is_empty() {
            return;
        }
        let offset = grace_offset_beats.max(0.015625);
        let mut new_notes = Vec::with_capacity(self.notes.len() * 2);
        for note in &self.notes {
            if note.start_beat >= offset {
                let grace_vel = ((note.velocity as f32) * velocity_scale).clamp(1.0, 127.0) as u8;
                new_notes.push(MidiNote::new(
                    note.pitch,
                    note.start_beat - offset,
                    offset,
                    grace_vel,
                ));
            }
            new_notes.push(note.clone());
        }
        self.notes = new_notes;
    }

    pub fn span(&mut self) {
        if self.notes.len() < 2 {
            return;
        }
        self.notes.sort_by(|a, b| {
            a.start_beat
                .partial_cmp(&b.start_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in 0..self.notes.len() - 1 {
            let next_start = self.notes[i + 1].start_beat;
            if next_start > self.notes[i].start_beat {
                self.notes[i].duration_beats = next_start - self.notes[i].start_beat;
            }
        }
    }

    pub fn time_warp(&mut self, exponent: f64) {
        if self.notes.is_empty() || (exponent - 1.0).abs() < 1e-4 {
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
        let span = max_start - min_start;
        if span <= 0.001 {
            return;
        }
        for note in &mut self.notes {
            let norm = (note.start_beat - min_start) / span;
            let warped_norm = norm.powf(exponent);
            note.start_beat = min_start + warped_norm * span;
        }
    }

    pub fn generate_euclidean(
        &mut self,
        pulses: u32,
        steps: u32,
        pitch: u8,
        duration_beats: f64,
        velocity: u8,
    ) {
        if steps == 0 || pulses == 0 {
            return;
        }
        let step_dur = duration_beats / steps as f64;
        let note_dur = step_dur * 0.8;
        for i in 0..steps {
            if (i * pulses) % steps < pulses {
                self.notes.push(MidiNote::new(
                    pitch,
                    i as f64 * step_dur,
                    note_dur,
                    velocity,
                ));
            }
        }
        self.notes.sort_by(|a, b| {
            a.start_beat
                .partial_cmp(&b.start_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn generate_stacks(
        &mut self,
        chord_degrees: &[u8],
        root_pitch: u8,
        duration_beats: f64,
        velocity: u8,
        scale: Option<&ScaleConfig>,
    ) {
        for &degree in chord_degrees {
            let mut pitch = (root_pitch as i16 + degree as i16).clamp(0, 127) as u8;
            if let Some(sc) = scale
                && sc.is_active
            {
                pitch = sc.snap_to_scale(pitch);
            }
            self.notes
                .push(MidiNote::new(pitch, 0.0, duration_beats, velocity));
        }
        self.notes.sort_by_key(|a| a.pitch);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AutomationTarget {
    TrackVolume,
    TrackPan,
    SendLevel(usize),
    DeviceParam { device_id: u64, param_key: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub beat: f64,
    pub value: f32,
    pub curve: f32, // -1.0 to 1.0 curvature
}

impl AutomationPoint {
    pub fn new(beat: f64, value: f32, curve: f32) -> Self {
        Self { beat, value, curve }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationLane {
    pub id: u64,
    pub target: AutomationTarget,
    pub points: Vec<AutomationPoint>,
    pub is_enabled: bool,
}

impl AutomationLane {
    pub fn new(id: u64, target: AutomationTarget) -> Self {
        Self {
            id,
            target,
            points: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn evaluate_at(&self, beat: f64) -> Option<f32> {
        if self.points.is_empty() || !self.is_enabled {
            return None;
        }
        if beat <= self.points[0].beat {
            return Some(self.points[0].value);
        }
        if beat >= self.points.last().unwrap().beat {
            return Some(self.points.last().unwrap().value);
        }
        // Find surrounding segment and interpolate
        for w in self.points.windows(2) {
            if beat >= w[0].beat && beat <= w[1].beat {
                let span = w[1].beat - w[0].beat;
                if span <= 1e-6 {
                    return Some(w[0].value);
                }
                let t = (beat - w[0].beat) / span;
                let factor = if w[0].curve.abs() > 1e-4 {
                    let exp = (-w[0].curve as f64 * 2.0).exp();
                    t.powf(exp)
                } else {
                    t
                };
                return Some(w[0].value + (w[1].value - w[0].value) * factor as f32);
            }
        }
        None
    }
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

/// Note name in Live's convention, where MIDI note 60 is C3 and note 0 is C-2.
pub fn note_name(pitch: u8) -> String {
    let octave = i32::from(pitch / 12) - 2;
    format!("{}{}", NOTE_NAMES[usize::from(pitch % 12)], octave)
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    AutoFilter,
    Chorus,
    AutoPan,
    Gate,
    EqThree,
    Redux,
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
            DeviceType::AutoFilter => {
                parameters.insert("filter_type".to_string(), 0.0); // 0=LP, 1=HP, 2=BP, 3=Notch
                parameters.insert("cutoff".to_string(), 1200.0);
                parameters.insert("resonance".to_string(), 1.5);
                parameters.insert("lfo_rate".to_string(), 1.0);
                parameters.insert("lfo_depth".to_string(), 0.4);
                parameters.insert("envelope_amount".to_string(), 0.3);
            }
            DeviceType::Chorus => {
                parameters.insert("rate_hz".to_string(), 1.2);
                parameters.insert("depth".to_string(), 0.5);
                parameters.insert("feedback".to_string(), 0.2);
                parameters.insert("dry_wet".to_string(), 0.5);
            }
            DeviceType::AutoPan => {
                parameters.insert("amount".to_string(), 1.0);
                parameters.insert("rate_hz".to_string(), 1.0);
                parameters.insert("phase_deg".to_string(), 180.0);
                parameters.insert("shape".to_string(), 0.0); // 0=Sine, 1=Tri
                parameters.insert("mode".to_string(), 0.0); // 0=Panning, 1=Tremolo
            }
            DeviceType::Gate => {
                parameters.insert("threshold_db".to_string(), -30.0);
                parameters.insert("return_db".to_string(), 3.0);
                parameters.insert("attack_ms".to_string(), 1.0);
                parameters.insert("hold_ms".to_string(), 50.0);
                parameters.insert("release_ms".to_string(), 100.0);
                parameters.insert("floor_db".to_string(), -60.0);
            }
            DeviceType::EqThree => {
                parameters.insert("low_gain_db".to_string(), 0.0);
                parameters.insert("mid_gain_db".to_string(), 0.0);
                parameters.insert("high_gain_db".to_string(), 0.0);
                parameters.insert("low_freq".to_string(), 250.0);
                parameters.insert("high_freq".to_string(), 2500.0);
                parameters.insert("low_on".to_string(), 1.0);
                parameters.insert("mid_on".to_string(), 1.0);
                parameters.insert("high_on".to_string(), 1.0);
            }
            DeviceType::Redux => {
                parameters.insert("rate_factor".to_string(), 1.0); // 1.0 to 32.0 downsample
                parameters.insert("bits".to_string(), 16.0); // 1.0 to 16.0 bits
                parameters.insert("jitter".to_string(), 0.0);
                parameters.insert("dry_wet".to_string(), 1.0);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuningSystem {
    pub name: String,
    pub description: String,
    pub degrees_cents: Vec<f64>,
    pub base_freq_hz: f64,
    pub base_midi_note: u8,
}

impl Default for TuningSystem {
    fn default() -> Self {
        Self::equal_temperament_12()
    }
}

impl TuningSystem {
    pub fn equal_temperament_12() -> Self {
        Self {
            name: "12-TET".to_string(),
            description: "Standard 12-Tone Equal Temperament".to_string(),
            degrees_cents: (0..12).map(|i| i as f64 * 100.0).collect(),
            base_freq_hz: 440.0,
            base_midi_note: 69,
        }
    }

    pub fn from_scala(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.starts_with('!') && !l.is_empty());
        let description = lines.next().unwrap_or("Custom Tuning").to_string();
        let count_str = lines
            .next()
            .ok_or_else(|| "Missing scale degree count".to_string())?;
        let count: usize = count_str
            .parse()
            .map_err(|e| format!("Invalid count: {}", e))?;

        let mut degrees = Vec::with_capacity(count + 1);
        degrees.push(0.0);

        for line in lines.take(count) {
            let token = line.split_whitespace().next().unwrap_or("");
            if token.contains('.') {
                let cents: f64 = token.parse().map_err(|e| format!("Invalid cents: {}", e))?;
                degrees.push(cents);
            } else if token.contains('/') {
                let parts: Vec<&str> = token.split('/').collect();
                if parts.len() == 2 {
                    let num: f64 = parts[0]
                        .parse()
                        .map_err(|e| format!("Invalid numerator: {}", e))?;
                    let den: f64 = parts[1]
                        .parse()
                        .map_err(|e| format!("Invalid denominator: {}", e))?;
                    if den > 0.0 {
                        let cents = 1200.0 * (num / den).log2();
                        degrees.push(cents);
                    }
                }
            } else if let Ok(val) = token.parse::<f64>() {
                degrees.push(val);
            }
        }

        Ok(Self {
            name: description.clone(),
            description,
            degrees_cents: degrees,
            base_freq_hz: 440.0,
            base_midi_note: 69,
        })
    }

    pub fn note_to_freq(&self, midi_note: u8) -> f64 {
        if self.degrees_cents.is_empty() {
            return 440.0 * 2.0f64.powf((midi_note as f64 - 69.0) / 12.0);
        }
        let (count, period_cents) = if self.degrees_cents.len() > 1
            && self.degrees_cents[0] == 0.0
            && self.degrees_cents.last().copied().unwrap_or(0.0) >= 100.0
            && self.degrees_cents.len() != 12
        {
            let cnt = self.degrees_cents.len() - 1;
            (cnt, self.degrees_cents[cnt])
        } else {
            (self.degrees_cents.len(), 1200.0)
        };
        let delta = midi_note as i32 - self.base_midi_note as i32;
        let octave = delta.div_euclid(count as i32);
        let deg = delta.rem_euclid(count as i32) as usize;
        let cents = octave as f64 * period_cents + self.degrees_cents[deg];
        let freq = self.base_freq_hz * 2.0f64.powf(cents / 1200.0);
        freq.clamp(20.0, 20000.0)
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
