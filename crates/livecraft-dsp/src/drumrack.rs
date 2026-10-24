use livecraft_core::model::Device;
use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct DrumPadVoice {
    pub note: u8,
    pub name: String,
    pub choke_group: Option<u8>,
    pub gain_db: f32,
    pub pan: f32,
    pub is_muted: bool,
    pub is_soloed: bool,
    // Synthesis state
    pub is_active: bool,
    pub sample_index: usize,
    pub pitch_env: f32,
    pub amp_env: f32,
    noise_seed: u32,
    sample_rate: f32,
}

impl DrumPadVoice {
    pub fn new(
        note: u8,
        name: impl Into<String>,
        choke_group: Option<u8>,
        sample_rate: f32,
    ) -> Self {
        Self {
            note,
            name: name.into(),
            choke_group,
            gain_db: 0.0,
            pan: 0.0,
            is_muted: false,
            is_soloed: false,
            is_active: false,
            sample_index: 0,
            pitch_env: 0.0,
            amp_env: 0.0,
            noise_seed: (note as u32).wrapping_mul(1664525).wrapping_add(1013904223),
            sample_rate,
        }
    }

    pub fn trigger(&mut self, velocity: u8) {
        self.is_active = true;
        self.sample_index = 0;
        self.amp_env = (velocity as f32 / 127.0).clamp(0.1, 1.0);
        self.pitch_env = 1.0;
    }

    pub fn choke(&mut self) {
        self.is_active = false;
        self.amp_env = 0.0;
    }

    fn next_noise(&mut self) -> f32 {
        self.noise_seed = self
            .noise_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        ((self.noise_seed >> 9) as f32 / 8388608.0) - 1.0
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_active || self.is_muted {
            return (0.0, 0.0);
        }

        let dt = 1.0 / self.sample_rate;
        let t = self.sample_index as f32 * dt;

        let sample = match self.note {
            // 36: Kick Drum (Punchy sine pitch drop)
            36 => {
                let freq = 45.0 + 160.0 * self.pitch_env;
                let phase = 2.0 * PI * freq * t;
                let s = phase.sin() * self.amp_env;
                self.pitch_env = (self.pitch_env - dt * 25.0).max(0.0);
                self.amp_env -= dt * 3.5;
                s
            }
            // 37: Side Stick / Rimshot
            37 => {
                let noise = self.next_noise();
                let tone = (2.0 * PI * 800.0 * t).sin();
                let s = (tone * 0.7 + noise * 0.3) * self.amp_env;
                self.amp_env -= dt * 25.0;
                s
            }
            // 38 | 40: Snare Drum (Pitched body + white noise burst)
            38 | 40 => {
                let body = (2.0 * PI * (185.0 + 60.0 * self.pitch_env) * t).sin();
                let noise = self.next_noise();
                let s = (body * 0.5 + noise * 0.6) * self.amp_env;
                self.pitch_env = (self.pitch_env - dt * 30.0).max(0.0);
                self.amp_env -= dt * 5.5;
                s
            }
            // 39: Hand Clap (Multi-pulse noise envelope)
            39 => {
                let noise = self.next_noise();
                let pulse_amp = if t < 0.010 || (t > 0.020 && t < 0.030) || (t > 0.040 && t < 0.050)
                {
                    1.0
                } else if t >= 0.050 {
                    (1.0 - (t - 0.050) * 8.0).max(0.0)
                } else {
                    0.2
                };
                let s = noise * pulse_amp * self.amp_env;
                self.amp_env -= dt * 4.0;
                s
            }
            // 42 | 44: Closed / Pedal Hi-Hat (Short metallic noise)
            42 | 44 => {
                let noise = self.next_noise();
                let metallic = (2.0 * PI * 7500.0 * t).sin() * 0.3;
                let s = (noise + metallic) * self.amp_env;
                self.amp_env -= dt * 22.0;
                s
            }
            // 46: Open Hi-Hat (Sustained metallic sizzle)
            46 => {
                let noise = self.next_noise();
                let metallic = (2.0 * PI * 7200.0 * t).sin() * 0.35;
                let s = (noise + metallic) * self.amp_env;
                self.amp_env -= dt * 3.0;
                s
            }
            // 41 | 43 | 45 | 47: Toms (Resonant deep tone with moderate decay)
            41 | 43 | 45 | 47 => {
                let base_freq = match self.note {
                    41 => 85.0,  // Low Tom
                    43 => 110.0, // Low-Mid Tom
                    45 => 140.0, // Mid Tom
                    _ => 175.0,  // High Tom
                };
                let freq = base_freq + 80.0 * self.pitch_env;
                let tone = (2.0 * PI * freq * t).sin();
                let s = tone * self.amp_env;
                self.pitch_env = (self.pitch_env - dt * 15.0).max(0.0);
                self.amp_env -= dt * 4.0;
                s
            }
            // 48..=50: Cymbals (Crash & Ride)
            48..=50 => {
                let noise = self.next_noise();
                let ring = (2.0 * PI * 4800.0 * t).sin() * 0.25;
                let s = (noise * 0.75 + ring) * self.amp_env;
                self.amp_env -= dt * 1.5;
                s
            }
            // 51: Tambourine / Shaker
            _ => {
                let noise = self.next_noise();
                let jingle = (2.0 * PI * 9000.0 * t).sin() * 0.4;
                let s = (noise * 0.6 + jingle) * self.amp_env;
                self.amp_env -= dt * 12.0;
                s
            }
        };

        if self.amp_env <= 0.0001 {
            self.is_active = false;
            return (0.0, 0.0);
        }

        self.sample_index += 1;

        let gain_lin = 10.0f32.powf(self.gain_db / 20.0);
        let pan_norm = (self.pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let angle = pan_norm * std::f32::consts::FRAC_PI_2;
        let pan_l = angle.cos();
        let pan_r = angle.sin();

        (sample * gain_lin * pan_l, sample * gain_lin * pan_r)
    }
}

#[derive(Debug, Clone)]
pub struct DrumRackInstrument {
    pub pads: Vec<DrumPadVoice>,
    pub is_enabled: bool,
    pub master_gain_db: f32,
}

impl DrumRackInstrument {
    pub fn new(sample_rate: f32) -> Self {
        // Standard 16 Ableton Drum Rack pads (C1 - D#2: notes 36 - 51)
        let pad_defs: &[(u8, &str, Option<u8>)] = &[
            (36, "Kick", None),
            (37, "Rimshot", None),
            (38, "Snare 1", None),
            (39, "Clap", None),
            (40, "Snare 2", None),
            (41, "Low Tom", None),
            (42, "Closed HH", Some(1)), // Choke group 1 for hats
            (43, "Low-Mid Tom", None),
            (44, "Pedal HH", Some(1)), // Choke group 1
            (45, "Mid Tom", None),
            (46, "Open HH", Some(1)), // Choke group 1
            (47, "High Tom", None),
            (48, "Crash Cymbal", None),
            (49, "Ride Cymbal", None),
            (50, "Ride Bell", None),
            (51, "Perc / Tamb", None),
        ];

        let pads = pad_defs
            .iter()
            .map(|&(note, name, choke)| DrumPadVoice::new(note, name, choke, sample_rate))
            .collect();

        Self {
            pads,
            is_enabled: true,
            master_gain_db: 0.0,
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if !self.is_enabled {
            return;
        }

        // Check if the triggered note belongs to a choke group
        let choke_group = self
            .pads
            .iter()
            .find(|p| p.note == note)
            .and_then(|p| p.choke_group);

        if let Some(group) = choke_group {
            for pad in &mut self.pads {
                if pad.choke_group == Some(group) && pad.note != note {
                    pad.choke();
                }
            }
        }

        // Trigger matching pad
        for pad in &mut self.pads {
            if pad.note == note {
                pad.trigger(velocity);
            }
        }
    }

    pub fn note_off(&mut self, _note: u8) {
        // One-shot drum pads decay naturally
    }

    pub fn all_notes_off(&mut self) {
        for pad in &mut self.pads {
            pad.choke();
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.master_gain_db = dev.get_param("gain_db");
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_enabled {
            return (0.0, 0.0);
        }

        let any_solo = self.pads.iter().any(|p| p.is_soloed);
        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;

        for pad in &mut self.pads {
            if any_solo && !pad.is_soloed {
                continue;
            }
            let (l, r) = pad.process_sample();
            sum_l += l;
            sum_r += r;
        }

        let master_lin = 10.0f32.powf(self.master_gain_db / 20.0);
        (sum_l * master_lin, sum_r * master_lin)
    }
}
