use crate::biquad::{BiquadFilter, FilterType};
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

#[derive(Debug, Clone)]
struct Voice {
    note: u8,
    phase1: f32,
    phase2: f32,
    stage: EnvStage,
    env_level: f32,
    filter: BiquadFilter,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        Self {
            note: 0,
            phase1: 0.0,
            phase2: 0.0,
            stage: EnvStage::Off,
            env_level: 0.0,
            filter: BiquadFilter::new(FilterType::LowPass, sample_rate, 2500.0, 0.0, 1.2),
        }
    }

    fn note_on(&mut self, note: u8) {
        self.note = note;
        self.phase1 = 0.0;
        self.phase2 = 0.0;
        self.stage = EnvStage::Attack;
        self.filter.reset_state();
    }

    fn note_off(&mut self) {
        if self.stage != EnvStage::Off {
            self.stage = EnvStage::Release;
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperatorSynth {
    voices: Vec<Voice>,
    sample_rate: f32,
    pub osc1_wave: Waveform,
    pub osc1_level: f32,
    pub osc2_wave: Waveform,
    pub osc2_level: f32,
    pub osc2_coarse: f32, // Frequency ratio multiplier (1.0, 2.0, 3.0...)
    pub envelope: AdsrEnvelope,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub is_enabled: bool,
}

impl OperatorSynth {
    pub fn new(sample_rate: f32) -> Self {
        let max_voices = 16;
        let voices = (0..max_voices).map(|_| Voice::new(sample_rate)).collect();

        Self {
            voices,
            sample_rate,
            osc1_wave: Waveform::Sine,
            osc1_level: 0.8,
            osc2_wave: Waveform::Sawtooth,
            osc2_level: 0.4,
            osc2_coarse: 2.0,
            envelope: AdsrEnvelope::default(),
            filter_cutoff: 3500.0,
            filter_resonance: 1.5,
            is_enabled: true,
        }
    }

    pub fn note_on(&mut self, note: u8, _velocity: u8) {
        // Find an inactive voice, or oldest release voice
        let voice_idx = self
            .voices
            .iter()
            .position(|v| v.stage == EnvStage::Off)
            .or_else(|| {
                self.voices
                    .iter()
                    .position(|v| v.stage == EnvStage::Release)
            })
            .unwrap_or(0);

        self.voices[voice_idx].note_on(note);
    }

    pub fn note_off(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.note == note
                && voice.stage != EnvStage::Off
                && voice.stage != EnvStage::Release
            {
                voice.note_off();
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.stage = EnvStage::Off;
            voice.env_level = 0.0;
        }
    }

    fn render_waveform(wave: Waveform, phase: f32) -> f32 {
        match wave {
            Waveform::Sine => (phase * 2.0 * PI).sin(),
            Waveform::Triangle => {
                let p = phase - phase.floor();
                4.0 * (p - 0.5).abs() - 1.0
            }
            Waveform::Sawtooth => {
                let p = phase - phase.floor();
                2.0 * p - 1.0
            }
            Waveform::Square => {
                let p = phase - phase.floor();
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Noise => {
                // Pseudo-random noise
                ((phase * 123_456.79).sin() * 43_758.547).fract() * 2.0 - 1.0
            }
        }
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_enabled {
            return (0.0, 0.0);
        }

        let mut mix = 0.0;
        let dt = 1.0 / self.sample_rate;

        for voice in &mut self.voices {
            if voice.stage == EnvStage::Off {
                continue;
            }

            // Envelope state machine
            match voice.stage {
                EnvStage::Attack => {
                    let rate = dt / self.envelope.attack_sec.max(0.001);
                    voice.env_level += rate;
                    if voice.env_level >= 1.0 {
                        voice.env_level = 1.0;
                        voice.stage = EnvStage::Decay;
                    }
                }
                EnvStage::Decay => {
                    let rate = dt / self.envelope.decay_sec.max(0.001);
                    voice.env_level -= rate * (1.0 - self.envelope.sustain_level);
                    if voice.env_level <= self.envelope.sustain_level {
                        voice.env_level = self.envelope.sustain_level;
                        voice.stage = EnvStage::Sustain;
                    }
                }
                EnvStage::Sustain => {
                    voice.env_level = self.envelope.sustain_level;
                }
                EnvStage::Release => {
                    let rate = dt / self.envelope.release_sec.max(0.001);
                    voice.env_level -= rate;
                    if voice.env_level <= 0.0001 {
                        voice.env_level = 0.0;
                        voice.stage = EnvStage::Off;
                        continue;
                    }
                }
                EnvStage::Off => continue,
            }

            // MIDI note to frequency: f = 440 * 2^((note - 69) / 12)
            let freq = 440.0 * 2.0f32.powf((voice.note as f32 - 69.0) / 12.0);

            // Osc 1
            let s1 = Self::render_waveform(self.osc1_wave, voice.phase1) * self.osc1_level;
            voice.phase1 = (voice.phase1 + freq * dt) % 1.0;

            // Osc 2 (with coarse ratio multiplier)
            let freq2 = freq * self.osc2_coarse.max(0.25);
            let s2 = Self::render_waveform(self.osc2_wave, voice.phase2) * self.osc2_level;
            voice.phase2 = (voice.phase2 + freq2 * dt) % 1.0;

            let raw_sample = (s1 + s2) * voice.env_level;

            // Filter
            voice
                .filter
                .set_params(self.filter_cutoff, 0.0, self.filter_resonance);
            let (filtered, _) = voice.filter.process_stereo(raw_sample, raw_sample);

            mix += filtered;
        }

        (mix * 0.25, mix * 0.25)
    }
}
