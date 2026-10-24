use livecraft_core::model::Device;

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
    combs_l: Vec<CombFilter>,
    combs_r: Vec<CombFilter>,
    allpass_l: Vec<AllPassFilter>,
    allpass_r: Vec<AllPassFilter>,
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
            combs_l,
            combs_r,
            allpass_l,
            allpass_r,
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
