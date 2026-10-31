use crate::biquad::{BiquadFilter, FilterType};
use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;

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
    pub device_id: DeviceId,
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
            device_id: DeviceId(0),
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

impl DeviceProcessor for EqEight {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the EQ Eight table: four per band, in the order frequency, gain, Q,
    /// and enabled. Band 1 is id 0 to 3.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        let band_idx = usize::from(id.0 / 4);
        if id.0 % 4 == 3 {
            self.set_band_enabled(band_idx, value > 0.5);
            return;
        }
        let Some(band) = self.bands.get_mut(band_idx) else {
            return;
        };
        match id.0 % 4 {
            0 => band.freq = value,
            1 => band.gain_db = value,
            _ => band.q = value,
        }
        band.filter.set_params(band.freq, band.gain_db, band.q);
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.filter.reset_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn eq_eight_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || EqEight::new(44100.0),
            &[(0, 200.0), (1, 3.0), (2, 1.2), (8, 1000.0), (9, -4.0)],
        );
    }

    #[test]
    fn eq_eight_band_gain_changes_the_output() {
        // Id 5 is band 2's gain. Band 1 is a high-pass, which has no gain.
        assert_parameter_changes_output(|| EqEight::new(44100.0), 5, 6.0);
    }

    #[test]
    fn eq_eight_band_enabled_flag_bypasses_the_band() {
        assert_parameter_changes_output(|| EqEight::new(44100.0), 3, 0.0);
    }
}
