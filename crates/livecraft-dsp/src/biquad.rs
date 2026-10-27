use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peaking,
    LowShelf,
    HighShelf,
}

#[derive(Debug, Clone, Copy)]
pub struct BiquadFilter {
    filter_type: FilterType,
    sample_rate: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    // Coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State memory (stereo left & right channels)
    x1_l: f32,
    x2_l: f32,
    y1_l: f32,
    y2_l: f32,
    x1_r: f32,
    x2_r: f32,
    y1_r: f32,
    y2_r: f32,
}

impl BiquadFilter {
    pub fn new(filter_type: FilterType, sample_rate: f32, freq: f32, gain_db: f32, q: f32) -> Self {
        let mut filter = Self {
            filter_type,
            sample_rate: sample_rate.max(1.0),
            freq: freq.clamp(20.0, sample_rate * 0.49),
            gain_db,
            q: q.max(0.01),
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1_l: 0.0,
            x2_l: 0.0,
            y1_l: 0.0,
            y2_l: 0.0,
            x1_r: 0.0,
            x2_r: 0.0,
            y1_r: 0.0,
            y2_r: 0.0,
        };
        filter.recalculate();
        filter
    }

    pub fn set_params(&mut self, freq: f32, gain_db: f32, q: f32) {
        self.freq = freq.clamp(20.0, self.sample_rate * 0.49);
        self.gain_db = gain_db;
        self.q = q.max(0.01);
        self.recalculate();
    }

    pub fn set_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.recalculate();
    }

    pub fn reset_state(&mut self) {
        self.x1_l = 0.0;
        self.x2_l = 0.0;
        self.y1_l = 0.0;
        self.y2_l = 0.0;
        self.x1_r = 0.0;
        self.x2_r = 0.0;
        self.y1_r = 0.0;
        self.y2_r = 0.0;
    }

    pub fn magnitude_response(&self, freq_hz: f32) -> f32 {
        let omega = 2.0 * PI * freq_hz / self.sample_rate;
        let cos1 = omega.cos();
        let cos2 = (2.0 * omega).cos();
        let sin1 = omega.sin();
        let sin2 = (2.0 * omega).sin();

        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -self.b1 * sin1 - self.b2 * sin2;
        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -self.a1 * sin1 - self.a2 * sin2;

        let num_sq = num_re * num_re + num_im * num_im;
        let den_sq = (den_re * den_re + den_im * den_im).max(1e-12);
        (num_sq / den_sq).sqrt()
    }

    fn recalculate(&mut self) {
        let omega = 2.0 * PI * self.freq / self.sample_rate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * self.q);
        let a = 10.0f32.powf(self.gain_db / 40.0); // For peaking & shelving

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => (
                (1.0 - cs) * 0.5,
                1.0 - cs,
                (1.0 - cs) * 0.5,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            FilterType::HighPass => (
                (1.0 + cs) * 0.5,
                -(1.0 + cs),
                (1.0 + cs) * 0.5,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            FilterType::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
            FilterType::Notch => (1.0, -2.0 * cs, 1.0, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
            FilterType::Peaking => (
                1.0 + alpha * a,
                -2.0 * cs,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cs,
                1.0 - alpha / a,
            ),
            FilterType::LowShelf => {
                let sqrt_a = a.sqrt();
                (
                    a * ((a + 1.0) - (a - 1.0) * cs + 2.0 * sqrt_a * alpha),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                    a * ((a + 1.0) - (a - 1.0) * cs - 2.0 * sqrt_a * alpha),
                    (a + 1.0) + (a - 1.0) * cs + 2.0 * sqrt_a * alpha,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                    (a + 1.0) + (a - 1.0) * cs - 2.0 * sqrt_a * alpha,
                )
            }
            FilterType::HighShelf => {
                let sqrt_a = a.sqrt();
                (
                    a * ((a + 1.0) + (a - 1.0) * cs + 2.0 * sqrt_a * alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                    a * ((a + 1.0) + (a - 1.0) * cs - 2.0 * sqrt_a * alpha),
                    (a + 1.0) - (a - 1.0) * cs + 2.0 * sqrt_a * alpha,
                    2.0 * ((a - 1.0) - (a + 1.0) * cs),
                    (a + 1.0) - (a - 1.0) * cs - 2.0 * sqrt_a * alpha,
                )
            }
        };

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline(always)]
    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        // Direct Form II Transposed or Direct Form I
        let out_l = self.b0 * in_l + self.b1 * self.x1_l + self.b2 * self.x2_l
            - self.a1 * self.y1_l
            - self.a2 * self.y2_l;
        self.x2_l = self.x1_l;
        self.x1_l = in_l;
        self.y2_l = self.y1_l;
        self.y1_l = out_l;

        let out_r = self.b0 * in_r + self.b1 * self.x1_r + self.b2 * self.x2_r
            - self.a1 * self.y1_r
            - self.a2 * self.y2_r;
        self.x2_r = self.x1_r;
        self.x1_r = in_r;
        self.y2_r = self.y1_r;
        self.y1_r = out_r;

        (out_l, out_r)
    }
}
