use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;

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

impl DeviceProcessor for CompressorEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Compressor table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.threshold_db = value,
            1 => self.ratio = value.max(1.0),
            2 => self.attack_ms = value.max(0.1),
            3 => self.release_ms = value.max(1.0),
            4 => self.makeup_gain_db = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.gain_reduction_db = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn compressor_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || CompressorEffect::new(44100.0),
            &[(0, -30.0), (1, 4.0)],
        );
    }

    #[test]
    fn compressor_threshold_changes_the_output() {
        assert_parameter_changes_output(|| CompressorEffect::new(44100.0), 0, -50.0);
    }
}
