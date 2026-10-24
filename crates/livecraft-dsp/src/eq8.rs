use crate::biquad::{BiquadFilter, FilterType};

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
            bands,
            sample_rate,
            is_enabled: true,
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
