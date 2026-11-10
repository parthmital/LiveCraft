use realfft::RealFftPlanner;
use std::f64::consts::PI;

/// 28 standard IEC 61260 1/3-octave nominal center frequencies from 31.5 Hz to 16 kHz.
pub const THIRD_OCTAVE_CENTERS_HZ: [f64; 28] = [
    31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0,
    800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0,
    10_000.0, 12_500.0, 16_000.0,
];

/// Summary loudness and level metrics for an audio buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioMetrics {
    pub peak_dbfs: f64,
    pub true_peak_dbtp: f64,
    pub rms_dbfs: f64,
    pub integrated_lufs: f64,
    pub short_term_max_lufs: f64,
    pub momentary_max_lufs: f64,
}

/// Spectral energy inside a single 1/3-octave band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandEnergy {
    pub center_hz: f64,
    pub energy_db: f64,
}

// 48-tap (4x12 polyphase) linear-phase FIR interpolation filter for 4x true-peak measurement.
// Designed as a Type-II 48-tap FIR (phases 0/3 and 1/2 are time-reverses) with flat passband
// across 0..20 kHz (< 0.25 dB) and exact unit DC gain per phase.
const TRUE_PEAK_PHASES: [[f64; 12]; 4] = [
    [
        -0.001_678_688_303_357_64,
        0.007_458_815_587_486_35,
        -0.015_505_358_820_312_2,
        0.028_911_514_971_898_5,
        -0.054_844_900_922_158_5,
        0.133_215_521_195_314,
        0.978_782_637_959_636,
        -0.106_066_508_810_823,
        0.050_780_383_141_514_5,
        -0.028_717_072_447_214_6,
        0.016_355_084_543_882_9,
        -0.008_691_428_095_866_7,
    ],
    [
        -0.012_604_231_591_139_9,
        0.027_257_002_955_721_6,
        -0.050_050_537_837_954_1,
        0.088_310_397_533_081_6,
        -0.166_575_715_785_914,
        0.468_348_724_147_083,
        0.786_164_594_193_95,
        -0.201_204_381_190_323,
        0.101_387_036_037_405,
        -0.056_945_513_160_714_3,
        0.031_402_118_881_678_3,
        -0.015_489_494_182_874_3,
    ],
    [
        -0.015_489_494_182_874_3,
        0.031_402_118_881_678_3,
        -0.056_945_513_160_714_2,
        0.101_387_036_037_405,
        -0.201_204_381_190_323,
        0.786_164_594_193_95,
        0.468_348_724_147_083,
        -0.166_575_715_785_913,
        0.088_310_397_533_081_6,
        -0.050_050_537_837_954_1,
        0.027_257_002_955_721_6,
        -0.012_604_231_591_139_9,
    ],
    [
        -0.008_691_428_095_866_7,
        0.016_355_084_543_882_9,
        -0.028_717_072_447_214_6,
        0.050_780_383_141_514_5,
        -0.106_066_508_810_823,
        0.978_782_637_959_636,
        0.133_215_521_195_314,
        -0.054_844_900_922_158_5,
        0.028_911_514_971_898_5,
        -0.015_505_358_820_312_2,
        0.007_458_815_587_486_35,
        -0.001_678_688_303_357_64,
    ],
];

#[inline]
fn linear_to_dbfs(amp: f64) -> f64 {
    if amp <= 0.0 || !amp.is_finite() {
        f64::NEG_INFINITY
    } else {
        20.0 * amp.log10()
    }
}

#[inline]
fn power_to_dbfs(mean_square: f64) -> f64 {
    if mean_square <= 0.0 || !mean_square.is_finite() {
        f64::NEG_INFINITY
    } else {
        10.0 * mean_square.log10()
    }
}

#[inline]
fn power_to_lufs(weighted_mean_square: f64) -> f64 {
    if weighted_mean_square <= 0.0 || !weighted_mean_square.is_finite() {
        f64::NEG_INFINITY
    } else {
        -0.691 + 10.0 * weighted_mean_square.log10()
    }
}

#[inline]
fn sanitize_f64(sample: f32) -> f64 {
    if sample.is_finite() {
        f64::from(sample)
    } else {
        0.0
    }
}

/// Returns the maximum absolute linear sample value across `samples` (`0.0` if empty).
pub fn peak_linear(samples: &[f32]) -> f64 {
    let mut max_abs = 0.0_f64;
    for &s in samples {
        let v = sanitize_f64(s).abs();
        if v > max_abs {
            max_abs = v;
        }
    }
    max_abs
}

/// Returns the sample peak level in dBFS (`-inf` if silent or empty).
pub fn peak_dbfs(samples: &[f32]) -> f64 {
    linear_to_dbfs(peak_linear(samples))
}

/// Returns the root-mean-square level in dBFS across all samples (`-inf` if silent or empty).
pub fn rms_dbfs(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return f64::NEG_INFINITY;
    }
    let mut sum_sq = 0.0_f64;
    for &s in samples {
        let v = sanitize_f64(s);
        sum_sq += v * v;
    }
    power_to_dbfs(sum_sq / samples.len() as f64)
}

/// Computes the RMS level in dBFS of `(reference[i] - candidate[i])` relative to 1.0 full scale.
///
/// If lengths differ, missing trailing samples are treated as `0.0`.
pub fn residual_rms_dbfs(reference: &[f32], candidate: &[f32]) -> f64 {
    let len = reference.len().max(candidate.len());
    if len == 0 {
        return f64::NEG_INFINITY;
    }
    let mut diff_sq = 0.0_f64;
    for i in 0..len {
        let r = reference.get(i).copied().map_or(0.0, sanitize_f64);
        let c = candidate.get(i).copied().map_or(0.0, sanitize_f64);
        let d = r - c;
        diff_sq += d * d;
    }
    power_to_dbfs(diff_sq / len as f64)
}

/// Computes the RMS level in dB of `(reference - candidate)` relative to the RMS of `reference`.
///
/// Returns `-inf` when both signals match identically (including both silent), and `+inf` when
/// `reference` is silent while `candidate` is non-zero.
pub fn relative_residual_rms_dbfs(reference: &[f32], candidate: &[f32]) -> f64 {
    let len = reference.len().max(candidate.len());
    if len == 0 {
        return f64::NEG_INFINITY;
    }
    let mut diff_sq = 0.0_f64;
    let mut ref_sq = 0.0_f64;
    for i in 0..len {
        let r = reference.get(i).copied().map_or(0.0, sanitize_f64);
        let c = candidate.get(i).copied().map_or(0.0, sanitize_f64);
        let d = r - c;
        diff_sq += d * d;
        ref_sq += r * r;
    }
    if diff_sq == 0.0 {
        f64::NEG_INFINITY
    } else if ref_sq == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (diff_sq / ref_sq).log10()
    }
}

/// Computes ITU-R BS.1770-4 true-peak level in dBTP using a 4x oversampling 48-tap (4x12)
/// polyphase FIR interpolation filter across all channels.
pub fn true_peak_dbtp(samples: &[f32], channels: usize, sample_rate: u32) -> f64 {
    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return f64::NEG_INFINITY;
    }
    let frames = samples.len() / channels;
    if frames == 0 {
        return f64::NEG_INFINITY;
    }

    let mut max_tp = 0.0_f64;
    let mut delay = vec![[0.0_f64; 12]; channels];
    let mut ring_pos = 0_usize;

    for frame in samples.chunks_exact(channels) {
        for (ch, &sample) in frame.iter().enumerate() {
            let x = sanitize_f64(sample);
            max_tp = max_tp.max(x.abs());
            delay[ch][ring_pos] = x;
            max_tp = max_tp.max(eval_polyphase_peak(&delay[ch], ring_pos));
        }
        ring_pos = (ring_pos + 1) % 12;
    }

    // Flush trailing zeros so peaks near the end of the buffer are evaluated completely.
    for _ in 0..11 {
        for ch_delay in &mut delay {
            ch_delay[ring_pos] = 0.0;
            max_tp = max_tp.max(eval_polyphase_peak(ch_delay, ring_pos));
        }
        ring_pos = (ring_pos + 1) % 12;
    }

    linear_to_dbfs(max_tp)
}

#[inline]
fn eval_polyphase_peak(delay: &[f64; 12], newest_idx: usize) -> f64 {
    let mut ordered = [0.0_f64; 12];
    for (k, slot) in ordered.iter_mut().enumerate() {
        let idx = (newest_idx + 12 - k) % 12;
        *slot = delay[idx];
    }

    let mut peak = 0.0_f64;
    for phase in &TRUE_PEAK_PHASES {
        let mut acc = 0.0_f64;
        for k in 0..12 {
            acc += phase[k] * ordered[k];
        }
        let abs_acc = acc.abs();
        if abs_acc > peak {
            peak = abs_acc;
        }
    }
    peak
}

#[derive(Debug, Clone, Copy)]
struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    #[inline]
    fn process(&mut self, c: &BiquadCoeffs, x: f64) -> f64 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

struct KWeightFilter {
    stage1: BiquadCoeffs,
    stage2: BiquadCoeffs,
    states: Vec<[BiquadState; 2]>,
}

impl KWeightFilter {
    /// Builds ITU-R BS.1770-4 K-weighting pre-filter coefficients via bilinear transform of the
    /// specification's analog shelf + RLB prototypes (matching Tables 1 & 2 at 48 kHz to 1e-15).
    fn new(channels: usize, sample_rate: u32) -> Self {
        let fs = f64::from(sample_rate.max(1));

        // Stage 1: acoustic head high-shelf (~1682 Hz, +4 dB)
        const F0_SHELF: f64 = 1_681.974_450_955_533;
        const DB_SHELF: f64 = 3.999_843_853_973_347;
        const Q_SHELF: f64 = 0.707_175_236_955_419_6;

        let f1 = F0_SHELF.min(fs * 0.49);
        let k1 = (PI * f1 / fs).tan();
        let vh = 10.0_f64.powf(DB_SHELF / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0_1 = 1.0 + k1 / Q_SHELF + k1 * k1;
        let stage1 = BiquadCoeffs {
            b0: (vh + vb * k1 / Q_SHELF + k1 * k1) / a0_1,
            b1: 2.0 * (k1 * k1 - vh) / a0_1,
            b2: (vh - vb * k1 / Q_SHELF + k1 * k1) / a0_1,
            a1: 2.0 * (k1 * k1 - 1.0) / a0_1,
            a2: (1.0 - k1 / Q_SHELF + k1 * k1) / a0_1,
        };

        // Stage 2: RLB high-pass (~38.1 Hz)
        const F0_RLB: f64 = 38.135_470_876_024_44;
        const Q_RLB: f64 = 0.500_327_037_323_877_3;

        let f2 = F0_RLB.min(fs * 0.49);
        let k2 = (PI * f2 / fs).tan();
        let a0_2 = 1.0 + k2 / Q_RLB + k2 * k2;
        let stage2 = BiquadCoeffs {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: 2.0 * (k2 * k2 - 1.0) / a0_2,
            a2: (1.0 - k2 / Q_RLB + k2 * k2) / a0_2,
        };

        Self {
            stage1,
            stage2,
            states: vec![[BiquadState::default(); 2]; channels],
        }
    }

    #[inline]
    fn process_sample(&mut self, ch: usize, x: f64) -> f64 {
        let s = &mut self.states[ch];
        let y1 = s[0].process(&self.stage1, x);
        s[1].process(&self.stage2, y1)
    }
}

fn channel_weights(channels: usize) -> Vec<f64> {
    let mut weights = vec![1.0_f64; channels];
    match channels {
        4 => {
            // L, R, Ls, Rs
            weights[2] = 1.41;
            weights[3] = 1.41;
        }
        5 => {
            // L, R, C, Ls, Rs
            weights[3] = 1.41;
            weights[4] = 1.41;
        }
        n if n >= 6 => {
            // 5.1+ layout: L, R, C, LFE (0.0), Ls (+1.5 dB), Rs (+1.5 dB)
            weights[3] = 0.0;
            weights[4] = 1.41;
            weights[5] = 1.41;
        }
        _ => {}
    }
    weights
}

#[derive(Debug, Clone, Copy)]
struct LoudnessSummary {
    integrated_lufs: f64,
    short_term_max_lufs: f64,
    momentary_max_lufs: f64,
}

fn compute_loudness_metrics(samples: &[f32], channels: usize, sample_rate: u32) -> LoudnessSummary {
    let empty = LoudnessSummary {
        integrated_lufs: f64::NEG_INFINITY,
        short_term_max_lufs: f64::NEG_INFINITY,
        momentary_max_lufs: f64::NEG_INFINITY,
    };
    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return empty;
    }
    let total_frames = samples.len() / channels;
    if total_frames == 0 {
        return empty;
    }

    let hop_frames = ((0.1 * f64::from(sample_rate)).round() as usize).max(1);
    let weights = channel_weights(channels);
    let mut filter = KWeightFilter::new(channels, sample_rate);

    let est_chunks = total_frames / hop_frames + 1;
    let mut chunk_energies = Vec::with_capacity(est_chunks);
    let mut current_chunk_sum = 0.0_f64;
    let mut current_chunk_frames = 0_usize;
    let mut total_energy = 0.0_f64;

    for frame in samples.chunks_exact(channels) {
        let mut frame_weighted_sq = 0.0_f64;
        for (ch, &sample) in frame.iter().enumerate() {
            let y = filter.process_sample(ch, sanitize_f64(sample));
            frame_weighted_sq += weights[ch] * y * y;
        }
        total_energy += frame_weighted_sq;
        current_chunk_sum += frame_weighted_sq;
        current_chunk_frames += 1;
        if current_chunk_frames == hop_frames {
            chunk_energies.push(current_chunk_sum);
            current_chunk_sum = 0.0;
            current_chunk_frames = 0;
        }
    }

    // Gating block: 400 ms = 4 x 100 ms chunks, step = 100 ms (75% overlap).
    let block_chunks = 4_usize;
    let block_frames = block_chunks * hop_frames;

    let mut block_powers = Vec::new();
    if chunk_energies.len() >= block_chunks {
        let inv_block_frames = 1.0 / block_frames as f64;
        for w in chunk_energies.windows(block_chunks) {
            let sum: f64 = w.iter().sum();
            block_powers.push(sum * inv_block_frames);
        }
    } else {
        // Fallback for clips shorter than 400 ms so short renders produce a valid gated reading.
        block_powers.push(total_energy / total_frames as f64);
    }

    let momentary_max_lufs = block_powers
        .iter()
        .map(|&p| power_to_lufs(p))
        .fold(f64::NEG_INFINITY, f64::max);

    // Stage 1: absolute gate at -70.0 LKFS.
    const ABS_GATE_LUFS: f64 = -70.0;
    let mut abs_sum = 0.0_f64;
    let mut abs_count = 0_usize;
    for &p in &block_powers {
        if power_to_lufs(p) > ABS_GATE_LUFS {
            abs_sum += p;
            abs_count += 1;
        }
    }

    let integrated_lufs = if abs_count == 0 {
        f64::NEG_INFINITY
    } else {
        let rel_threshold = power_to_lufs(abs_sum / abs_count as f64) - 10.0;
        let mut rel_sum = 0.0_f64;
        let mut rel_count = 0_usize;
        for &p in &block_powers {
            let l = power_to_lufs(p);
            if l > ABS_GATE_LUFS && l > rel_threshold {
                rel_sum += p;
                rel_count += 1;
            }
        }
        if rel_count == 0 {
            f64::NEG_INFINITY
        } else {
            power_to_lufs(rel_sum / rel_count as f64)
        }
    };

    // Short-term max: 3.0 s window (30 x 100 ms chunks), falling back to full clip if < 3.0 s.
    let short_chunks = 30_usize;
    let short_term_max_lufs = if chunk_energies.len() >= short_chunks {
        let inv_short_frames = 1.0 / (short_chunks * hop_frames) as f64;
        chunk_energies
            .windows(short_chunks)
            .map(|w| power_to_lufs(w.iter().sum::<f64>() * inv_short_frames))
            .fold(f64::NEG_INFINITY, f64::max)
    } else {
        power_to_lufs(total_energy / total_frames as f64)
    };

    LoudnessSummary {
        integrated_lufs,
        short_term_max_lufs,
        momentary_max_lufs,
    }
}

/// Computes ITU-R BS.1770-4 integrated loudness in LUFS (K-weighted, 400 ms / 100 ms blocks,
/// -70.0 LKFS absolute gate, -10.0 LU relative gate).
pub fn integrated_lufs(samples: &[f32], channels: usize, sample_rate: u32) -> f64 {
    compute_loudness_metrics(samples, channels, sample_rate).integrated_lufs
}

/// Computes the maximum momentary (400 ms) loudness in LUFS.
pub fn momentary_max_lufs(samples: &[f32], channels: usize, sample_rate: u32) -> f64 {
    compute_loudness_metrics(samples, channels, sample_rate).momentary_max_lufs
}

/// Computes the maximum short-term (3.0 s) loudness in LUFS.
pub fn short_term_max_lufs(samples: &[f32], channels: usize, sample_rate: u32) -> f64 {
    compute_loudness_metrics(samples, channels, sample_rate).short_term_max_lufs
}

/// Computes the 28 standard IEC 61260 1/3-octave band energies (31.5 Hz to 16000 Hz) in dBFS
/// using a Hann-windowed averaged PSD via `realfft`.
///
/// Multi-channel buffers average per-channel band power so opposite-phase channels never cancel.
pub fn third_octave_bands(samples: &[f32], channels: usize, sample_rate: u32) -> Vec<BandEnergy> {
    let empty = || {
        THIRD_OCTAVE_CENTERS_HZ
            .iter()
            .map(|&center_hz| BandEnergy {
                center_hz,
                energy_db: f64::NEG_INFINITY,
            })
            .collect()
    };

    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return empty();
    }
    let total_frames = samples.len() / channels;
    if total_frames == 0 {
        return empty();
    }

    // Ensure <= ~3 Hz bin spacing so the lowest 31.5 Hz band (~7.3 Hz wide) captures multiple bins.
    let fft_size = ((f64::from(sample_rate) / 3.0).ceil() as usize)
        .next_power_of_two()
        .clamp(16_384, 65_536);

    let win_len = total_frames.min(fft_size);
    let mut hann = Vec::with_capacity(win_len);
    let mut sum_w2 = 0.0_f64;
    if win_len == 1 {
        hann.push(1.0);
        sum_w2 = 1.0;
    } else {
        for n in 0..win_len {
            let w = 0.5 * (1.0 - (2.0 * PI * n as f64 / win_len as f64).cos());
            hann.push(w);
            sum_w2 += w * w;
        }
    }

    let mut starts = Vec::new();
    if total_frames <= fft_size {
        starts.push(0_usize);
    } else {
        let hop = fft_size / 2;
        let mut s = 0_usize;
        while s + fft_size <= total_frames {
            starts.push(s);
            s += hop;
        }
        if let Some(&last) = starts.last()
            && last + fft_size < total_frames
        {
            starts.push(total_frames - fft_size);
        }
    }

    let mut planner = RealFftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut fft_in = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();

    let nyquist_bin = fft_size / 2;
    let mut avg_psd = vec![0.0_f64; nyquist_bin + 1];
    let norm = 1.0 / (fft_size as f64 * sum_w2 * starts.len() as f64 * channels as f64);

    for ch in 0..channels {
        for &start in &starts {
            fft_in.fill(0.0);
            for n in 0..win_len {
                let sample_idx = (start + n) * channels + ch;
                fft_in[n] = sanitize_f64(samples[sample_idx]) * hann[n];
            }
            if fft
                .process_with_scratch(&mut fft_in, &mut spectrum, &mut scratch)
                .is_err()
            {
                return empty();
            }
            for (bin_idx, bin) in spectrum.iter().enumerate() {
                let weight = if bin_idx == 0 || bin_idx == nyquist_bin {
                    1.0
                } else {
                    2.0
                };
                avg_psd[bin_idx] += bin.norm_sqr() * weight * norm;
            }
        }
    }

    let bin_hz = f64::from(sample_rate) / fft_size as f64;
    let mut bands = Vec::with_capacity(THIRD_OCTAVE_CENTERS_HZ.len());
    let mut next_bin = 1_usize;

    for (idx, &center_hz) in THIRD_OCTAVE_CENTERS_HZ.iter().enumerate() {
        let k_band = idx as i32 - 15;
        let exact_center = 1000.0 * 10.0_f64.powf(f64::from(k_band) / 10.0);
        let f_low = exact_center * 10.0_f64.powf(-0.05);
        let f_high = exact_center * 10.0_f64.powf(0.05);

        while next_bin <= nyquist_bin && (next_bin as f64 * bin_hz) < f_low {
            next_bin += 1;
        }
        let mut band_power = 0.0_f64;
        let mut b = next_bin;
        while b <= nyquist_bin && (b as f64 * bin_hz) < f_high {
            band_power += avg_psd[b];
            b += 1;
        }
        next_bin = b;

        bands.push(BandEnergy {
            center_hz,
            energy_db: power_to_dbfs(band_power),
        });
    }

    bands
}

/// Returns the maximum absolute dB difference across all 28 1/3-octave bands (31.5 Hz to 16 kHz)
/// between `reference` and `candidate`, ignoring bands where both are below `-100.0 dBFS`.
pub fn max_third_octave_diff_db(
    reference: &[f32],
    candidate: &[f32],
    channels: usize,
    sample_rate: u32,
) -> f64 {
    const FLOOR_DB: f64 = -100.0;
    let ref_bands = third_octave_bands(reference, channels, sample_rate);
    let cand_bands = third_octave_bands(candidate, channels, sample_rate);

    let mut max_diff = 0.0_f64;
    for (r, c) in ref_bands.iter().zip(cand_bands.iter()) {
        if r.energy_db < FLOOR_DB && c.energy_db < FLOOR_DB {
            continue;
        }
        let diff = (r.energy_db - c.energy_db).abs();
        if diff.is_nan() {
            return f64::INFINITY;
        }
        if diff > max_diff {
            max_diff = diff;
        }
    }
    max_diff
}

/// Computes the full [`AudioMetrics`] summary for an interleaved audio buffer.
pub fn analyze_audio(samples: &[f32], channels: usize, sample_rate: u32) -> AudioMetrics {
    let loudness = compute_loudness_metrics(samples, channels, sample_rate);
    AudioMetrics {
        peak_dbfs: peak_dbfs(samples),
        true_peak_dbtp: true_peak_dbtp(samples, channels, sample_rate),
        rms_dbfs: rms_dbfs(samples),
        integrated_lufs: loudness.integrated_lufs,
        short_term_max_lufs: loudness.short_term_max_lufs,
        momentary_max_lufs: loudness.momentary_max_lufs,
    }
}
