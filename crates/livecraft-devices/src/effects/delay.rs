use crate::biquad::{BiquadFilter, FilterType};
use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;
use livecraft_time::seconds_per_beat;

#[derive(Debug, Clone)]
pub struct DelayEffect {
    pub device_id: DeviceId,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    pub delay_time_ms: f32,
    pub feedback: f32,
    pub dry_wet: f32,
    pub ping_pong: bool,
    pub is_enabled: bool,
    filter_l: BiquadFilter,
    filter_r: BiquadFilter,
}

impl DelayEffect {
    pub fn new(sample_rate: f32) -> Self {
        // Up to 2.0 seconds of delay buffer
        let max_samples = (sample_rate * 2.0) as usize;
        let filter_l = BiquadFilter::new(FilterType::LowPass, sample_rate, 4500.0, 0.0, 0.707);
        let filter_r = BiquadFilter::new(FilterType::LowPass, sample_rate, 4500.0, 0.0, 0.707);

        Self {
            device_id: DeviceId(0),
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            delay_time_ms: 375.0, // 1/8th note at 120 bpm
            feedback: 0.40,
            dry_wet: 0.30,
            ping_pong: true,
            is_enabled: true,
            filter_l,
            filter_r,
        }
    }

    pub fn set_tempo_synced(&mut self, bpm: f32, subdivision: f32) {
        // e.g. subdivision 0.5 = 1/8th note
        let beat_sec = seconds_per_beat(f64::from(bpm)) as f32;
        self.delay_time_ms = (beat_sec * subdivision * 1000.0).clamp(1.0, 1900.0);
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.delay_time_ms = dev.get_param("time_ms").clamp(1.0, 1900.0);
        self.feedback = dev.get_param("feedback").clamp(0.0, 0.98);
        self.dry_wet = dev.get_param("dry_wet").clamp(0.0, 1.0);
        self.ping_pong = dev.get_param("ping_pong") > 0.5;
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let delay_samples = ((self.delay_time_ms / 1000.0) * self.sample_rate)
            .clamp(1.0, self.buffer_l.len() as f32 - 1.0) as usize;
        let buf_len = self.buffer_l.len();

        let read_pos = if self.write_pos >= delay_samples {
            self.write_pos - delay_samples
        } else {
            buf_len + self.write_pos - delay_samples
        };

        let delayed_l = self.buffer_l[read_pos];
        let delayed_r = self.buffer_r[read_pos];

        // Filter delayed signal
        let (filtered_l, filtered_r) = if self.ping_pong {
            (
                self.filter_l.process_stereo(delayed_r, delayed_r).0,
                self.filter_r.process_stereo(delayed_l, delayed_l).0,
            )
        } else {
            (
                self.filter_l.process_stereo(delayed_l, delayed_l).0,
                self.filter_r.process_stereo(delayed_r, delayed_r).0,
            )
        };

        // Feedback into buffer
        self.buffer_l[self.write_pos] = in_l + filtered_l * self.feedback.clamp(0.0, 0.98);
        self.buffer_r[self.write_pos] = in_r + filtered_r * self.feedback.clamp(0.0, 0.98);

        self.write_pos = (self.write_pos + 1) % buf_len;

        let out_l = in_l * (1.0 - self.dry_wet) + delayed_l * self.dry_wet;
        let out_r = in_r * (1.0 - self.dry_wet) + delayed_r * self.dry_wet;

        (out_l, out_r)
    }
}

impl DeviceProcessor for DelayEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Delay table of `livecraft_model::device`.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            0 => self.delay_time_ms = value.clamp(1.0, 1900.0),
            1 => self.feedback = value.clamp(0.0, 0.98),
            2 => self.dry_wet = value.clamp(0.0, 1.0),
            3 => self.ping_pong = value > 0.5,
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn tail_samples(&self) -> Option<u64> {
        if !self.is_enabled
            || !self.dry_wet.is_finite()
            || self.dry_wet <= 0.0
            || !self.sample_rate.is_finite()
            || self.sample_rate <= 0.0
            || !self.delay_time_ms.is_finite()
        {
            return None;
        }
        let max_delay = self.buffer_l.len().saturating_sub(1);
        if max_delay == 0 {
            return None;
        }
        let delay_samples = ((f64::from(self.delay_time_ms) / 1000.0)
            * f64::from(self.sample_rate))
        .clamp(1.0, max_delay as f64);
        let fb = if self.feedback.is_finite() {
            f64::from(self.feedback).clamp(0.0, 0.98)
        } else {
            0.0
        };
        let repeats = if fb <= 0.0 {
            1.0
        } else {
            // Repeats until feedback^n <= 1e-4 (-80 dB).
            (1e-4_f64.ln() / fb.ln()).ceil().max(1.0)
        };
        let max_tail = (30.0 * f64::from(self.sample_rate)).ceil();
        let tail = (repeats * delay_samples).ceil().min(max_tail);
        Some(tail as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_block_size_does_not_change_output, assert_parameter_changes_output,
    };

    #[test]
    fn delay_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || DelayEffect::new(44100.0),
            &[(0, 50.0), (1, 0.5), (2, 0.7)],
        );
    }

    #[test]
    fn delay_dry_wet_changes_the_output() {
        assert_parameter_changes_output(|| DelayEffect::new(44100.0), 2, 0.0);
    }

    #[test]
    fn delay_tail_samples_is_none_when_dry_or_bypassed() {
        let mut delay = DelayEffect::new(44100.0);
        delay.dry_wet = 0.0;
        assert_eq!(delay.tail_samples(), None);

        delay.dry_wet = 0.5;
        delay.is_enabled = false;
        assert_eq!(delay.tail_samples(), None);
    }

    #[test]
    fn delay_tail_samples_zero_feedback_returns_single_delay() {
        let mut delay = DelayEffect::new(44100.0);
        delay.delay_time_ms = 500.0;
        delay.feedback = 0.0;
        delay.dry_wet = 0.5;
        assert_eq!(delay.tail_samples(), Some(22_050));
    }

    #[test]
    fn delay_tail_samples_scales_with_sample_rate_delay_and_feedback() {
        let mut delay_44k = DelayEffect::new(44100.0);
        delay_44k.delay_time_ms = 100.0;
        delay_44k.feedback = 0.5;
        delay_44k.dry_wet = 0.5;

        let mut delay_88k = DelayEffect::new(88200.0);
        delay_88k.delay_time_ms = 100.0;
        delay_88k.feedback = 0.5;
        delay_88k.dry_wet = 0.5;

        let tail_44k = delay_44k.tail_samples().expect("wet delay has tail");
        let tail_88k = delay_88k.tail_samples().expect("wet delay has tail");
        assert_eq!(tail_88k, tail_44k * 2);

        let mut longer_delay = delay_44k.clone();
        longer_delay.delay_time_ms = 200.0;
        assert_eq!(longer_delay.tail_samples(), Some(tail_44k * 2));

        let mut low_fb = delay_44k.clone();
        low_fb.feedback = 0.0;
        let mut high_fb = delay_44k.clone();
        high_fb.feedback = 0.8;
        assert!(low_fb.tail_samples().expect("tail") < tail_44k);
        assert!(high_fb.tail_samples().expect("tail") > tail_44k);

        let mut extreme = delay_44k;
        extreme.delay_time_ms = 1900.0;
        extreme.feedback = 0.98;
        assert_eq!(extreme.tail_samples(), Some(30 * 44_100));
    }
}
