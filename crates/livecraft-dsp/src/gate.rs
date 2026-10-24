/// Live 12 Noise Gate effect
/// Passes only signals whose level exceeds a user-specified threshold,
/// with hysteresis (Return), attack, hold, release, and floor controls.
pub struct GateEffect {
    pub sample_rate: f32,
    pub threshold_db: f32,
    pub return_db: f32, // Hysteresis (level difference to close gate)
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
    pub floor_db: f32,
    is_open: bool,
    hold_samples_remaining: usize,
    current_gain: f32,
}

impl GateEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1000.0),
            threshold_db: -30.0,
            return_db: 3.0,
            attack_ms: 1.0,
            hold_ms: 50.0,
            release_ms: 100.0,
            floor_db: -60.0,
            is_open: false,
            hold_samples_remaining: 0,
            current_gain: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.is_open = false;
        self.hold_samples_remaining = 0;
        self.current_gain = 0.0;
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let thresh_linear = 10.0f32.powf(self.threshold_db / 20.0);
        let close_linear = 10.0f32.powf((self.threshold_db - self.return_db.abs()) / 20.0);
        let floor_linear = 10.0f32.powf(self.floor_db / 20.0);

        let attack_coeff = (-1.0 / (self.attack_ms.max(0.1) * 0.001 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release_ms.max(1.0) * 0.001 * self.sample_rate)).exp();
        let hold_total_samples = (self.hold_ms.max(0.0) * 0.001 * self.sample_rate) as usize;

        let num_samples = left.len().min(right.len());
        for i in 0..num_samples {
            let input_peak = left[i].abs().max(right[i].abs());

            if !self.is_open {
                if input_peak >= thresh_linear {
                    self.is_open = true;
                    self.hold_samples_remaining = hold_total_samples;
                }
            } else if input_peak < close_linear {
                if self.hold_samples_remaining > 0 {
                    self.hold_samples_remaining -= 1;
                } else {
                    self.is_open = false;
                }
            } else {
                // Signal is still above threshold, replenish hold counter
                self.hold_samples_remaining = hold_total_samples;
            }

            let target_gain = if self.is_open { 1.0 } else { floor_linear };
            if target_gain > self.current_gain {
                self.current_gain = target_gain + attack_coeff * (self.current_gain - target_gain);
            } else {
                self.current_gain = target_gain + release_coeff * (self.current_gain - target_gain);
            }

            left[i] *= self.current_gain;
            right[i] *= self.current_gain;
        }
    }
}
