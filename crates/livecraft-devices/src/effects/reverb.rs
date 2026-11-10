use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use livecraft_model::param::ParamId;

// Freeverb Comb and All-Pass tuning constants (at 44100 Hz, scaled to sample_rate)
const COMB_TUNINGS_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const COMB_TUNINGS_R: [usize; 8] = [1139, 1211, 1300, 1379, 1445, 1514, 1580, 1640];
const ALLPASS_TUNINGS_L: [usize; 4] = [556, 441, 341, 225];
const ALLPASS_TUNINGS_R: [usize; 4] = [579, 464, 364, 248];

#[derive(Debug, Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    index: usize,
    filter_store: f32,
    damp: f32,
    feedback: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
            filter_store: 0.0,
            damp: 0.4,
            feedback: 0.8,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter_store = output * (1.0 - self.damp) + self.filter_store * self.damp;
        self.buffer[self.index] = input + self.filter_store * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

#[derive(Debug, Clone)]
struct AllPassFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
            feedback: 0.5,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.index];
        let output = -input + buf_out;
        self.buffer[self.index] = input + buf_out * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

#[derive(Debug, Clone)]
pub struct ReverbEffect {
    pub device_id: DeviceId,
    combs_l: Vec<CombFilter>,
    combs_r: Vec<CombFilter>,
    allpass_l: Vec<AllPassFilter>,
    allpass_r: Vec<AllPassFilter>,
    sample_rate: f32,
    pub room_size: f32,
    pub damping: f32,
    pub stereo_width: f32,
    pub dry_wet: f32,
    pub is_enabled: bool,
}

impl ReverbEffect {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;

        let combs_l = COMB_TUNINGS_L
            .iter()
            .map(|&t| CombFilter::new(((t as f32) * scale) as usize))
            .collect();
        let combs_r = COMB_TUNINGS_R
            .iter()
            .map(|&t| CombFilter::new(((t as f32) * scale) as usize))
            .collect();
        let allpass_l = ALLPASS_TUNINGS_L
            .iter()
            .map(|&t| AllPassFilter::new(((t as f32) * scale) as usize))
            .collect();
        let allpass_r = ALLPASS_TUNINGS_R
            .iter()
            .map(|&t| AllPassFilter::new(((t as f32) * scale) as usize))
            .collect();

        let mut rev = Self {
            device_id: DeviceId(0),
            combs_l,
            combs_r,
            allpass_l,
            allpass_r,
            sample_rate,
            room_size: 0.75,
            damping: 0.35,
            stereo_width: 1.0,
            dry_wet: 0.25,
            is_enabled: true,
        };
        rev.update_params();
        rev
    }

    pub fn set_params(&mut self, room_size: f32, damping: f32, dry_wet: f32) {
        self.room_size = room_size.clamp(0.0, 0.98);
        self.damping = damping.clamp(0.0, 1.0);
        self.dry_wet = dry_wet.clamp(0.0, 1.0);
        self.update_params();
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        let room_size = dev.get_param("room_size");
        let damping = dev.get_param("damping");
        let dry_wet = dev.get_param("dry_wet");
        self.stereo_width = dev.get_param("stereo_width").clamp(0.0, 1.0);
        self.set_params(room_size, damping, dry_wet);
    }

    fn update_params(&mut self) {
        let fb = self.room_size * 0.28 + 0.7;
        let damp = self.damping * 0.4;
        for c in &mut self.combs_l {
            c.feedback = fb;
            c.damp = damp;
        }
        for c in &mut self.combs_r {
            c.feedback = fb;
            c.damp = damp;
        }
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let input = (in_l + in_r) * 0.015; // Input scaling

        // Comb filters in parallel
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for c in &mut self.combs_l {
            out_l += c.process(input);
        }
        for c in &mut self.combs_r {
            out_r += c.process(input);
        }

        // Allpass filters in series
        for ap in &mut self.allpass_l {
            out_l = ap.process(out_l);
        }
        for ap in &mut self.allpass_r {
            out_r = ap.process(out_r);
        }

        // Wet / dry mix with stereo spread
        let wet1 = self.dry_wet * (self.stereo_width * 0.5 + 0.5);
        let wet2 = self.dry_wet * ((1.0 - self.stereo_width) * 0.5);
        let dry = 1.0 - self.dry_wet;

        let res_l = in_l * dry + out_l * wet1 + out_r * wet2;
        let res_r = in_r * dry + out_r * wet1 + out_l * wet2;

        (res_l, res_r)
    }
}

impl DeviceProcessor for ReverbEffect {
    fn process(
        &mut self,
        _ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        _events: &EventList,
        _out_events: &mut EventSink,
    ) {
        io.for_each_frame(|l, r| self.process_stereo(l, r));
    }

    /// Ids are positions in the Reverb table of `livecraft_model::device`. Id 0, the decay time,
    /// has no effect in this port, as before M05.
    fn set_param(&mut self, id: ParamId, value: f32, _offset: u32) {
        match id.0 {
            1 => self.set_params(value, self.damping, self.dry_wet),
            2 => self.set_params(self.room_size, value, self.dry_wet),
            3 => self.set_params(self.room_size, self.damping, value),
            4 => self.stereo_width = value.clamp(0.0, 1.0),
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
            || !self.room_size.is_finite()
            || !self.damping.is_finite()
        {
            return None;
        }
        let max_comb_len = self
            .combs_l
            .iter()
            .chain(self.combs_r.iter())
            .map(|c| c.buffer.len())
            .fold(0usize, usize::max);
        if max_comb_len == 0 {
            return None;
        }
        let room = f64::from(self.room_size).clamp(0.0, 0.98);
        let damp = f64::from(self.damping).clamp(0.0, 1.0) * 0.4;
        // Freeverb comb loop feedback with mild high-frequency damping attenuation.
        let fb = (room * 0.28 + 0.7) * (1.0 - damp * 0.1);
        let loops = (1e-4_f64.ln() / fb.ln()).ceil().max(1.0);
        let max_tail = (30.0 * f64::from(self.sample_rate)).ceil();
        let tail = (loops * max_comb_len as f64).ceil().min(max_tail);
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
    fn reverb_output_does_not_depend_on_the_block_size() {
        assert_block_size_does_not_change_output(
            || ReverbEffect::new(44100.0),
            &[(1, 0.8), (2, 0.4), (3, 0.5)],
        );
    }

    #[test]
    fn reverb_room_size_changes_the_output() {
        assert_parameter_changes_output(|| ReverbEffect::new(44100.0), 1, 0.1);
    }

    #[test]
    fn reverb_tail_samples_is_none_when_dry_or_bypassed() {
        let mut rev = ReverbEffect::new(44100.0);
        rev.dry_wet = 0.0;
        assert_eq!(rev.tail_samples(), None);

        rev.dry_wet = 0.5;
        rev.is_enabled = false;
        assert_eq!(rev.tail_samples(), None);
    }

    #[test]
    fn reverb_tail_samples_scales_with_sample_rate_room_size_and_damping() {
        let rev_44k = ReverbEffect::new(44100.0);
        let rev_88k = ReverbEffect::new(88200.0);
        let tail_44k = rev_44k.tail_samples().expect("wet reverb has tail");
        let tail_88k = rev_88k.tail_samples().expect("wet reverb has tail");
        assert_eq!(tail_88k, tail_44k * 2);

        let mut small_room = rev_44k.clone();
        small_room.set_params(0.2, 0.35, 0.25);
        let mut large_room = rev_44k.clone();
        large_room.set_params(0.9, 0.35, 0.25);
        assert!(small_room.tail_samples().expect("tail") < tail_44k);
        assert!(large_room.tail_samples().expect("tail") > tail_44k);

        let mut no_damp = rev_44k.clone();
        no_damp.set_params(0.75, 0.0, 0.25);
        let mut max_damp = rev_44k.clone();
        max_damp.set_params(0.75, 1.0, 0.25);
        assert!(max_damp.tail_samples().expect("tail") < no_damp.tail_samples().expect("tail"));

        let mut extreme = rev_44k;
        extreme.set_params(0.98, 0.0, 1.0);
        assert!(extreme.tail_samples().expect("tail") <= 30 * 44_100);
    }
}
