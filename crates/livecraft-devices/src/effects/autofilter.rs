use crate::biquad::{BiquadFilter, FilterType};
use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoFilterMode {
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

#[derive(Debug, Clone)]
pub struct AutoFilterEffect {
    pub device_id: DeviceId,
    pub mode: AutoFilterMode,
    pub cutoff: f32,
    pub resonance: f32,
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub envelope_amount: f32,
    pub is_enabled: bool,
    filter_l: BiquadFilter,
    filter_r: BiquadFilter,
    sample_rate: f32,
    lfo_phase: f32,
    envelope: f32,
}

impl AutoFilterEffect {
    pub fn new(sample_rate: f32) -> Self {
        let filter_l = BiquadFilter::new(FilterType::LowPass, sample_rate, 1200.0, 0.0, 1.5);
        let filter_r = BiquadFilter::new(FilterType::LowPass, sample_rate, 1200.0, 0.0, 1.5);

        Self {
            device_id: DeviceId(0),
            mode: AutoFilterMode::LowPass,
            cutoff: 1200.0,
            resonance: 1.5,
            lfo_rate: 1.0,
            lfo_depth: 0.4,
            envelope_amount: 0.3,
            is_enabled: true,
            filter_l,
            filter_r,
            sample_rate,
            lfo_phase: 0.0,
            envelope: 0.0,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        let m = dev.get_param("filter_type").round() as i32;
        self.mode = match m {
            0 => AutoFilterMode::LowPass,
            1 => AutoFilterMode::HighPass,
            2 => AutoFilterMode::BandPass,
            3 => AutoFilterMode::Notch,
            _ => AutoFilterMode::LowPass,
        };
        self.cutoff = dev.get_param("cutoff").clamp(20.0, 20000.0);
        self.resonance = dev.get_param("resonance").clamp(0.1, 15.0);
        self.lfo_rate = dev.get_param("lfo_rate").clamp(0.05, 30.0);
        self.lfo_depth = dev.get_param("lfo_depth").clamp(0.0, 1.0);
        self.envelope_amount = dev.get_param("envelope_amount").clamp(-1.0, 1.0);
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        // LFO sine oscillator
        let lfo_val = (self.lfo_phase * 2.0 * PI).sin();
        let dt = 1.0 / self.sample_rate;
        self.lfo_phase = (self.lfo_phase + self.lfo_rate * dt).fract();

        // Envelope follower (peak detector)
        let peak = in_l.abs().max(in_r.abs());
        let attack_coeff = (-1.0 / (0.005 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (0.050 * self.sample_rate)).exp();
        let coeff = if peak > self.envelope {
            attack_coeff
        } else {
            release_coeff
        };
        self.envelope = peak + coeff * (self.envelope - peak);

        // Modulate cutoff frequency
        let mod_octaves =
            (lfo_val * self.lfo_depth * 2.5) + (self.envelope * self.envelope_amount * 3.5);
        let dynamic_cutoff = (self.cutoff * 2.0f32.powf(mod_octaves)).clamp(20.0, 20000.0);

        let filter_type = match self.mode {
            AutoFilterMode::LowPass => FilterType::LowPass,
            AutoFilterMode::HighPass => FilterType::HighPass,
            AutoFilterMode::BandPass => FilterType::BandPass,
            AutoFilterMode::Notch => FilterType::Notch,
        };

        self.filter_l.set_type(filter_type);
        self.filter_r.set_type(filter_type);
        self.filter_l
            .set_params(dynamic_cutoff, 0.0, self.resonance);
        self.filter_r
            .set_params(dynamic_cutoff, 0.0, self.resonance);

        let out_l = self.filter_l.process_stereo(in_l, in_l).0;
        let out_r = self.filter_r.process_stereo(in_r, in_r).1;

        (out_l, out_r)
    }
}

impl DeviceProcessor for AutoFilterEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Auto Filter table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => {
                self.mode = match value.round() as i32 {
                    1 => AutoFilterMode::HighPass,
                    2 => AutoFilterMode::BandPass,
                    3 => AutoFilterMode::Notch,
                    _ => AutoFilterMode::LowPass,
                };
            }
            1 => self.cutoff = value.clamp(20.0, 20000.0),
            2 => self.resonance = value.clamp(0.1, 10.0),
            3 => self.lfo_rate = value.clamp(0.01, 40.0),
            4 => self.lfo_depth = value.clamp(0.0, 1.0),
            5 => self.envelope_amount = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn auto_filter_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || AutoFilterEffect::new(44100.0),
            &[(1, 800.0), (2, 2.0), (3, 2.0)],
        );
    }

    #[test]
    fn auto_filter_cutoff_changes_the_output() {
        assert_parameter_changes_output(|| AutoFilterEffect::new(44100.0), 1, 200.0);
    }
}
