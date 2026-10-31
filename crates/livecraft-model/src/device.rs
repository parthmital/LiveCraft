use crate::ids::DeviceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub id: DeviceId,
    pub name: String,
    pub device_type: DeviceType,
    pub is_enabled: bool,
    pub parameters: HashMap<String, f32>,
}

impl Device {
    pub fn new(id: DeviceId, name: impl Into<String>, device_type: DeviceType) -> Self {
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
