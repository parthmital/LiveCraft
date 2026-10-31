//! MIDI transformations moved unchanged from the M01 model. M19 replaces them.

use crate::note::{MidiClipData, MidiNote};
use crate::scale::ScaleConfig;

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
            note.velocity = (note.velocity + f32::from(delta_v)).clamp(1.0, 127.0);
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
            note.velocity = (vel as f32).clamp(1.0, 127.0);
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
                let mut slice = note.clone();
                slice.start_beat = note.start_beat + s as f64 * slice_dur;
                slice.duration_beats = slice_dur;
                new_notes.push(slice);
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
                let mut grace = MidiNote::new(note.pitch, note.start_beat - offset, offset, 1);
                grace.velocity = (note.velocity * velocity_scale).clamp(1.0, 127.0);
                new_notes.push(grace);
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
