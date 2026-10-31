use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;

#[derive(Debug, Clone)]
pub struct CompressorEffect {
    pub device_id: DeviceId,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_gain_db: f32,
    pub knee_db: f32,
    pub is_enabled: bool,
    sample_rate: f32,
    envelope: f32,
    pub gain_reduction_db: f32, // For visual meters
}

impl CompressorEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            device_id: DeviceId(0),
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 15.0,
            release_ms: 100.0,
            makeup_gain_db: 2.0,
            knee_db: 3.0,
            is_enabled: true,
            sample_rate,
            envelope: 0.0,
            gain_reduction_db: 0.0,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.threshold_db = dev.get_param("threshold_db");
        self.ratio = dev.get_param("ratio").max(1.0);
        self.attack_ms = dev.get_param("attack_ms").max(0.1);
        self.release_ms = dev.get_param("release_ms").max(1.0);
        self.makeup_gain_db = dev.get_param("makeup_gain_db");
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            self.gain_reduction_db = 0.0;
            return (in_l, in_r);
        }

        let peak = in_l.abs().max(in_r.abs()).max(1e-6);
        let peak_db = 20.0 * peak.log10();

        // Target gain in dB
        let diff = peak_db - self.threshold_db;
        let mut target_gain_db = 0.0;

        if diff > 0.0 {
            // Above threshold
            let compressed_over = diff / self.ratio;
            target_gain_db = compressed_over - diff; // Negative dB reduction
        }

        // Attack & Release ballistics
        let attack_coeff = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate).max(1.0)).exp();
        let release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate).max(1.0)).exp();

        let coeff = if target_gain_db < self.envelope {
            attack_coeff
        } else {
            release_coeff
        };

        self.envelope = target_gain_db + coeff * (self.envelope - target_gain_db);
        self.gain_reduction_db = -self.envelope; // Positive value representing reduction

        let gain_lin = 10.0f32.powf((self.envelope + self.makeup_gain_db) / 20.0);

        (in_l * gain_lin, in_r * gain_lin)
    }
}
