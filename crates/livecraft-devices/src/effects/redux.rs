use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::param::ParamId;
/// Live 12 Redux effect
/// Digital downsampling (sample rate reduction) and bit reduction (quantization distortion).
pub struct ReduxEffect {
    pub device_id: DeviceId,
    pub is_enabled: bool,
    pub sample_rate: f32,
    pub rate_factor: f32, // Downsample ratio (1.0 = none, up to 32.0)
    pub bits: f32,        // Bit depth (1.0 to 16.0 bits)
    pub jitter: f32,      // Downsample clock jitter (0.0 to 1.0)
    pub dry_wet: f32,     // 0.0 to 1.0
    counter_l: f32,
    counter_r: f32,
    held_sample_l: f32,
    held_sample_r: f32,
    rng_state: u32,
}

impl ReduxEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            device_id: DeviceId(0),
            is_enabled: true,
            sample_rate: sample_rate.max(1000.0),
            rate_factor: 1.0,
            bits: 16.0,
            jitter: 0.0,
            dry_wet: 1.0,
            counter_l: 0.0,
            counter_r: 0.0,
            held_sample_l: 0.0,
            held_sample_r: 0.0,
            rng_state: 123456789,
        }
    }

    pub fn reset(&mut self) {
        self.counter_l = 0.0;
        self.counter_r = 0.0;
        self.held_sample_l = 0.0;
        self.held_sample_r = 0.0;
        self.rng_state = 123456789;
    }

    #[inline]
    fn next_rand(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state as f32 / 4294967296.0) * 2.0 - 1.0
    }

    #[inline]
    fn quantize_bits(&self, sample: f32, bits: f32) -> f32 {
        if bits >= 16.0 {
            return sample;
        }
        let levels = 2.0f32.powf(bits.max(1.0));
        let half_levels = levels * 0.5;
        (sample.clamp(-1.0, 1.0) * half_levels).round() / half_levels
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if !self.is_enabled {
            return;
        }
        let base_rate = self.rate_factor.max(1.0);
        let bits = self.bits.clamp(1.0, 16.0);
        let dry_wet = self.dry_wet.clamp(0.0, 1.0);

        let num_samples = left.len().min(right.len());
        for i in 0..num_samples {
            let in_l = left[i];
            let in_r = right[i];

            // Downsampling sample-and-hold with optional jitter
            self.counter_l += 1.0;
            let jitter_offset_l = if self.jitter > 0.0 {
                self.next_rand() * self.jitter * (base_rate * 0.5)
            } else {
                0.0
            };
            if self.counter_l >= (base_rate + jitter_offset_l).max(1.0) {
                self.held_sample_l = in_l;
                self.counter_l = 0.0;
            }

            self.counter_r += 1.0;
            let jitter_offset_r = if self.jitter > 0.0 {
                self.next_rand() * self.jitter * (base_rate * 0.5)
            } else {
                0.0
            };
            if self.counter_r >= (base_rate + jitter_offset_r).max(1.0) {
                self.held_sample_r = in_r;
                self.counter_r = 0.0;
            }

            // Bit reduction quantization
            let bit_l = self.quantize_bits(self.held_sample_l, bits);
            let bit_r = self.quantize_bits(self.held_sample_r, bits);

            // Dry/Wet blend
            left[i] = (1.0 - dry_wet) * in_l + dry_wet * bit_l;
            right[i] = (1.0 - dry_wet) * in_r + dry_wet * bit_r;
        }
    }
}

impl DeviceProcessor for ReduxEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        ReduxEffect::process(self, io.left, io.right);
    }

    /// Ids are positions in the Redux table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.rate_factor = value.clamp(1.0, 32.0),
            1 => self.bits = value.clamp(1.0, 16.0),
            2 => self.jitter = value.clamp(0.0, 1.0),
            3 => self.dry_wet = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        ReduxEffect::reset(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn redux_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || ReduxEffect::new(44100.0),
            &[(0, 4.0), (1, 6.0), (2, 0.2), (3, 0.8)],
        );
    }

    #[test]
    fn redux_bit_depth_changes_the_output() {
        assert_parameter_changes_output(|| ReduxEffect::new(44100.0), 1, 4.0);
    }
}
