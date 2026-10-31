use serde::{Deserialize, Serialize};

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
