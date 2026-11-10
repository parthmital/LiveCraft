use crate::biquad::*;
use crate::metrics::*;
use std::f64::consts::PI;

#[test]
fn test_biquad_all_filter_types() {
    let types = [
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
        FilterType::Peaking,
        FilterType::LowShelf,
        FilterType::HighShelf,
    ];

    for &ft in &types {
        let mut filter = BiquadFilter::new(ft, 44100.0, 1000.0, 3.0, 0.707);
        for _ in 0..50 {
            let (l, r) = filter.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }
}

fn gen_sine(
    freq_hz: f64,
    amplitude: f64,
    sample_rate: u32,
    seconds: f64,
    channels: usize,
) -> Vec<f32> {
    let frames = (f64::from(sample_rate) * seconds).round() as usize;
    let mut out = Vec::with_capacity(frames * channels);
    for n in 0..frames {
        let s = (amplitude * (2.0 * PI * freq_hz * n as f64 / f64::from(sample_rate)).sin()) as f32;
        for _ in 0..channels {
            out.push(s);
        }
    }
    out
}

#[test]
fn test_analytical_sine_rms_and_peak() {
    let sine = gen_sine(1000.0, 1.0, 48_000, 1.0, 2);
    let peak = peak_dbfs(&sine);
    let rms = rms_dbfs(&sine);
    assert!(peak.abs() < 1e-4, "expected ~0.0 dBFS peak, got {peak}");
    assert!(
        (rms - (-3.0103)).abs() < 1e-3,
        "expected -3.0103 dBFS RMS, got {rms}"
    );
}

#[test]
fn test_intersample_true_peak_exceeds_sample_peak() {
    // fs/4 sine shifted by pi/4: all sample amplitudes are +-0.90 (peak = -0.915 dBFS),
    // while true continuous inter-sample peak is 0.90 * sqrt(2) = 1.2728 (+2.095 dBTP).
    // Tapered edges avoid step discontinuity Gibbs overshoot at buffer boundaries.
    let sr = 48_000_u32;
    let frames = 2048;
    let amp = 0.90 * std::f64::consts::SQRT_2;
    let mut samples = Vec::with_capacity(frames);
    for n in 0..frames {
        let env = if n < 64 {
            n as f64 / 64.0
        } else if n >= frames - 64 {
            (frames - 1 - n) as f64 / 64.0
        } else {
            1.0
        };
        let phase = PI * 0.5 * n as f64 + PI * 0.25;
        samples.push((amp * env * phase.sin()) as f32);
    }
    let sample_peak = peak_dbfs(&samples);
    let tp = true_peak_dbtp(&samples, 1, sr);
    assert!(sample_peak < -0.9, "sample peak={sample_peak}");
    assert!(
        tp > 0.0,
        "expected inter-sample true peak > 0.0 dBTP, got {tp}"
    );
    assert!((tp - 2.095).abs() < 0.05, "expected ~+2.095 dBTP, got {tp}");
}

#[test]
fn test_k_weighted_997hz_lufs() {
    let amp_neg20_peak = 10.0_f64.powf(-20.0 / 20.0);
    // Stereo -20 dBFS peak 997 Hz sine has summed channel power equivalent to -20.0 LUFS.
    for sr in [44_100_u32, 48_000, 96_000] {
        let stereo = gen_sine(997.0, amp_neg20_peak, sr, 2.0, 2);
        let lufs = integrated_lufs(&stereo, 2, sr);
        assert!(
            (lufs - (-20.0)).abs() <= 0.15,
            "sr={sr}: expected -20.0 LUFS +-0.15, got {lufs}"
        );
    }

    // Mono -20 dBFS RMS sine (peak = 0.1 * sqrt(2)) also measures -20.0 LUFS.
    let mono = gen_sine(
        997.0,
        amp_neg20_peak * std::f64::consts::SQRT_2,
        48_000,
        2.0,
        1,
    );
    let mono_lufs = integrated_lufs(&mono, 1, 48_000);
    assert!(
        (mono_lufs - (-20.0)).abs() <= 0.15,
        "mono expected -20.0 LUFS +-0.15, got {mono_lufs}"
    );
}

#[test]
fn test_lufs_gating_ignores_quiet_segments_and_silence() {
    let sr = 48_000_u32;
    assert_eq!(
        integrated_lufs(&vec![0.0; 48_000 * 2], 2, sr),
        f64::NEG_INFINITY
    );

    // Sub-threshold (-75 dBFS) audio fails the -70 LKFS absolute gate completely.
    let sub_amp = 10.0_f64.powf(-75.0 / 20.0);
    let quiet_only = gen_sine(997.0, sub_amp, sr, 2.0, 2);
    assert_eq!(integrated_lufs(&quiet_only, 2, sr), f64::NEG_INFINITY);

    // 4s at -20 dBFS followed by 8s at -35 dBFS: ungated average would be ~ -24.5 LUFS,
    // while BS.1770 relative gate (-10 LU) excludes the -35 dBFS blocks (exact transition
    // 400 ms boundary windows pull by only ~0.17 LU).
    let amp_20 = 10.0_f64.powf(-20.0 / 20.0);
    let amp_35 = 10.0_f64.powf(-35.0 / 20.0);
    let mut signal = gen_sine(997.0, amp_20, sr, 4.0, 2);
    signal.extend(gen_sine(997.0, amp_35, sr, 8.0, 2));
    let lufs = integrated_lufs(&signal, 2, sr);
    assert!(
        (lufs - (-20.17)).abs() <= 0.20,
        "gated LUFS should exclude -35 dBFS section, got {lufs}"
    );
}

#[test]
fn test_third_octave_1khz_band_peak_and_diff() {
    let sr = 48_000_u32;
    let sine_1k = gen_sine(1000.0, 1.0, sr, 1.0, 2);
    let bands = third_octave_bands(&sine_1k, 2, sr);
    assert_eq!(bands.len(), 28);
    assert!((bands[0].center_hz - 31.5).abs() < 1e-9);
    assert!((bands[27].center_hz - 16_000.0).abs() < 1e-9);

    let peak_band = bands
        .iter()
        .max_by(|a, b| a.energy_db.total_cmp(&b.energy_db))
        .unwrap();
    assert_eq!(peak_band.center_hz, 1000.0);
    assert!(
        (peak_band.energy_db - (-3.0103)).abs() < 0.2,
        "1 kHz 0 dBFS sine band energy expected ~ -3.01 dBFS, got {}",
        peak_band.energy_db
    );

    let scaled_sine = gen_sine(1000.0, 10.0_f64.powf(-0.5 / 20.0), sr, 1.0, 2);
    let diff = max_third_octave_diff_db(&sine_1k, &scaled_sine, 2, sr);
    assert!(
        (diff - 0.5).abs() < 0.15,
        "expected ~0.5 dB diff, got {diff}"
    );

    // Noise-floor bands below -100 dBFS are ignored by max_third_octave_diff_db.
    let sub_a = gen_sine(250.0, 10.0_f64.powf(-110.0 / 20.0), sr, 1.0, 2);
    let sub_b = gen_sine(4000.0, 10.0_f64.powf(-115.0 / 20.0), sr, 1.0, 2);
    assert_eq!(max_third_octave_diff_db(&sub_a, &sub_b, 2, sr), 0.0);

    // Short buffers (< fft_size) still resolve 1 kHz peak accurately.
    let short_1k = gen_sine(1000.0, 1.0, sr, 0.05, 1);
    let short_bands = third_octave_bands(&short_1k, 1, sr);
    let short_peak = short_bands
        .iter()
        .max_by(|a, b| a.energy_db.total_cmp(&b.energy_db))
        .unwrap();
    assert_eq!(short_peak.center_hz, 1000.0);
}

#[test]
fn test_residual_rms_null_vs_shifted() {
    let sine = gen_sine(440.0, 0.5, 48_000, 0.5, 2);
    assert_eq!(residual_rms_dbfs(&sine, &sine), f64::NEG_INFINITY);
    assert_eq!(relative_residual_rms_dbfs(&sine, &sine), f64::NEG_INFINITY);

    // Micro-perturbation at -100 dBFS passes <= -90 dBFS gate.
    let eps = 10.0_f32.powf(-100.0 / 20.0);
    let perturbed: Vec<f32> = sine
        .iter()
        .enumerate()
        .map(|(i, &s)| if i % 2 == 0 { s + eps } else { s - eps })
        .collect();
    let res = residual_rms_dbfs(&sine, &perturbed);
    assert!(res <= -90.0, "expected <= -90 dBFS, got {res}");

    // 1-sample shift at 440 Hz / 48 kHz produces ~ -33.8 dBFS residual (clearly fails <= -90 dBFS).
    let mut shifted = vec![0.0_f32; 2];
    shifted.extend_from_slice(&sine[..sine.len() - 2]);
    let shifted_res = residual_rms_dbfs(&sine, &shifted);
    assert!(
        shifted_res > -40.0,
        "expected large residual, got {shifted_res}"
    );
}

#[test]
fn test_analyze_audio_populates_all_fields() {
    let sr = 48_000_u32;
    let amp = 10.0_f64.powf(-20.0 / 20.0);
    let stereo = gen_sine(997.0, amp, sr, 4.0, 2);
    let metrics = analyze_audio(&stereo, 2, sr);
    assert!((metrics.peak_dbfs - (-20.0)).abs() < 0.01);
    assert!((metrics.true_peak_dbtp - (-20.0)).abs() < 0.05);
    assert!((metrics.rms_dbfs - (-23.0103)).abs() < 0.05);
    assert!((metrics.integrated_lufs - (-20.0)).abs() < 0.15);
    assert!((metrics.short_term_max_lufs - (-20.0)).abs() < 0.15);
    assert!((metrics.momentary_max_lufs - (-20.0)).abs() < 0.15);
}
