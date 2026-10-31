use crate::biquad::{BiquadFilter, FilterType};
use livecraft_model::model::Device;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Triangle,
    Sawtooth,
    Square,
    Noise,
}

#[derive(Debug, Clone, Copy)]
pub struct AdsrEnvelope {
    pub attack_sec: f32,
    pub decay_sec: f32,
    pub sustain_level: f32, // 0.0 to 1.0
    pub release_sec: f32,
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self {
            attack_sec: 0.005,
            decay_sec: 0.150,
            sustain_level: 0.70,
            release_sec: 0.250,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct OperatorUnit {
    pub wave: Waveform,
    pub level: f32,
    pub coarse: f32, // Multiplier (0.5, 1, 2, 3...)
    pub fine: f32,
    pub phase: f32,
    pub envelope: AdsrEnvelope,
}

impl Default for OperatorUnit {
    fn default() -> Self {
        Self {
            wave: Waveform::Sine,
            level: 0.8,
            coarse: 1.0,
            fine: 0.0,
            phase: 0.0,
            envelope: AdsrEnvelope::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct VoiceOp {
    phase: f32,
    stage: EnvStage,
    env_level: f32,
}

impl VoiceOp {
    fn new() -> Self {
        Self {
            phase: 0.0,
            stage: EnvStage::Off,
            env_level: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct Voice {
    note: u8,
    pub velocity: f32,
    pub note_id: u64,
    age: u64,
    ops: [VoiceOp; 4],
    feedback_mem: f32,
    filter: BiquadFilter,
    pub freq_hz: f32,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        Self {
            note: 0,
            velocity: 1.0,
            note_id: 0,
            age: 0,
            ops: [VoiceOp::new(); 4],
            feedback_mem: 0.0,
            filter: BiquadFilter::new(FilterType::LowPass, sample_rate, 2500.0, 0.0, 1.2),
            freq_hz: 440.0,
        }
    }

    fn is_active(&self) -> bool {
        self.ops.iter().any(|op| op.stage != EnvStage::Off)
    }

    fn env_level(&self) -> f32 {
        self.ops[0].env_level
    }

    fn note_on(&mut self, note: u8, velocity: f32, note_id: u64, freq_hz: f32) {
        self.note = note;
        self.velocity = velocity;
        self.note_id = note_id;
        self.age = 0;
        self.freq_hz = freq_hz.clamp(20.0, 20000.0);
        self.feedback_mem = 0.0;
        for op in &mut self.ops {
            op.phase = 0.0;
            op.stage = EnvStage::Attack;
            if op.env_level > 0.5 {
                op.env_level = 0.5;
            }
        }
        self.filter.reset_state();
    }

    fn note_off(&mut self) {
        for op in &mut self.ops {
            if op.stage != EnvStage::Off {
                op.stage = EnvStage::Release;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperatorSynth {
    voices: Vec<Voice>,
    sample_rate: f32,
    pub ops: [OperatorUnit; 4],
    pub algorithm: u8,
    pub feedback: f32,
    pub osc1_wave: Waveform,
    pub osc1_level: f32,
    pub osc2_wave: Waveform,
    pub osc2_level: f32,
    pub osc2_coarse: f32,
    pub envelope: AdsrEnvelope,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub is_enabled: bool,
    note_counter: u64,
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
        EnvStage::Off => {}
    }
}

impl OperatorSynth {
    pub fn new(sample_rate: f32) -> Self {
        let max_voices = 16;
        let voices = (0..max_voices).map(|_| Voice::new(sample_rate)).collect();

        let op_a = OperatorUnit {
            wave: Waveform::Sine,
            level: 0.8,
            coarse: 1.0,
            fine: 0.0,
            phase: 0.0,
            envelope: AdsrEnvelope::default(),
        };
        let op_b = OperatorUnit {
            wave: Waveform::Sawtooth,
            level: 0.4,
            coarse: 2.0,
            fine: 0.0,
            phase: 0.0,
            envelope: AdsrEnvelope::default(),
        };
        let op_c = OperatorUnit {
            wave: Waveform::Sine,
            level: 0.0,
            coarse: 1.0,
            fine: 0.0,
            phase: 0.0,
            envelope: AdsrEnvelope::default(),
        };
        let op_d = OperatorUnit {
            wave: Waveform::Sine,
            level: 0.0,
            coarse: 1.0,
            fine: 0.0,
            phase: 0.0,
            envelope: AdsrEnvelope::default(),
        };

        Self {
            voices,
            sample_rate,
            ops: [op_a, op_b, op_c, op_d],
            algorithm: 1,
            feedback: 0.0,
            osc1_wave: Waveform::Sine,
            osc1_level: 0.8,
            osc2_wave: Waveform::Sawtooth,
            osc2_level: 0.4,
            osc2_coarse: 2.0,
            envelope: AdsrEnvelope::default(),
            filter_cutoff: 3500.0,
            filter_resonance: 1.5,
            is_enabled: true,
            note_counter: 0,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        if let Some(alg) = dev.param("algorithm") {
            self.algorithm = (alg.round() as u8).clamp(1, 11);
        }
        if let Some(fb) = dev.param("feedback") {
            self.feedback = fb.clamp(0.0, 1.0);
        }
        let w1 = dev.get_param("osc1_waveform").round() as i32;
        self.osc1_wave = match w1 {
            0 => Waveform::Sine,
            1 => Waveform::Sawtooth,
            2 => Waveform::Square,
            3 => Waveform::Triangle,
            4 => Waveform::Noise,
            _ => Waveform::Sine,
        };
        self.osc1_level = dev.get_param("osc1_level").clamp(0.0, 2.0);
        let w2 = dev.get_param("osc2_waveform").round() as i32;
        self.osc2_wave = match w2 {
            0 => Waveform::Sine,
            1 => Waveform::Sawtooth,
            2 => Waveform::Square,
            3 => Waveform::Triangle,
            4 => Waveform::Noise,
            _ => Waveform::Sawtooth,
        };
        self.osc2_level = dev.get_param("osc2_level").clamp(0.0, 2.0);
        self.osc2_coarse = dev.get_param("osc2_coarse").clamp(0.25, 16.0);
        self.filter_cutoff = dev.get_param("filter_cutoff").clamp(20.0, 20000.0);
        self.filter_resonance = dev.get_param("filter_resonance").clamp(0.1, 10.0);
        self.envelope.attack_sec = (dev.get_param("attack_ms") * 0.001).clamp(0.001, 10.0);
        self.envelope.decay_sec = (dev.get_param("decay_ms") * 0.001).clamp(0.001, 10.0);
        self.envelope.sustain_level = dev.get_param("sustain").clamp(0.0, 1.0);
        self.envelope.release_sec = (dev.get_param("release_ms") * 0.001).clamp(0.001, 10.0);

        self.ops[0].wave = self.osc1_wave;
        self.ops[0].level = self.osc1_level;
        self.ops[0].envelope = self.envelope;
        self.ops[1].wave = self.osc2_wave;
        self.ops[1].level = self.osc2_level;
        self.ops[1].coarse = self.osc2_coarse;
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let freq = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
        self.note_on_with_freq(note, velocity, freq);
    }

    pub fn note_on_with_freq(&mut self, note: u8, velocity: u8, freq_hz: f32) {
        let norm_vel = (velocity as f32 / 127.0).clamp(0.01, 1.0);
        self.note_counter = self.note_counter.wrapping_add(1);
        let note_id = self.note_counter;

        let voice_idx = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_active())
            .map(|(idx, _)| idx)
            .next()
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.ops[0].stage == EnvStage::Release)
                    .min_by(|(_, a), (_, b)| {
                        a.env_level()
                            .partial_cmp(&b.env_level())
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

        self.voices[voice_idx].note_on(note, norm_vel, note_id, freq_hz);
    }

    pub fn note_off(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.note == note && voice.is_active() {
                voice.note_off();
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            for op in &mut voice.ops {
                op.stage = EnvStage::Off;
                op.env_level = 0.0;
            }
        }
    }

    fn render_waveform(wave: Waveform, phase: f32) -> f32 {
        let p = (phase - phase.floor()).clamp(0.0, 1.0);
        match wave {
            Waveform::Sine => (p * 2.0 * PI).sin(),
            Waveform::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
            Waveform::Sawtooth => 2.0 * p - 1.0,
            Waveform::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Noise => ((p * 123_456.79).sin() * 43_758.547).fract() * 2.0 - 1.0,
        }
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_enabled {
            return (0.0, 0.0);
        }

        // Sync legacy parameters into Operator A and B
        self.ops[0].wave = self.osc1_wave;
        self.ops[0].level = self.osc1_level;
        self.ops[0].envelope = self.envelope;
        self.ops[1].wave = self.osc2_wave;
        self.ops[1].level = self.osc2_level;
        self.ops[1].coarse = self.osc2_coarse;

        let mut total_l = 0.0f32;
        let mut total_r = 0.0f32;
        let dt = 1.0 / self.sample_rate;
        let mod_scale = 2.0f32;

        for voice in &mut self.voices {
            if !voice.is_active() {
                continue;
            }
            voice.age = voice.age.saturating_add(1);

            // 1. Process envelopes for all 4 operators
            for (i, op_state) in voice.ops.iter_mut().enumerate() {
                process_env(
                    &mut op_state.stage,
                    &mut op_state.env_level,
                    &self.ops[i].envelope,
                    dt,
                );
            }

            // 2. Advance phase increments
            let base_freq = voice.freq_hz;
            let vel = voice.velocity;

            // Operator D with feedback
            let d_freq = base_freq * self.ops[3].coarse.max(0.1) * (1.0 + self.ops[3].fine * 0.01);
            voice.ops[3].phase = (voice.ops[3].phase + d_freq * dt) % 1.0;
            let fb_mod = (voice.feedback_mem * self.feedback * 1.5).clamp(-8.0, 8.0);
            let d_phase = (voice.ops[3].phase + fb_mod) % 1.0;
            let out_d = Self::render_waveform(self.ops[3].wave, d_phase)
                * self.ops[3].level
                * voice.ops[3].env_level
                * vel;
            voice.feedback_mem = out_d;

            // Helper to render operator i given a phase modulation offset
            let render_op = |op_idx: usize,
                             pm: f32,
                             v_ops: &mut [VoiceOp; 4],
                             ops_cfg: &[OperatorUnit; 4]|
             -> f32 {
                let freq_i = base_freq
                    * ops_cfg[op_idx].coarse.max(0.1)
                    * (1.0 + ops_cfg[op_idx].fine * 0.01);
                v_ops[op_idx].phase = (v_ops[op_idx].phase + freq_i * dt) % 1.0;
                let eff_phase = (v_ops[op_idx].phase + pm.clamp(-16.0, 16.0)) % 1.0;
                let raw = Self::render_waveform(ops_cfg[op_idx].wave, eff_phase);
                raw * ops_cfg[op_idx].level * v_ops[op_idx].env_level * vel
            };

            // 3. Routing Algorithms (1..=11)
            let carrier_mix = match self.algorithm {
                1 => {
                    // Alg 1: D -> C -> B -> A (Cascade FM)
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, out_c * mod_scale, &mut voice.ops, &self.ops);
                    render_op(0, out_b * mod_scale, &mut voice.ops, &self.ops)
                }
                2 => {
                    // Alg 2: (D + C) -> B -> A
                    let out_c = render_op(2, 0.0, &mut voice.ops, &self.ops);
                    let out_b =
                        render_op(1, (out_d + out_c) * mod_scale, &mut voice.ops, &self.ops);
                    render_op(0, out_b * mod_scale, &mut voice.ops, &self.ops)
                }
                3 => {
                    // Alg 3: (D -> C) + (B -> A)
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, out_b * mod_scale, &mut voice.ops, &self.ops);
                    (out_a + out_c) * 0.7
                }
                4 => {
                    // Alg 4: (D -> C -> B) + A
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, out_c * mod_scale, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, 0.0, &mut voice.ops, &self.ops);
                    (out_a + out_b) * 0.7
                }
                5 => {
                    // Alg 5: (D -> A) + (C -> B -> A)
                    let out_c = render_op(2, 0.0, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, out_c * mod_scale, &mut voice.ops, &self.ops);
                    render_op(0, (out_b + out_d) * mod_scale, &mut voice.ops, &self.ops)
                }
                6 => {
                    // Alg 6: D -> (C + B + A)
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, out_d * mod_scale, &mut voice.ops, &self.ops);
                    (out_a + out_b + out_c) * 0.5
                }
                7 => {
                    // Alg 7: (D -> C) + B + A
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, 0.0, &mut voice.ops, &self.ops);
                    (out_a + out_b + out_c) * 0.5
                }
                8 => {
                    // Alg 8: Pure additive: D + C + B + A
                    let out_c = render_op(2, 0.0, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, 0.0, &mut voice.ops, &self.ops);
                    (out_a + out_b + out_c + out_d) * 0.35
                }
                9 => {
                    // Alg 9: (D -> C -> A) + (B -> A)
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    render_op(0, (out_c + out_b) * mod_scale, &mut voice.ops, &self.ops)
                }
                10 => {
                    // Alg 10: (D -> C) + (B -> A) with D carrier
                    let out_c = render_op(2, out_d * mod_scale, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, out_b * mod_scale, &mut voice.ops, &self.ops);
                    (out_a + out_c + out_d * 0.5) * 0.5
                }
                _ => {
                    // Alg 11: Pure parallel with feedback on D
                    let out_c = render_op(2, 0.0, &mut voice.ops, &self.ops);
                    let out_b = render_op(1, 0.0, &mut voice.ops, &self.ops);
                    let out_a = render_op(0, 0.0, &mut voice.ops, &self.ops);
                    (out_a + out_b + out_c + out_d) * 0.35
                }
            };

            // 4. Filter with velocity modulation
            let dynamic_cutoff = (self.filter_cutoff * (0.3 + 0.7 * vel)).clamp(20.0, 20000.0);
            voice
                .filter
                .set_params(dynamic_cutoff, 0.0, self.filter_resonance);
            let (filtered, _) = voice.filter.process_stereo(carrier_mix, carrier_mix);

            total_l += filtered;
            total_r += filtered;
        }

        (total_l * 0.35, total_r * 0.35)
    }
}
