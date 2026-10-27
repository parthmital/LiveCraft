use crate::biquad::{BiquadFilter, FilterType};
use crate::operator::AdsrEnvelope;
use livecraft_core::model::Device;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimplerPlaybackMode {
    Classic,
    OneShot,
    Slicing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone)]
pub struct SimplerSample {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SimplerVoice {
    pub note: u8,
    pub velocity: f32,
    pub playhead: f64,
    pub speed: f64,
    pub stage: EnvStage,
    pub env_level: f32,
    pub is_active: bool,
    pub start_sample: usize,
    pub end_sample: usize,
    pub age: u64,
}

impl SimplerVoice {
    pub fn new() -> Self {
        Self {
            note: 0,
            velocity: 1.0,
            playhead: 0.0,
            speed: 1.0,
            stage: EnvStage::Off,
            env_level: 0.0,
            is_active: false,
            start_sample: 0,
            end_sample: 0,
            age: 0,
        }
    }
}

impl Default for SimplerVoice {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SimplerInstrument {
    pub sample: Option<SimplerSample>,
    sample_rate: f32,
    pub mode: SimplerPlaybackMode,
    pub voices: Vec<SimplerVoice>,
    pub is_active: bool,
    pub envelope: AdsrEnvelope,
    filter: BiquadFilter,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub root_note: u8,
    pub is_looping: bool,
    pub is_enabled: bool,
    pub velocity: f32,
    pub start_pct: f32,
    pub length_pct: f32,
    pub crossfade_pct: f32,
    pub gate_mode: bool,
    pub slices: Vec<usize>,
}

fn process_env(stage: &mut EnvStage, level: &mut f32, adsr: &AdsrEnvelope, dt: f32) {
    match *stage {
        EnvStage::Attack => {
            let rate = dt / adsr.attack_sec.max(0.001);
            *level += rate;
            if *level >= 1.0 {
                *level = 1.0;
                *stage = EnvStage::Decay;
            }
        }
        EnvStage::Decay => {
            let rate = dt / adsr.decay_sec.max(0.001);
            *level -= rate * (1.0 - adsr.sustain_level);
            if *level <= adsr.sustain_level {
                *level = adsr.sustain_level;
                *stage = EnvStage::Sustain;
            }
        }
        EnvStage::Sustain => {
            *level = adsr.sustain_level;
        }
        EnvStage::Release => {
            let rate = dt / adsr.release_sec.max(0.001);
            *level -= rate;
            if *level <= 0.0001 {
                *level = 0.0;
                *stage = EnvStage::Off;
            }
        }
        EnvStage::Off => {
            *level = 0.0;
        }
    }
}

impl SimplerInstrument {
    pub fn new(sample_rate: f32) -> Self {
        let max_voices = 16;
        let voices = (0..max_voices).map(|_| SimplerVoice::new()).collect();

        Self {
            sample: None,
            sample_rate,
            mode: SimplerPlaybackMode::Classic,
            voices,
            is_active: false,
            envelope: AdsrEnvelope::default(),
            filter: BiquadFilter::new(FilterType::LowPass, sample_rate, 18000.0, 0.0, 0.707),
            filter_cutoff: 18000.0,
            filter_resonance: 0.707,
            root_note: 60, // Middle C
            is_looping: false,
            is_enabled: true,
            velocity: 1.0,
            start_pct: 0.0,
            length_pct: 1.0,
            crossfade_pct: 0.0,
            gate_mode: true,
            slices: Vec::new(),
        }
    }

    pub fn set_sample_data(&mut self, samples_l: Vec<f32>, samples_r: Vec<f32>, sample_rate: u32) {
        let channels = if samples_r.is_empty() { 1 } else { 2 };
        let r = if samples_r.is_empty() {
            samples_l.clone()
        } else {
            samples_r
        };
        self.sample = Some(SimplerSample {
            channels,
            sample_rate,
            samples_l,
            samples_r: r,
        });
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.filter_cutoff = dev.get_param("filter_cutoff").clamp(20.0, 20000.0);
        if let Some(res) = dev.parameters.get("filter_resonance") {
            self.filter_resonance = res.clamp(0.1, 10.0);
        }
        if let Some(m) = dev.parameters.get("playback_mode") {
            let mi = m.round() as i32;
            self.mode = match mi {
                0 => SimplerPlaybackMode::Classic,
                1 => SimplerPlaybackMode::OneShot,
                2 => SimplerPlaybackMode::Slicing,
                _ => SimplerPlaybackMode::Classic,
            };
        }
        if let Some(sp) = dev.parameters.get("start_pct") {
            self.start_pct = sp.clamp(0.0, 1.0);
        }
        if let Some(lp) = dev.parameters.get("length_pct") {
            self.length_pct = lp.clamp(0.01, 1.0);
        }
        if let Some(xf) = dev.parameters.get("crossfade_pct") {
            self.crossfade_pct = xf.clamp(0.0, 1.0);
        }
        self.envelope.attack_sec = (dev.get_param("attack_ms") * 0.001).clamp(0.001, 10.0);
        self.envelope.decay_sec = (dev.get_param("decay_ms") * 0.001).clamp(0.001, 10.0);
        self.envelope.sustain_level = dev.get_param("sustain").clamp(0.0, 1.0);
        self.envelope.release_sec = (dev.get_param("release_ms") * 0.001).clamp(0.001, 10.0);
    }

    pub fn load_wav<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let channels = spec.channels;
        let srate = spec.sample_rate;

        let mut samples_l = Vec::new();
        let mut samples_r = Vec::new();

        let max_val = match spec.bits_per_sample {
            16 => i16::MAX as f32,
            24 => 8388607.0,
            32 => i32::MAX as f32,
            _ => 32767.0,
        };

        if spec.sample_format == hound::SampleFormat::Float {
            let mut ch = 0;
            for sample in reader.samples::<f32>() {
                let s = sample.unwrap_or(0.0);
                if ch == 0 {
                    samples_l.push(s);
                    if channels == 1 {
                        samples_r.push(s);
                    }
                } else {
                    samples_r.push(s);
                }
                ch = (ch + 1) % channels;
            }
        } else {
            let mut ch = 0;
            for sample in reader.samples::<i32>() {
                let s = sample.unwrap_or(0) as f32 / max_val;
                if ch == 0 {
                    samples_l.push(s);
                    if channels == 1 {
                        samples_r.push(s);
                    }
                } else {
                    samples_r.push(s);
                }
                ch = (ch + 1) % channels;
            }
        }

        self.sample = Some(SimplerSample {
            channels,
            sample_rate: srate,
            samples_l,
            samples_r,
        });

        Ok(())
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let freq = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
        self.note_on_with_freq(note, velocity, freq);
    }

    pub fn note_on_with_freq(&mut self, note: u8, velocity: u8, freq_hz: f32) {
        let norm_vel = (velocity as f32 / 127.0).clamp(0.01, 1.0);
        self.velocity = norm_vel;

        let sample = match self.sample {
            Some(ref s) if !s.samples_l.is_empty() => s,
            _ => return,
        };

        let total_samples = sample.samples_l.len();
        self.is_active = true;

        match self.mode {
            SimplerPlaybackMode::Classic => {
                let root_freq = 440.0 * 2.0f64.powf((self.root_note as f64 - 69.0) / 12.0);
                let pitch_ratio = (freq_hz as f64 / root_freq).clamp(0.01, 100.0);
                let speed = pitch_ratio * (sample.sample_rate as f64 / self.sample_rate as f64);

                let start_idx = ((self.start_pct.clamp(0.0, 1.0) * total_samples as f32) as usize)
                    .min(total_samples - 1);
                let len_samples =
                    ((self.length_pct.clamp(0.01, 1.0) * total_samples as f32) as usize).max(16);
                let end_idx = (start_idx + len_samples).min(total_samples);

                // Find voice: inactive, or releasing, or oldest
                let voice_idx = self
                    .voices
                    .iter()
                    .position(|v| !v.is_active)
                    .or_else(|| {
                        self.voices
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| v.stage == EnvStage::Release)
                            .min_by(|(_, a), (_, b)| {
                                a.env_level
                                    .partial_cmp(&b.env_level)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(idx, _)| idx)
                    })
                    .unwrap_or_else(|| {
                        self.voices
                            .iter()
                            .enumerate()
                            .max_by_key(|(_, v)| v.age)
                            .map(|(idx, _)| idx)
                            .unwrap_or(0)
                    });

                let v = &mut self.voices[voice_idx];
                v.note = note;
                v.velocity = norm_vel;
                v.playhead = start_idx as f64;
                v.speed = speed;
                v.start_sample = start_idx;
                v.end_sample = end_idx;
                v.stage = EnvStage::Attack;
                v.is_active = true;
                v.age = 0;
            }
            SimplerPlaybackMode::OneShot => {
                let root_freq = 440.0 * 2.0f64.powf((self.root_note as f64 - 69.0) / 12.0);
                let pitch_ratio = (freq_hz as f64 / root_freq).clamp(0.01, 100.0);
                let speed = pitch_ratio * (sample.sample_rate as f64 / self.sample_rate as f64);

                let start_idx = ((self.start_pct.clamp(0.0, 1.0) * total_samples as f32) as usize)
                    .min(total_samples - 1);
                let end_idx = total_samples;

                // Monophonic voice 0
                let v = &mut self.voices[0];
                v.note = note;
                v.velocity = norm_vel;
                v.playhead = start_idx as f64;
                v.speed = speed;
                v.start_sample = start_idx;
                v.end_sample = end_idx;
                v.stage = EnvStage::Attack;
                v.is_active = true;
                v.age = 0;
            }
            SimplerPlaybackMode::Slicing => {
                let slice_idx = if note >= 36 { (note - 36) as usize } else { 0 };
                let (start_idx, end_idx) = if self.slices.is_empty() {
                    let num_slices = 16;
                    let slice_len = (total_samples / num_slices).max(1);
                    let s = (slice_idx * slice_len).min(total_samples - 1);
                    let e = ((slice_idx + 1) * slice_len).min(total_samples);
                    (s, e)
                } else {
                    let s = self
                        .slices
                        .get(slice_idx)
                        .copied()
                        .unwrap_or(0)
                        .min(total_samples - 1);
                    let e = self
                        .slices
                        .get(slice_idx + 1)
                        .copied()
                        .unwrap_or(total_samples)
                        .min(total_samples);
                    (s, e)
                };

                let speed = sample.sample_rate as f64 / self.sample_rate as f64;

                let voice_idx = self.voices.iter().position(|v| !v.is_active).unwrap_or(0);

                let v = &mut self.voices[voice_idx];
                v.note = note;
                v.velocity = norm_vel;
                v.playhead = start_idx as f64;
                v.speed = speed;
                v.start_sample = start_idx;
                v.end_sample = end_idx;
                v.stage = EnvStage::Attack;
                v.is_active = true;
                v.age = 0;
            }
        }
    }

    pub fn note_off(&mut self, note: u8) {
        match self.mode {
            SimplerPlaybackMode::Classic => {
                for voice in &mut self.voices {
                    if voice.note == note && voice.is_active && voice.stage != EnvStage::Off {
                        voice.stage = EnvStage::Release;
                    }
                }
            }
            SimplerPlaybackMode::OneShot => {
                if self.gate_mode
                    && let Some(v) = self.voices.get_mut(0)
                    && v.note == note
                    && v.is_active
                {
                    v.stage = EnvStage::Release;
                }
            }
            SimplerPlaybackMode::Slicing => {
                if self.gate_mode {
                    for voice in &mut self.voices {
                        if voice.note == note && voice.is_active {
                            voice.stage = EnvStage::Release;
                        }
                    }
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.is_active = false;
            voice.stage = EnvStage::Off;
            voice.env_level = 0.0;
        }
        self.is_active = false;
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_enabled || !self.is_active {
            return (0.0, 0.0);
        }

        let sample = match self.sample {
            Some(ref s) if !s.samples_l.is_empty() => s,
            _ => {
                self.is_active = false;
                return (0.0, 0.0);
            }
        };

        let total_samples = sample.samples_l.len();
        let dt = 1.0 / self.sample_rate;
        let mut mix_l = 0.0f32;
        let mut mix_r = 0.0f32;
        let mut any_active = false;

        for voice in &mut self.voices {
            if !voice.is_active {
                continue;
            }
            voice.age = voice.age.saturating_add(1);

            // Envelope advance
            process_env(&mut voice.stage, &mut voice.env_level, &self.envelope, dt);
            if voice.stage == EnvStage::Off {
                voice.is_active = false;
                continue;
            }

            // Boundary check
            if voice.playhead >= voice.end_sample as f64 {
                if self.mode == SimplerPlaybackMode::Classic && self.is_looping {
                    let loop_len = (voice.end_sample - voice.start_sample) as f64;
                    if loop_len > 1.0 {
                        voice.playhead = voice.start_sample as f64
                            + (voice.playhead - voice.start_sample as f64) % loop_len;
                    } else {
                        voice.is_active = false;
                        voice.stage = EnvStage::Off;
                        continue;
                    }
                } else {
                    voice.is_active = false;
                    voice.stage = EnvStage::Off;
                    continue;
                }
            }

            let idx0 = (voice.playhead as usize).clamp(0, total_samples - 1);
            let idx1 = (idx0 + 1).min(total_samples - 1);
            let frac = (voice.playhead - idx0 as f64) as f32;

            let mut sl = sample.samples_l[idx0] * (1.0 - frac) + sample.samples_l[idx1] * frac;
            let mut sr = sample.samples_r[idx0] * (1.0 - frac) + sample.samples_r[idx1] * frac;

            // Optional crossfade near loop boundary in Classic mode
            if self.mode == SimplerPlaybackMode::Classic
                && self.is_looping
                && self.crossfade_pct > 0.0
            {
                let loop_len = (voice.end_sample - voice.start_sample) as f64;
                let xfade_span = (loop_len * self.crossfade_pct as f64 * 0.25).max(1.0);
                let dist_to_end = voice.end_sample as f64 - voice.playhead;
                if dist_to_end < xfade_span {
                    let blend = (dist_to_end / xfade_span) as f32; // 1.0 down to 0.0
                    let loop_head = voice.start_sample as f64 + (xfade_span - dist_to_end);
                    let l_idx0 = (loop_head as usize).clamp(0, total_samples - 1);
                    let l_idx1 = (l_idx0 + 1).min(total_samples - 1);
                    let l_frac = (loop_head - l_idx0 as f64) as f32;
                    let loop_sl = sample.samples_l[l_idx0] * (1.0 - l_frac)
                        + sample.samples_l[l_idx1] * l_frac;
                    let loop_sr = sample.samples_r[l_idx0] * (1.0 - l_frac)
                        + sample.samples_r[l_idx1] * l_frac;
                    sl = sl * blend + loop_sl * (1.0 - blend);
                    sr = sr * blend + loop_sr * (1.0 - blend);
                }
            }

            voice.playhead += voice.speed;
            let gain = voice.env_level * voice.velocity;
            mix_l += sl * gain;
            mix_r += sr * gain;
            any_active = true;
        }

        self.is_active = any_active;

        if !any_active {
            return (0.0, 0.0);
        }

        self.filter
            .set_params(self.filter_cutoff, 0.0, self.filter_resonance);
        self.filter.process_stereo(mix_l, mix_r)
    }
}
