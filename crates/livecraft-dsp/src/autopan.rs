use std::f32::consts::PI;

/// Live 12 Auto Pan / Tremolo effect
/// Uses LFO modulation to affect the stereo position or amplitude
pub struct AutoPanEffect {
    pub sample_rate: f32,
    pub amount: f32,    // 0.0 to 1.0
    pub rate_hz: f32,   // 0.01 to 40.0 Hz
    pub phase_deg: f32, // Stereo phase offset (0.0 to 360.0 degrees)
    pub shape: f32,     // 0.0 = Sine, 1.0 = Triangle
    pub mode: f32,      // 0.0 = Panning, 1.0 = Tremolo
    phase: f32,
}

impl AutoPanEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1000.0),
            amount: 1.0,
            rate_hz: 1.0,
            phase_deg: 180.0,
            shape: 0.0,
            mode: 0.0,
            phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    #[inline]
    fn compute_lfo(&self, phi: f32) -> f32 {
        if self.shape >= 0.5 {
            // Triangle waveform: range -1.0 to 1.0
            let norm = (phi / (2.0 * PI)).fract();
            if norm < 0.5 {
                4.0 * norm - 1.0
            } else {
                3.0 - 4.0 * norm
            }
        } else {
            // Sine waveform
            phi.sin()
        }
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let phase_inc = (2.0 * PI * self.rate_hz) / self.sample_rate;
        let stereo_offset = (self.phase_deg * PI) / 180.0;
        let is_tremolo = self.mode >= 0.5;

        let num_samples = left.len().min(right.len());
        for i in 0..num_samples {
            let lfo_l = self.compute_lfo(self.phase);
            if is_tremolo {
                let mod_gain = 1.0 - self.amount * 0.5 * (1.0 + lfo_l);
                left[i] *= mod_gain;
                right[i] *= mod_gain;
            } else {
                let lfo_r = self.compute_lfo(self.phase + stereo_offset);
                let gain_l = 1.0 - self.amount * 0.5 * (1.0 + lfo_l);
                let gain_r = 1.0 - self.amount * 0.5 * (1.0 + lfo_r);
                left[i] *= gain_l;
                right[i] *= gain_r;
            }

            self.phase += phase_inc;
            if self.phase >= 2.0 * PI {
                self.phase -= 2.0 * PI;
            }
        }
    }
}
