use crate::biquad::{BiquadFilter, FilterType};
use livecraft_model::ids::DeviceId;

/// Live 12 EQ Three effect
/// 3-band DJ style isolator equalizer with Low, Mid, High band gains and kills
pub struct EqThreeEffect {
    pub device_id: DeviceId,
    pub is_enabled: bool,
    pub sample_rate: f32,
    pub low_gain_db: f32,
    pub mid_gain_db: f32,
    pub high_gain_db: f32,
    pub low_freq: f32,
    pub high_freq: f32,
    pub low_on: bool,
    pub mid_on: bool,
    pub high_on: bool,
    low_filter: BiquadFilter,
    high_filter: BiquadFilter,
}

impl EqThreeEffect {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1000.0);
        let low_f = 250.0;
        let high_f = 2500.0;
        Self {
            device_id: DeviceId(0),
            is_enabled: true,
            sample_rate: sr,
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            high_gain_db: 0.0,
            low_freq: low_f,
            high_freq: high_f,
            low_on: true,
            mid_on: true,
            high_on: true,
            low_filter: BiquadFilter::new(FilterType::LowPass, sr, low_f, 0.0, 0.707),
            high_filter: BiquadFilter::new(FilterType::HighPass, sr, high_f, 0.0, 0.707),
        }
    }

    pub fn reset(&mut self) {
        self.low_filter.reset_state();
        self.high_filter.reset_state();
    }

    pub fn update_filters(&mut self) {
        self.low_filter
            .set_params(self.low_freq.clamp(50.0, 1000.0), 0.0, 0.707);
        self.high_filter
            .set_params(self.high_freq.clamp(1000.0, 10000.0), 0.0, 0.707);
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if !self.is_enabled {
            return;
        }
        self.update_filters();

        let g_low = if self.low_on {
            10.0f32.powf(self.low_gain_db / 20.0)
        } else {
            0.0
        };
        let g_mid = if self.mid_on {
            10.0f32.powf(self.mid_gain_db / 20.0)
        } else {
            0.0
        };
        let g_high = if self.high_on {
            10.0f32.powf(self.high_gain_db / 20.0)
        } else {
            0.0
        };

        let num_samples = left.len().min(right.len());
        for i in 0..num_samples {
            let in_l = left[i];
            let in_r = right[i];

            let (low_l, low_r) = self.low_filter.process_stereo(in_l, in_r);
            let (high_l, high_r) = self.high_filter.process_stereo(in_l, in_r);

            let mid_l = in_l - low_l - high_l;
            let mid_r = in_r - low_r - high_r;

            left[i] = low_l * g_low + mid_l * g_mid + high_l * g_high;
            right[i] = low_r * g_low + mid_r * g_mid + high_r * g_high;
        }
    }
}
