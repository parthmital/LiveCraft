use std::fmt;
use std::str::FromStr;

const SCALE_I16: f64 = 32768.0;
const SCALE_I24: f64 = 8388608.0;
const I24_MIN: i32 = -8_388_608;
const I24_MAX: i32 = 8_388_607;
const MAX_SHAPER_ORDER: usize = 9;

// Minimum-phase FIR error-feedback coefficients c[k] for NTF(z) = 1 - sum c[k] * z^-(k+1).
// All zeros sit strictly inside the unit circle (|r| <= 0.48), keeping feedback unconditionally
// bounded while shifting quantization noise out of the 1-5 kHz ear-sensitive band toward Nyquist.
const POWR1_COEFFS: [f64; 3] = [0.926_455_697_114_727, -0.310_607_595_192_123, 0.036_288];

const POWR2_COEFFS: [f64; 5] = [
    1.853_518_704_405_581,
    -1.433_410_610_484_682,
    0.578_523_176_491_932,
    -0.121_933_233_261_089,
    0.010_725_12,
];

const POWR3_COEFFS: [f64; 9] = [
    2.694_931_224_05,
    -3.306_616_060_337,
    2.423_725_179_699,
    -1.170_330_592_747,
    0.386_663_147_062,
    -0.087_635_015_46,
    0.013_185_254_391,
    -0.001_199_929_449,
    0.000_050_518_341,
];

/// Dither algorithms matching Live's Export Audio/Video Dither Options (Manual 5.1.3.3, p. 129).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DitherMode {
    NoDither,
    Rectangular,
    #[default]
    Triangular,
    PowR1,
    PowR2,
    PowR3,
}

impl DitherMode {
    /// All dither modes in Live's chooser order.
    pub const ALL: [DitherMode; 6] = [
        DitherMode::NoDither,
        DitherMode::Rectangular,
        DitherMode::Triangular,
        DitherMode::PowR1,
        DitherMode::PowR2,
        DitherMode::PowR3,
    ];

    /// Returns the exact Live manual display label for this dither mode.
    pub const fn label(self) -> &'static str {
        match self {
            DitherMode::NoDither => "No Dither",
            DitherMode::Rectangular => "Rectangular",
            DitherMode::Triangular => "Triangular",
            DitherMode::PowR1 => "POW-r 1",
            DitherMode::PowR2 => "POW-r 2",
            DitherMode::PowR3 => "POW-r 3",
        }
    }

    /// Parses a dither mode from its canonical label or common CLI/normalized aliases.
    pub fn parse_label(label: &str) -> Option<Self> {
        let normalized: String = label
            .trim()
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '_'))
            .flat_map(char::to_lowercase)
            .collect();

        match normalized.as_str() {
            "nodither" | "none" => Some(DitherMode::NoDither),
            "rectangular" | "rpdf" => Some(DitherMode::Rectangular),
            "triangular" | "tpdf" => Some(DitherMode::Triangular),
            "powr1" => Some(DitherMode::PowR1),
            "powr2" => Some(DitherMode::PowR2),
            "powr3" => Some(DitherMode::PowR3),
            _ => None,
        }
    }

    fn shaper_coeffs(self) -> &'static [f64] {
        match self {
            DitherMode::NoDither | DitherMode::Rectangular | DitherMode::Triangular => &[],
            DitherMode::PowR1 => &POWR1_COEFFS,
            DitherMode::PowR2 => &POWR2_COEFFS,
            DitherMode::PowR3 => &POWR3_COEFFS,
        }
    }
}

impl fmt::Display for DitherMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Error returned when parsing an unrecognized [`DitherMode`] label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseDitherModeError;

impl fmt::Display for ParseDitherModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid dither mode; expected No Dither, Rectangular, Triangular, POW-r 1, POW-r 2, or POW-r 3")
    }
}

impl std::error::Error for ParseDitherModeError {}

impl FromStr for DitherMode {
    type Err = ParseDitherModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_label(s).ok_or(ParseDitherModeError)
    }
}

/// Deterministic in-house PCG32 (XSH-RR) pseudorandom number generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

    /// Creates a new PCG32 stream from a 64-bit seed and stream selector.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        let _ = rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        let _ = rng.next_u32();
        rng
    }

    /// Advances the generator and returns a uniform 32-bit unsigned integer.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old_state = self.state;
        self.state = old_state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.inc);
        let xorshifted = (((old_state >> 18) ^ old_state) >> 27) as u32;
        let rot = (old_state >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Returns a uniform `f64` in `[0.0, 1.0)`.
    #[inline]
    pub fn next_unit_f64(&mut self) -> f64 {
        const INV_2_POW_32: f64 = 1.0 / 4_294_967_296.0;
        f64::from(self.next_u32()) * INV_2_POW_32
    }
}

/// Stateful per-channel audio ditherer and integer PCM quantizer.
///
/// Uses a deterministic seeded [`Pcg32`] PRNG and per-channel noise-shaping error history for
/// `POW-r 1`, `POW-r 2`, and `POW-r 3`. Integer quantization scales 16-bit by `32768.0` and
/// 24-bit by `8388608.0` (matching Live 39.2.1 / `audio_clip.rs`) so undithered same-depth
/// round-trips cancel completely. 32-bit float audio is never clamped or dithered.
#[derive(Debug, Clone)]
pub struct Ditherer {
    mode: DitherMode,
    rng: Pcg32,
    history: Vec<[f64; MAX_SHAPER_ORDER]>,
}

impl Ditherer {
    /// Fixed default seed for deterministic offline renders.
    pub const DEFAULT_SEED: u64 = 0x4C69_7665_4372_6166;
    const DEFAULT_STREAM: u64 = 0x9E37_79B9_7F4A_7C15;

    /// Creates a ditherer with explicit mode, channel count, and deterministic PRNG seed.
    pub fn new(mode: DitherMode, channels: usize, seed: u64) -> Self {
        Self {
            mode,
            rng: Pcg32::new(seed, Self::DEFAULT_STREAM),
            history: vec![[0.0; MAX_SHAPER_ORDER]; channels.max(1)],
        }
    }

    /// Creates a ditherer initialized with [`Self::DEFAULT_SEED`].
    pub fn with_default_seed(mode: DitherMode, channels: usize) -> Self {
        Self::new(mode, channels, Self::DEFAULT_SEED)
    }

    /// Returns the active [`DitherMode`].
    pub fn mode(&self) -> DitherMode {
        self.mode
    }

    /// Changes the active [`DitherMode`] and clears noise-shaping filter state.
    pub fn set_mode(&mut self, mode: DitherMode) {
        self.mode = mode;
        self.reset();
    }

    /// Clears per-channel noise-shaping error history while retaining the PRNG state.
    pub fn reset(&mut self) {
        self.history.fill([0.0; MAX_SHAPER_ORDER]);
    }

    /// Quantizes interleaved `f32` samples to signed 16-bit PCM (`i16`) using `32768.0` scaling.
    ///
    /// Clamping occurs strictly after dithering/noise-shaping at `[i16::MIN, i16::MAX]`.
    pub fn quantize_i16(&mut self, samples: &[f32], channels: usize) -> Vec<i16> {
        let ch_count = self.ensure_channels(channels);
        let mut out = Vec::with_capacity(samples.len());
        self.quantize_interleaved(
            samples,
            ch_count,
            SCALE_I16,
            i32::from(i16::MIN),
            i32::from(i16::MAX),
            |q| {
                out.push(q as i16);
            },
        );
        out
    }

    /// Quantizes interleaved `f32` samples to signed 24-bit PCM stored in `i32` (`[-8388608, 8388607]`)
    /// using `8388608.0` scaling.
    ///
    /// Clamping occurs strictly after dithering/noise-shaping at `[-8388608, 8388607]`.
    pub fn quantize_i24(&mut self, samples: &[f32], channels: usize) -> Vec<i32> {
        let ch_count = self.ensure_channels(channels);
        let mut out = Vec::with_capacity(samples.len());
        self.quantize_interleaved(samples, ch_count, SCALE_I24, I24_MIN, I24_MAX, |q| {
            out.push(q);
        });
        out
    }

    fn ensure_channels(&mut self, channels: usize) -> usize {
        let ch_count = channels.max(1);
        if self.history.len() < ch_count {
            self.history.resize(ch_count, [0.0; MAX_SHAPER_ORDER]);
        }
        ch_count
    }

    fn quantize_interleaved<F>(
        &mut self,
        samples: &[f32],
        ch_count: usize,
        scale: f64,
        min_int: i32,
        max_int: i32,
        mut push: F,
    ) where
        F: FnMut(i32),
    {
        if samples.is_empty() {
            return;
        }
        let coeffs = self.mode.shaper_coeffs();
        let min_f = f64::from(min_int);
        let max_f = f64::from(max_int);

        for frame in samples.chunks(ch_count) {
            for (ch, &sample) in frame.iter().enumerate() {
                let q = self.quantize_one(sample, ch, coeffs, scale, min_f, max_f);
                push(q);
            }
        }
    }

    #[inline]
    fn quantize_one(
        &mut self,
        sample: f32,
        channel: usize,
        coeffs: &[f64],
        scale: f64,
        min_f: f64,
        max_f: f64,
    ) -> i32 {
        let sanitized = if sample.is_nan() {
            0.0
        } else {
            f64::from(sample)
        };
        let scaled = sanitized * scale;

        if !scaled.is_finite() {
            if !coeffs.is_empty() {
                self.history[channel] = [0.0; MAX_SHAPER_ORDER];
            }
            return scaled.clamp(min_f, max_f) as i32;
        }

        match self.mode {
            DitherMode::NoDither => scaled.round().clamp(min_f, max_f) as i32,
            DitherMode::Rectangular => {
                // Zero-centered RPDF in [-0.5, 0.5) LSB.
                let dither = self.rng.next_unit_f64() - 0.5;
                (scaled + dither).round().clamp(min_f, max_f) as i32
            }
            DitherMode::Triangular => {
                // Exact zero-mean TPDF in (-1.0, 1.0) LSB.
                let dither = self.rng.next_unit_f64() - self.rng.next_unit_f64();
                (scaled + dither).round().clamp(min_f, max_f) as i32
            }
            DitherMode::PowR1 | DitherMode::PowR2 | DitherMode::PowR3 => {
                let hist = &mut self.history[channel];
                let mut feedback = 0.0;
                for (k, &c) in coeffs.iter().enumerate() {
                    feedback += c * hist[k];
                }
                let quantizer_in = scaled - feedback;
                let dither = self.rng.next_unit_f64() - self.rng.next_unit_f64();
                let q_unclamped = (quantizer_in + dither).round();
                // Track error against unclamped quantization so saturation cannot destabilize feedback.
                let err = q_unclamped - quantizer_in;
                for k in (1..coeffs.len()).rev() {
                    hist[k] = hist[k - 1];
                }
                hist[0] = err;
                q_unclamped.clamp(min_f, max_f) as i32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use realfft::RealFftPlanner;
    use std::f64::consts::PI;

    #[test]
    fn test_dither_mode_labels_and_default() {
        assert_eq!(DitherMode::default(), DitherMode::Triangular);
        let expected = [
            (DitherMode::NoDither, "No Dither"),
            (DitherMode::Rectangular, "Rectangular"),
            (DitherMode::Triangular, "Triangular"),
            (DitherMode::PowR1, "POW-r 1"),
            (DitherMode::PowR2, "POW-r 2"),
            (DitherMode::PowR3, "POW-r 3"),
        ];
        for (mode, label) in expected {
            assert_eq!(mode.label(), label);
            assert_eq!(mode.to_string(), label);
            assert_eq!(DitherMode::parse_label(label), Some(mode));
            assert_eq!(label.parse::<DitherMode>(), Ok(mode));
        }
        assert_eq!(DitherMode::parse_label("pow-r-2"), Some(DitherMode::PowR2));
        assert_eq!(
            DitherMode::parse_label("no_dither"),
            Some(DitherMode::NoDither)
        );
        assert_eq!(DitherMode::parse_label("invalid"), None);
        assert!("invalid".parse::<DitherMode>().is_err());
    }

    #[test]
    fn test_pcg32_determinism() {
        let mut a = Pcg32::new(12345, 67890);
        let mut b = Pcg32::new(12345, 67890);
        let mut c = Pcg32::new(12346, 67890);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        assert_ne!(a.next_u32(), c.next_u32());
    }

    #[test]
    fn test_no_dither_i16_exact_roundtrip() {
        let mut dither = Ditherer::with_default_seed(DitherMode::NoDither, 1);
        let all_i16: Vec<i16> = (i16::MIN..=i16::MAX).collect();
        let floats: Vec<f32> = all_i16.iter().map(|&s| f32::from(s) / 32768.0).collect();
        let roundtrip = dither.quantize_i16(&floats, 1);
        assert_eq!(roundtrip, all_i16);
    }

    #[test]
    fn test_no_dither_i24_exact_roundtrip() {
        let mut dither = Ditherer::with_default_seed(DitherMode::NoDither, 2);
        // Sample across the full 24-bit signed range including boundaries and step sweep
        let mut expected = Vec::new();
        for v in [I24_MIN, I24_MIN + 1, -1, 0, 1, I24_MAX - 1, I24_MAX] {
            expected.push(v);
        }
        let mut v = I24_MIN;
        while v <= I24_MAX - 1021 {
            expected.push(v);
            v += 1021;
        }
        if expected.len() % 2 != 0 {
            expected.push(0);
        }
        let floats: Vec<f32> = expected.iter().map(|&s| s as f32 / 8_388_608.0).collect();
        let roundtrip = dither.quantize_i24(&floats, 2);
        assert_eq!(roundtrip, expected);
    }

    #[test]
    fn test_dither_clamps_extremes_and_handles_non_finite() {
        let mut dither = Ditherer::with_default_seed(DitherMode::Triangular, 2);
        let samples = [
            2.0_f32,
            -2.0_f32,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            0.0,
        ];
        let q16 = dither.quantize_i16(&samples, 2);
        assert_eq!(q16[0], i16::MAX);
        assert_eq!(q16[1], i16::MIN);
        assert_eq!(q16[3], i16::MAX);
        assert_eq!(q16[4], i16::MIN);

        let q24 = dither.quantize_i24(&samples, 2);
        assert_eq!(q24[0], I24_MAX);
        assert_eq!(q24[1], I24_MIN);
        assert_eq!(q24[3], I24_MAX);
        assert_eq!(q24[4], I24_MIN);
    }

    #[test]
    fn test_powr_noise_shaping_pushes_noise_to_high_frequencies() {
        let sample_rate = 44_100.0;
        let len = 16_384;
        // Low-level -70 dBFS 997 Hz sine
        let amp = 10.0_f64.powf(-70.0 / 20.0);
        let input: Vec<f32> = (0..len)
            .map(|n| (amp * (2.0 * PI * 997.0 * (n as f64) / sample_rate).sin()) as f32)
            .collect();

        let band_power = |mode: DitherMode| -> (f64, f64) {
            let mut dither = Ditherer::with_default_seed(mode, 1);
            let quantized = dither.quantize_i16(&input, 1);
            let mut err: Vec<f64> = quantized
                .iter()
                .zip(input.iter())
                .map(|(&q, &x)| (f64::from(q) / SCALE_I16) - f64::from(x))
                .collect();
            // Hann window to isolate spectral bands cleanly
            for (n, s) in err.iter_mut().enumerate() {
                let w = 0.5 * (1.0 - (2.0 * PI * n as f64 / len as f64).cos());
                *s *= w;
            }
            let mut planner = RealFftPlanner::<f64>::new();
            let fft = planner.plan_fft_forward(len);
            let mut spec = fft.make_output_vec();
            let mut scratch = fft.make_scratch_vec();
            fft.process_with_scratch(&mut err, &mut spec, &mut scratch)
                .unwrap();

            let mut mid_band = 0.0;
            let mut high_band = 0.0;
            for (k, bin) in spec.iter().enumerate() {
                let freq = k as f64 * sample_rate / len as f64;
                let p = bin.norm_sqr();
                if (1200.0..=5000.0).contains(&freq) {
                    mid_band += p;
                } else if (16_000.0..=21_500.0).contains(&freq) {
                    high_band += p;
                }
            }
            (10.0 * mid_band.log10(), 10.0 * high_band.log10())
        };

        let (tri_mid, tri_high) = band_power(DitherMode::Triangular);
        let (p1_mid, p1_high) = band_power(DitherMode::PowR1);
        let (p2_mid, p2_high) = band_power(DitherMode::PowR2);
        let (p3_mid, p3_high) = band_power(DitherMode::PowR3);

        // Midband (1.2-5 kHz) noise drops monotonically across Triangular -> POW-r 1 -> 2 -> 3.
        assert!(
            p1_mid < tri_mid - 4.0,
            "POW-r 1 mid={p1_mid} vs tri={tri_mid}"
        );
        assert!(p2_mid < p1_mid - 4.0, "POW-r 2 mid={p2_mid} vs p1={p1_mid}");
        assert!(p3_mid < p2_mid - 4.0, "POW-r 3 mid={p3_mid} vs p2={p2_mid}");

        // High-frequency (16-21.5 kHz) noise increases monotonically across Triangular -> POW-r 1 -> 2 -> 3.
        assert!(
            p1_high > tri_high + 2.0,
            "POW-r 1 high={p1_high} vs tri={tri_high}"
        );
        assert!(
            p2_high > p1_high + 2.0,
            "POW-r 2 high={p2_high} vs p1={p1_high}"
        );
        assert!(
            p3_high > p2_high + 2.0,
            "POW-r 3 high={p3_high} vs p2={p2_high}"
        );
    }

    #[test]
    fn test_powr_per_channel_state_isolation_and_stability() {
        let mut dither = Ditherer::with_default_seed(DitherMode::PowR3, 2);
        // Channel 0 has silence, channel 1 has full-scale sine: channel states stay isolated and stable
        let mut stereo = Vec::with_capacity(4096 * 2);
        for n in 0..4096 {
            stereo.push(0.0_f32);
            stereo.push((0.99 * (2.0 * PI * 1000.0 * n as f64 / 44_100.0).sin()) as f32);
        }
        let q = dither.quantize_i16(&stereo, 2);
        let max_ch0 = q.iter().step_by(2).map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_ch0 <= 32,
            "silence channel leaked or oscillated: max={max_ch0}"
        );
    }

    #[test]
    fn test_ditherer_deterministic_across_instances() {
        let input: Vec<f32> = (0..512)
            .map(|n| (0.25 * (2.0 * PI * 440.0 * n as f64 / 44_100.0).sin()) as f32)
            .collect();
        for mode in DitherMode::ALL {
            let mut a = Ditherer::with_default_seed(mode, 2);
            let mut b = Ditherer::with_default_seed(mode, 2);
            assert_eq!(a.quantize_i16(&input, 2), b.quantize_i16(&input, 2));
            assert_eq!(a.quantize_i24(&input, 2), b.quantize_i24(&input, 2));
        }
    }
}
