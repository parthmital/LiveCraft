use super::*;

#[test]
fn test_eq8_processing() {
    let mut eq = EqEight::new(44100.0);
    // Disable other default bands to isolate band 2
    for i in 0..8 {
        if i != 2 {
            eq.set_band_enabled(i, false);
        }
    }
    eq.set_band_params(2, 500.0, 6.0, 1.0);
    let (out_l, out_r) = eq.process_stereo(1.0, 1.0);
    assert!(out_l.is_finite());
    assert!(out_r.is_finite());

    // Magnitude response verification at center frequency (+6 dB peak)
    let mag = eq.magnitude_at_freq(500.0);
    let gain_db = eq.gain_db_at_freq(500.0);
    assert!((gain_db - 6.0).abs() < 0.5);
    assert!((mag - 10.0f32.powf(6.0 / 20.0)).abs() < 0.15);

    // Curve plotting points verification
    let points = eq.response_curve_points(100);
    assert_eq!(points.len(), 100);
    assert!(points[0].0 >= 19.9 && points[0].0 <= 20.1);
    for p in &points {
        assert!(p.0.is_finite());
        assert!(p.1.is_finite());
    }

    // Band type switching: Notch filter
    eq.set_band_type(2, FilterType::Notch);
    let notch_mag = eq.magnitude_at_freq(500.0);
    assert!(notch_mag < 0.05);

    // Disabling band returns flat response
    eq.set_band_enabled(2, false);
    let flat_mag = eq.magnitude_at_freq(500.0);
    assert!((flat_mag - 1.0).abs() < 1e-4);
}

#[test]
fn test_saturator_processing() {
    let mut sat = SaturatorEffect::new(44100.0);
    sat.drive_db = 12.0;
    let (out_l, out_r) = sat.process_stereo(0.5, -0.5);
    assert!(out_l.is_finite());
    assert!(out_r.is_finite());
    assert!(out_l > 0.0);
    assert!(out_r < 0.0);

    // Test all curve modes
    for &curve in &[
        SaturatorCurve::SoftSine,
        SaturatorCurve::HardClip,
        SaturatorCurve::AnalogWarmth,
        SaturatorCurve::Foldback,
    ] {
        sat.curve = curve;
        let (l, r) = sat.process_stereo(1.5, -1.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }
}

#[test]
fn test_delay_processing() {
    let mut delay = DelayEffect::new(44100.0);
    delay.delay_time_ms = 100.0;
    delay.feedback = 0.5;
    delay.dry_wet = 0.5;

    // Feed impulse
    let (first_l, _) = delay.process_stereo(1.0, 0.0);
    assert_eq!(first_l, 0.5); // 50% dry

    // Process silence until delayed tap arrives
    let mut max_delayed = 0.0f32;
    for _ in 0..5000 {
        let (l, _) = delay.process_stereo(0.0, 0.0);
        max_delayed = max_delayed.max(l.abs());
    }
    assert!(max_delayed > 0.01);
}

#[test]
fn test_reverb_processing() {
    let mut rev = ReverbEffect::new(44100.0);
    rev.set_params(0.8, 0.3, 0.5);

    // Feed impulse
    rev.process_stereo(1.0, 1.0);
    let mut tail_energy = 0.0f32;
    for _ in 0..10000 {
        let (l, r) = rev.process_stereo(0.0, 0.0);
        tail_energy += l.abs() + r.abs();
    }
    assert!(tail_energy > 0.001);
}

#[test]
fn test_compressor_gain_reduction() {
    let mut comp = CompressorEffect::new(44100.0);
    comp.threshold_db = -20.0;
    comp.ratio = 4.0;
    comp.attack_ms = 1.0;

    // Feed heavy signal (+6 dB lin = 2.0)
    for _ in 0..500 {
        comp.process_stereo(2.0, 2.0);
    }

    // Gain reduction should be active (greater than 0 dB)
    assert!(comp.gain_reduction_db > 2.0);
}

#[test]
fn test_utility_effect() {
    let util = UtilityEffect {
        gain_db: 6.0,
        width: 0.0, // Mono
        ..Default::default()
    };
    let (mono_l, mono_r) = util.process_stereo(1.0, -1.0);
    // Phase inverted inputs cancelled in mono
    assert!(mono_l.abs() < 1e-4);
    assert!(mono_r.abs() < 1e-4);
}

#[test]
fn test_operator_synth_note() {
    let mut synth = OperatorSynth::new(44100.0);
    synth.note_on(60, 100); // Middle C

    let mut samples = Vec::new();
    for _ in 0..500 {
        let (l, _) = synth.process_sample();
        samples.push(l);
    }

    let max_amp = samples.iter().copied().fold(0.0f32, f32::max);
    assert!(max_amp > 0.01);

    // Test velocity scaling: quiet note vs loud note
    let mut synth_quiet = OperatorSynth::new(44100.0);
    synth_quiet.note_on(60, 20);
    let mut quiet_peak = 0.0f32;
    for _ in 0..500 {
        let (l, _) = synth_quiet.process_sample();
        quiet_peak = quiet_peak.max(l.abs());
    }

    let mut synth_loud = OperatorSynth::new(44100.0);
    synth_loud.note_on(60, 127);
    let mut loud_peak = 0.0f32;
    for _ in 0..500 {
        let (l, _) = synth_loud.process_sample();
        loud_peak = loud_peak.max(l.abs());
    }
    assert!(loud_peak > quiet_peak * 2.0);

    // Test voice stealing with 20 simultaneous notes (synth has 16 voices)
    let mut poly_synth = OperatorSynth::new(44100.0);
    for n in 40..60 {
        poly_synth.note_on(n, 90);
    }
    for _ in 0..100 {
        let (l, r) = poly_synth.process_sample();
        assert!(l.is_finite());
        assert!(r.is_finite());
    }
}

#[test]
fn test_operator_all_waveforms() {
    let waves = [
        Waveform::Sine,
        Waveform::Triangle,
        Waveform::Sawtooth,
        Waveform::Square,
        Waveform::Noise,
    ];

    for &w in &waves {
        let mut synth = OperatorSynth::new(44100.0);
        synth.osc1_wave = w;
        synth.note_on(64, 100);
        let mut energy = 0.0f32;
        for _ in 0..100 {
            let (l, _) = synth.process_sample();
            energy += l.abs();
        }
        assert!(energy > 0.001);
    }

    // Test all 11 FM routing algorithms and feedback
    for alg in 1..=11 {
        let mut synth = OperatorSynth::new(44100.0);
        synth.algorithm = alg;
        synth.feedback = 0.5;
        synth.ops[0].level = 0.8;
        synth.ops[1].level = 0.6;
        synth.ops[2].level = 0.5;
        synth.ops[3].level = 0.5;
        synth.note_on(60, 100);

        let mut alg_energy = 0.0f32;
        for _ in 0..100 {
            let (l, r) = synth.process_sample();
            assert!(l.is_finite(), "Algorithm {} produced non-finite left", alg);
            assert!(r.is_finite(), "Algorithm {} produced non-finite right", alg);
            alg_energy += l.abs();
        }
        assert!(alg_energy > 0.001, "Algorithm {} produced no energy", alg);
    }
}

#[test]
fn test_limiter_brickwall() {
    let mut limiter = LimiterEffect::new(44100.0);
    limiter.ceiling_db = -0.3;

    let mut max_out = 0.0f32;
    for _ in 0..100 {
        let (l, r) = limiter.process_stereo(5.0, 5.0);
        max_out = max_out.max(l.abs()).max(r.abs());
    }

    assert!(max_out <= 1.05);
}

#[test]
fn test_drum_rack_synthesis_and_choke() {
    let mut dr = DrumRackInstrument::new(44100.0);
    assert_eq!(dr.pads.len(), 16);

    // Trigger Kick (note 36)
    dr.note_on(36, 110);
    let mut kick_energy = 0.0f32;
    for _ in 0..100 {
        let (l, r) = dr.process_sample();
        kick_energy += l.abs() + r.abs();
    }
    assert!(kick_energy > 0.0);

    // Trigger Open Hi-Hat (note 46, choke group 1)
    dr.note_on(46, 100);
    let open_hh = dr.pads.iter().find(|p| p.note == 46).unwrap();
    assert!(open_hh.amp_env > 0.0);

    // Trigger Closed Hi-Hat (note 42, choke group 1) -> should choke Open Hi-Hat
    dr.note_on(42, 100);
    let open_hh_after = dr.pads.iter().find(|p| p.note == 46).unwrap();
    assert!(!open_hh_after.is_active);
    assert_eq!(open_hh_after.amp_env, 0.0);

    // Test Pad Solo and Mute routing
    dr.all_notes_off();
    // Mute Kick (36)
    dr.pads.iter_mut().find(|p| p.note == 36).unwrap().is_muted = true;
    dr.note_on(36, 100);
    let (mut m_l, mut m_r) = (0.0f32, 0.0f32);
    for _ in 0..50 {
        let (l, r) = dr.process_sample();
        m_l += l.abs();
        m_r += r.abs();
    }
    assert_eq!(m_l, 0.0);
    assert_eq!(m_r, 0.0);
    dr.pads.iter_mut().find(|p| p.note == 36).unwrap().is_muted = false;

    // Solo Snare 1 (38) while Kick (36) and Snare 1 (38) are triggered
    dr.all_notes_off();
    dr.pads.iter_mut().find(|p| p.note == 38).unwrap().is_soloed = true;
    dr.note_on(36, 100);
    dr.note_on(38, 100);
    let mut snare_only_energy = 0.0f32;
    for _ in 0..50 {
        let (l, r) = dr.process_sample();
        snare_only_energy += l.abs() + r.abs();
    }
    assert!(snare_only_energy > 0.0);
    dr.pads.iter_mut().find(|p| p.note == 38).unwrap().is_soloed = false;

    // Test per-pad stereo pan
    dr.all_notes_off();
    let clap = dr.pads.iter_mut().find(|p| p.note == 39).unwrap();
    clap.pan = -1.0; // Hard left
    dr.note_on(39, 100);
    let (mut pan_l, mut pan_r) = (0.0f32, 0.0f32);
    for _ in 0..50 {
        let (l, r) = dr.process_sample();
        pan_l += l.abs();
        pan_r += r.abs();
    }
    assert!(pan_l > 0.01);
    assert!(pan_r < 0.001);
}

#[test]
fn test_autofilter_modulation() {
    let mut af = AutoFilterEffect::new(44100.0);
    af.cutoff = 1000.0;
    af.lfo_rate = 2.0;
    af.lfo_depth = 0.5;

    for _ in 0..200 {
        let (l, r) = af.process_stereo(0.3, -0.3);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    // Test all filter modes
    for mode in &[
        AutoFilterMode::LowPass,
        AutoFilterMode::HighPass,
        AutoFilterMode::BandPass,
        AutoFilterMode::Notch,
    ] {
        af.mode = *mode;
        let (l, r) = af.process_stereo(0.5, 0.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }
}

#[test]
fn test_chorus_processing() {
    let mut chorus = ChorusEffect::new(44100.0);
    chorus.rate_hz = 1.5;
    chorus.depth = 0.7;
    chorus.dry_wet = 0.5;

    for _ in 0..500 {
        let (l, r) = chorus.process_stereo(0.4, 0.4);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }
}

#[test]
fn test_autopan_modulation() {
    let mut ap = AutoPanEffect::new(44100.0);
    ap.rate_hz = 2.0;
    ap.amount = 1.0;
    ap.phase_deg = 180.0;
    ap.mode = 0.0; // Panning

    let mut left = vec![1.0; 256];
    let mut right = vec![1.0; 256];
    ap.process(&mut left, &mut right);

    for i in 0..256 {
        assert!(left[i] >= 0.0 && left[i] <= 1.0);
        assert!(right[i] >= 0.0 && right[i] <= 1.0);
    }
}

#[test]
fn test_gate_dynamics() {
    let mut gate = GateEffect::new(44100.0);
    gate.threshold_db = -20.0;
    gate.floor_db = -60.0;

    // Very quiet signal (-40 dB): should be heavily attenuated
    let mut quiet_l = vec![0.01; 500];
    let mut quiet_r = vec![0.01; 500];
    gate.process(&mut quiet_l, &mut quiet_r);
    assert!(quiet_l[499] < 0.001);

    // Loud signal (0 dB): should open gate
    let mut loud_l = vec![0.9; 500];
    let mut loud_r = vec![0.9; 500];
    gate.process(&mut loud_l, &mut loud_r);
    assert!(loud_l[499] > 0.5);
}

#[test]
fn test_eq3_processing() {
    let mut eq = EqThreeEffect::new(44100.0);
    let mut left = vec![0.5; 128];
    let mut right = vec![0.5; 128];
    eq.process(&mut left, &mut right);
    for &val in &left[10..128] {
        assert!((val - 0.5).abs() < 0.05);
    }

    // Kill low band
    eq.low_on = false;
    let mut low_l = vec![0.5; 128];
    let mut low_r = vec![0.5; 128];
    eq.process(&mut low_l, &mut low_r);
    assert!(low_l[127].is_finite());
}

#[test]
fn test_redux_quantization() {
    let mut redux = ReduxEffect::new(44100.0);
    redux.rate_factor = 4.0;
    redux.bits = 4.0; // 16 levels
    let mut left = vec![0.333333; 64];
    let mut right = vec![0.333333; 64];
    redux.process(&mut left, &mut right);
    for &val in &left {
        assert!(val.is_finite());
    }
}

#[test]
fn test_simpler_playback_modes() {
    let sample_rate = 44100.0;
    let mut simpler = SimplerInstrument::new(sample_rate);

    // Missing sample outputs silence without panic
    simpler.note_on(60, 100);
    let (sil_l, sil_r) = simpler.process_sample();
    assert_eq!(sil_l, 0.0);
    assert_eq!(sil_r, 0.0);

    // Synthesize 1000 samples of test audio (sine wave)
    let samples_l: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.1).sin() * 0.8).collect();
    let samples_r = samples_l.clone();
    simpler.set_sample_data(samples_l, samples_r, 44100);

    // 1. Classic Mode (Polyphonic)
    simpler.mode = SimplerPlaybackMode::Classic;
    simpler.is_looping = true;
    simpler.crossfade_pct = 0.2;
    simpler.note_on(60, 100);
    simpler.note_on(64, 90);

    let mut classic_energy = 0.0f32;
    for _ in 0..100 {
        let (l, r) = simpler.process_sample();
        assert!(l.is_finite());
        assert!(r.is_finite());
        classic_energy += l.abs();
    }
    assert!(classic_energy > 0.01);
    simpler.all_notes_off();

    // 2. OneShot Mode
    simpler.mode = SimplerPlaybackMode::OneShot;
    simpler.gate_mode = false;
    simpler.note_on(60, 100);
    let mut oneshot_energy = 0.0f32;
    for _ in 0..100 {
        let (l, _) = simpler.process_sample();
        oneshot_energy += l.abs();
    }
    assert!(oneshot_energy > 0.01);
    simpler.all_notes_off();

    // 3. Slicing Mode
    simpler.mode = SimplerPlaybackMode::Slicing;
    simpler.slices = vec![0, 250, 500, 750, 1000];
    // Trigger slice 1 (note 37)
    simpler.note_on(37, 100);
    assert_eq!(simpler.voices[0].start_sample, 250);
    assert_eq!(simpler.voices[0].end_sample, 500);

    let mut slice_energy = 0.0f32;
    for _ in 0..100 {
        let (l, _) = simpler.process_sample();
        slice_energy += l.abs();
    }
    assert!(slice_energy > 0.01);
}
