use crate::biquad::{BiquadFilter, FilterType};
use livecraft_model::model::Device;

#[derive(Debug, Clone)]
pub struct EqEightBand {
    pub filter: BiquadFilter,
    pub is_enabled: bool,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub filter_type: FilterType,
}

#[derive(Debug, Clone)]
pub struct EqEight {
    pub device_id: u64,
    pub bands: Vec<EqEightBand>,
    pub sample_rate: f32,
    pub is_enabled: bool,
}

impl EqEight {
    pub fn new(sample_rate: f32) -> Self {
        // Ableton 8 default bands
        let default_types = [
            FilterType::HighPass,  // Band 1: Highpass 30 Hz
            FilterType::LowShelf,  // Band 2: Low Shelf 100 Hz
            FilterType::Peaking,   // Band 3: 250 Hz
            FilterType::Peaking,   // Band 4: 600 Hz
            FilterType::Peaking,   // Band 5: 1500 Hz
            FilterType::Peaking,   // Band 6: 4000 Hz
            FilterType::HighShelf, // Band 7: High Shelf 8000 Hz
            FilterType::LowPass,   // Band 8: Lowpass 18000 Hz
        ];
        let default_freqs = [30.0, 100.0, 250.0, 600.0, 1500.0, 4000.0, 8000.0, 18000.0];

        let mut bands = Vec::with_capacity(8);
        for i in 0..8 {
            let filter =
                BiquadFilter::new(default_types[i], sample_rate, default_freqs[i], 0.0, 0.707);
            bands.push(EqEightBand {
                filter,
                is_enabled: true,
                freq: default_freqs[i],
                gain_db: 0.0,
                q: 0.707,
                filter_type: default_types[i],
            });
        }

        Self {
            device_id: 0,
            bands,
            sample_rate,
            is_enabled: true,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        for i in 1..=8 {
            let freq = dev.get_param(&format!("band_{}_freq", i));
            let gain = dev.get_param(&format!("band_{}_gain", i));
            let q = dev.get_param(&format!("band_{}_q", i));
            let en = dev.get_param(&format!("band_{}_enabled", i)) > 0.5;
            if freq > 0.0 {
                self.set_band_params(i - 1, freq, gain, q);
            }
            self.set_band_enabled(i - 1, en);
        }
    }

    pub fn set_band_type(&mut self, band_idx: usize, filter_type: FilterType) {
        if let Some(band) = self.bands.get_mut(band_idx) {
            band.filter_type = filter_type;
            band.filter.set_type(filter_type);
        }
    }

    pub fn set_band_params(&mut self, band_idx: usize, freq: f32, gain_db: f32, q: f32) {
        if let Some(band) = self.bands.get_mut(band_idx) {
            band.freq = freq;
            band.gain_db = gain_db;
            band.q = q;
            band.filter.set_params(freq, gain_db, q);
        }
    }

    pub fn set_band_enabled(&mut self, band_idx: usize, enabled: bool) {
        if let Some(band) = self.bands.get_mut(band_idx) {
            band.is_enabled = enabled;
            if !enabled {
                band.filter.reset_state();
            }
        }
    }

    pub fn magnitude_at_freq(&self, freq_hz: f32) -> f32 {
        if !self.is_enabled {
            return 1.0;
        }
        let mut total_mag = 1.0f32;
        for band in &self.bands {
            if band.is_enabled {
                total_mag *= band.filter.magnitude_response(freq_hz);
            }
        }
        total_mag
    }

    pub fn gain_db_at_freq(&self, freq_hz: f32) -> f32 {
        let mag = self.magnitude_at_freq(freq_hz).max(1e-5);
        20.0 * mag.log10()
    }

    pub fn response_curve_points(&self, num_points: usize) -> Vec<(f32, f32)> {
        let num = num_points.max(2);
        let min_freq = 20.0f32;
        let max_freq = (self.sample_rate * 0.49).min(20000.0);
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        (0..num)
            .map(|i| {
                let t = i as f32 / (num - 1) as f32;
                let freq = (log_min + t * (log_max - log_min)).exp();
                let gain_db = self.gain_db_at_freq(freq);
                (freq, gain_db)
            })
            .collect()
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let mut l = in_l;
        let mut r = in_r;
        for band in &mut self.bands {
            if band.is_enabled {
                let (out_l, out_r) = band.filter.process_stereo(l, r);
                l = out_l;
                r = out_r;
            }
        }
        (l, r)
    }
}
