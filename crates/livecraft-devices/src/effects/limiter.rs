use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;

#[derive(Debug, Clone)]
pub struct LimiterEffect {
    pub device_id: DeviceId,
    pub ceiling_db: f32,
    pub gain_db: f32,
    pub release_ms: f32,
    pub is_enabled: bool,
    sample_rate: f32,
    envelope: f32,
}

impl LimiterEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            device_id: DeviceId(0),
            ceiling_db: -0.3,
            gain_db: 0.0,
            release_ms: 100.0,
            is_enabled: true,
            sample_rate,
            envelope: 1.0,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.ceiling_db = dev.get_param("ceiling_db");
        self.gain_db = dev.get_param("gain_db");
        self.release_ms = dev.get_param("release_ms").clamp(1.0, 1000.0);
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let input_gain = 10.0f32.powf(self.gain_db / 20.0);
        let ceiling = 10.0f32.powf(self.ceiling_db / 20.0);

        let boosted_l = in_l * input_gain;
        let boosted_r = in_r * input_gain;
        let peak = boosted_l.abs().max(boosted_r.abs());

        let target_gain = if peak > ceiling { ceiling / peak } else { 1.0 };

        let release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate).max(1.0)).exp();
        if target_gain < self.envelope {
            self.envelope = target_gain; // Instant attack to prevent clipping
        } else {
            self.envelope = target_gain + release_coeff * (self.envelope - target_gain);
        }

        (boosted_l * self.envelope, boosted_r * self.envelope)
    }
}

impl DeviceProcessor for LimiterEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Limiter table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.ceiling_db = value,
            1 => self.gain_db = value,
            2 => self.release_ms = value.clamp(1.0, 1000.0),
            _ => {}
        }
    }

    /// Returns the envelope to its starting value, as `new` sets it.
    fn reset(&mut self) {
        self.envelope = 1.0;
    }
}

#[derive(Debug, Clone)]
pub struct UtilityEffect {
    pub device_id: DeviceId,
    pub gain_db: f32,
    pub pan: f32,   // -1.0 to +1.0
    pub width: f32, // 0.0 (Mono) to 1.0 (Normal) to 2.0+ (Wide)
    pub mute: bool,
    pub invert_l: bool,
    pub invert_r: bool,
    pub is_enabled: bool,
}

impl Default for UtilityEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl UtilityEffect {
    pub fn new() -> Self {
        Self {
            device_id: DeviceId(0),
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            mute: false,
            invert_l: false,
            invert_r: false,
            is_enabled: true,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.gain_db = dev.get_param("gain_db");
        self.pan = dev.get_param("pan").clamp(-1.0, 1.0);
        self.width = dev.get_param("width").clamp(0.0, 4.0);
        self.mute = dev.get_param("mute") > 0.5;
    }

    pub fn process_stereo(&self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled || self.mute {
            return (0.0, 0.0);
        }

        let mut l = if self.invert_l { -in_l } else { in_l };
        let mut r = if self.invert_r { -in_r } else { in_r };

        // Mid/Side stereo width processing
        let mid = (l + r) * 0.5;
        let side = (l - r) * 0.5 * self.width;
        l = mid + side;
        r = mid - side;

        // Panning (constant-power)
        let pan_norm = (self.pan + 1.0) * 0.5; // 0.0 to 1.0
        let angle = pan_norm * std::f32::consts::FRAC_PI_2;
        let pan_l = angle.cos();
        let pan_r = angle.sin();

        let gain_lin = 10.0f32.powf(self.gain_db / 20.0);

        (l * pan_l * gain_lin, r * pan_r * gain_lin)
    }
}

impl DeviceProcessor for UtilityEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Utility table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.gain_db = value,
            1 => self.pan = value.clamp(-1.0, 1.0),
            2 => self.width = value.clamp(0.0, 4.0),
            3 => self.mute = value > 0.5,
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
    fn limiter_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || LimiterEffect::new(44100.0),
            &[(0, -6.0), (1, 3.0)],
        );
    }

    #[test]
    fn limiter_ceiling_changes_the_output() {
        assert_parameter_changes_output(|| LimiterEffect::new(44100.0), 0, -20.0);
    }
}
