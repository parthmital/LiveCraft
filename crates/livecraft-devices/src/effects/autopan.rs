use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::param::ParamId;
use std::f32::consts::PI;

/// Live 12 Auto Pan / Tremolo effect
/// Uses LFO modulation to affect the stereo position or amplitude
pub struct AutoPanEffect {
    pub device_id: DeviceId,
    pub is_enabled: bool,
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
            device_id: DeviceId(0),
            is_enabled: true,
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
        if !self.is_enabled {
            return;
        }
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

impl DeviceProcessor for AutoPanEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        AutoPanEffect::process(self, io.left, io.right);
    }

    /// Ids are positions in the Auto Pan table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.amount = value.clamp(0.0, 1.0),
            1 => self.rate_hz = value.clamp(0.01, 40.0),
            2 => self.phase_deg = value.clamp(0.0, 360.0),
            3 => self.shape = value,
            4 => self.mode = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        AutoPanEffect::reset(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn auto_pan_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || AutoPanEffect::new(44100.0),
            &[(0, 0.6), (1, 3.0), (2, 90.0)],
        );
    }

    #[test]
    fn auto_pan_amount_changes_the_output() {
        assert_parameter_changes_output(|| AutoPanEffect::new(44100.0), 0, 0.2);
    }
}
