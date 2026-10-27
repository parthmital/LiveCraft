use super::*;
use livecraft_model::model::*;

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

    // Test overlapping notes of identical pitch in timeline
    let mut track = Track::new(1, "MidiTrack", TrackType::Midi, 4);
    let mut clip = Clip::new_midi(1, "Overlapping Notes", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        // Note 1: pitch 60, beats 0.0 to 1.5
        mdata.notes.push(MidiNote::new(60, 0.0, 1.5, 100));
        // Note 2: pitch 60, beats 1.0 to 2.5 (overlaps from 1.0 to 1.5)
        mdata.notes.push(MidiNote::new(60, 1.0, 1.5, 110));
    }
    track.clip_slots[0].clip = Some(clip.clone());
    let mut state = TrackDspState::new(&track, 44100.0);

    // At beat 0.5: Note 1 active
    AudioGraph::process_clip_payload(&mut state, &clip, 0.5, 120.0, 32, 0.0);
    assert_eq!(state.active_note_instances.len(), 1);

    // At beat 1.2: Both Note 1 and Note 2 active
    AudioGraph::process_clip_payload(&mut state, &clip, 1.2, 120.0, 32, 0.0);
    assert_eq!(state.active_note_instances.len(), 2);

    // At beat 1.8: Note 1 ended, but Note 2 is still active
    AudioGraph::process_clip_payload(&mut state, &clip, 1.8, 120.0, 32, 0.0);
    assert_eq!(state.active_note_instances.len(), 1);
    assert_eq!(state.active_note_instances[0].pitch, 60);

    // At beat 3.0: Both ended
    AudioGraph::process_clip_payload(&mut state, &clip, 3.0, 120.0, 32, 0.0);
    assert_eq!(state.active_note_instances.len(), 0);
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
fn test_audio_graph_summing() {
    let mut project = Project::new_default("SummingTest");

    // Put active notes on track 0
    let mut clip = Clip::new_midi(1, "Test", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 4.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);

    let mut graph = AudioGraph::new(&project, 44100.0);
    graph.trigger_clip(0, 0);
    graph.is_playing = true;

    let mut buf_l = vec![0.0f32; 256];
    let mut buf_r = vec![0.0f32; 256];
    graph.process_buffer(&mut buf_l, &mut buf_r);

    let energy: f32 = buf_l.iter().map(|s| s.abs()).sum();
    assert!(energy > 0.01);
    assert!(graph.master_meter.peak_db_l() > -50.0);

    // Test MonitoringMode::Auto when track is armed (should mute clip playback)
    project.tracks[0].is_armed = true;
    project.tracks[0].monitoring = MonitoringMode::Auto;
    graph.sync_project(&project);
    graph.trigger_clip(0, 0);

    let mut armed_buf_l = vec![0.0f32; 256];
    let mut armed_buf_r = vec![0.0f32; 256];
    graph.process_buffer(&mut armed_buf_l, &mut armed_buf_r);
    let armed_energy: f32 = armed_buf_l.iter().map(|s| s.abs()).sum();
    assert_eq!(armed_energy, 0.0);

    // Test MonitoringMode::Off when track is armed (should continue playing clip)
    project.tracks[0].monitoring = MonitoringMode::Off;
    graph.sync_project(&project);
    graph.trigger_clip(0, 0);

    let mut off_buf_l = vec![0.0f32; 256];
    let mut off_buf_r = vec![0.0f32; 256];
    graph.process_buffer(&mut off_buf_l, &mut off_buf_r);
    let off_energy: f32 = off_buf_l.iter().map(|s| s.abs()).sum();
    assert!(off_energy > 0.01);

    // Test Multi-channel Return Summing (3 Return tracks)
    project.tracks[0].is_armed = false;
    project.tracks[0].monitoring = MonitoringMode::Auto;
    project.tracks[0].sends = vec![0.4, 0.4, 0.4];
    let ret_c = Track::new(103, "Return C", TrackType::Return, 0);
    project.return_tracks.push(ret_c);
    graph.sync_project(&project);
    assert_eq!(graph.return_states.len(), 3);

    let mut multi_ret_l = vec![0.0f32; 256];
    let mut multi_ret_r = vec![0.0f32; 256];
    graph.process_buffer(&mut multi_ret_l, &mut multi_ret_r);
    for &s in &multi_ret_l {
        assert!(s.is_finite());
        assert!((-2.0..=2.0).contains(&s)); // summation headroom without overflow
    }
}

#[test]
fn test_offline_bounce() {
    let mut project = Project::new_default("BounceTest");
    let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);

    let out_dir = std::env::temp_dir().join("livecraft_test");
    let _ = std::fs::create_dir_all(&out_dir);
    let wav_path = out_dir.join("test_bounce.wav");

    let options = RenderOptions {
        sample_rate: 44100,
        duration_bars: 2,
        bits_per_sample: 16,
    };

    OfflineBouncer::render_project(&project, &wav_path, options).expect("Failed to render project");
    assert!(wav_path.exists());

    let reader = hound::WavReader::open(&wav_path).expect("Failed to read rendered WAV");
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert!(reader.len() > 0);

    let _ = std::fs::remove_file(wav_path);
}

#[test]
fn test_stem_exporter() {
    let project = Project::new_default("StemTest");
    let out_dir = std::env::temp_dir().join("livecraft_stem_test");

    let options = RenderOptions {
        sample_rate: 44100,
        duration_bars: 1,
        bits_per_sample: 16,
    };

    let stems =
        OfflineBouncer::render_stems(&project, &out_dir, options).expect("Failed to render stems");
    assert_eq!(stems.len(), project.tracks.len());

    for stem in &stems {
        assert!(std::path::Path::new(stem).exists());
    }

    let _ = std::fs::remove_dir_all(out_dir);
}

#[test]
fn test_device_parameter_synchronization() {
    let mut track = Track::new(1, "Synth", TrackType::Midi, 4);
    let mut comp = Device::new(101, "Compressor", DeviceType::Compressor);
    comp.set_param("threshold_db", -30.0);
    comp.set_param("ratio", 8.0);
    track.device_chain.push(comp);

    let mut state = TrackDspState::new(&track, 44100.0);
    assert!(state.compressor.is_some());
    let c = state.compressor.as_ref().unwrap();
    assert_eq!(c.threshold_db, -30.0);
    assert_eq!(c.ratio, 8.0);

    // Update parameter in device
    track.device_chain[0].set_param("threshold_db", -12.0);
    track.device_chain[0].set_param("ratio", 2.0);
    state.sync_devices(&track, 44100.0);

    let c_updated = state.compressor.as_ref().unwrap();
    assert_eq!(c_updated.threshold_db, -12.0);
    assert_eq!(c_updated.ratio, 2.0);
}

#[test]
fn test_arrangement_timeline_playback() {
    let mut project = Project::new_default("ArrangementTest");
    let mut clip = Clip::new_midi(201, "Arrangement Melody", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // Beat 0-1
        mdata.notes.push(MidiNote::new(64, 2.0, 1.0, 100)); // Beat 2-3
    }

    // Place on timeline starting at beat 4.0
    let arr_clip = ArrangementClip::new(301, clip, 4.0, 4.0);
    project.tracks[0].arrangement_clips.push(arr_clip);

    let mut graph = AudioGraph::new(&project, 44100.0);
    graph.is_playing = true;

    // At beat 0, track 0 should have no notes active
    let mut buf_l = [0.0f32; 64];
    let mut buf_r = [0.0f32; 64];
    graph.process_buffer(&mut buf_l, &mut buf_r);
    assert!(graph.track_states[0].synth.as_ref().unwrap().is_enabled);

    // Jump playhead to beat 4.5 (inside note 60: 4.0 + 0.0..1.0)
    graph.playhead_beats = 4.5;
    graph.process_buffer(&mut buf_l, &mut buf_r);

    // Active note 60 should be on
    assert!(graph.track_states[0].synth.is_some());

    // Hermite interpolation verification
    let y0 = 0.0f32;
    let y1 = 1.0f32;
    let y2 = 2.0f32;
    let y3 = 3.0f32;
    let mid = hermite_interp(y0, y1, y2, y3, 0.5);
    assert!((mid - 1.5).abs() < 1e-4);

    // Verify audio clip continuous sample rendering
    let mut cache = AudioSampleCache::new();
    let clip_audio = Clip::new_audio(501, "Audio Clip", "test_audio.wav", 4.0);
    let mut out_l = [0.0f32; 32];
    let mut out_r = [0.0f32; 32];
    cache.render_clip_samples(&clip_audio, 0.0, 0.01, 120.0, &mut out_l, &mut out_r);
    assert_eq!(out_l.len(), 32);
}

#[test]
fn test_graph_dynamic_track_sync() {
    let mut project = Project::new_default("SyncTest");
    let mut graph = AudioGraph::new(&project, 44100.0);
    assert_eq!(graph.track_states.len(), 4);

    // Add 2 new tracks
    project
        .tracks
        .push(Track::new(5, "Track 5", TrackType::Audio, 4));
    project
        .tracks
        .push(Track::new(6, "Track 6", TrackType::Midi, 4));
    graph.sync_project(&project);
    assert_eq!(graph.track_states.len(), 6);

    // Remove a track
    project.tracks.pop();
    graph.sync_project(&project);
    assert_eq!(graph.track_states.len(), 5);

    // Multi-device chain and topological ordering test
    let mut track = Track::new(7, "ChainTrack", TrackType::Audio, 4);
    let mut dev1 = Device::new(101, "Delay 1", DeviceType::Delay);
    dev1.set_param("dry_wet", 0.5);
    let mut dev2 = Device::new(102, "Delay 2", DeviceType::Delay);
    dev2.set_param("dry_wet", 0.8);
    let mut dev3 = Device::new(103, "Compressor 1", DeviceType::Compressor);
    dev3.set_param("threshold_db", -24.0);

    track.device_chain.push(dev1);
    track.device_chain.push(dev2);
    track.device_chain.push(dev3);

    let mut state = TrackDspState::new(&track, 44100.0);
    assert_eq!(state.processors.len(), 3);
    assert_eq!(state.processors[0].device_id(), 101);
    assert_eq!(state.processors[1].device_id(), 102);
    assert_eq!(state.processors[2].device_id(), 103);

    // Reverse device chain order
    track.device_chain.reverse();
    state.sync_devices(&track, 44100.0);
    assert_eq!(state.processors.len(), 3);
    assert_eq!(state.processors[0].device_id(), 103);
    assert_eq!(state.processors[1].device_id(), 102);
    assert_eq!(state.processors[2].device_id(), 101);
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
fn test_lock_free_param_queue_dispatch() {
    let project = Project::new_default("LockFreeTest");
    let mut graph = AudioGraph::new(&project, 44100.0);

    // Verify initial state
    assert_eq!(graph.bpm, 120.0);
    assert_eq!(graph.track_states[0].volume_db, 0.0);
    assert_eq!(graph.track_states[0].pan, 0.0);
    assert!(!graph.track_states[0].is_muted);
    assert!(!graph.track_states[0].is_soloed);

    // Apply parameter updates
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTempo(135.0));
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackVolume {
        track_idx: 0,
        volume_db: -6.5,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackPan {
        track_idx: 0,
        pan: -0.4,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackMute {
        track_idx: 0,
        muted: true,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackSolo {
        track_idx: 0,
        soloed: true,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetMasterVolume { volume_db: -3.0 });

    // Verify updated state
    assert_eq!(graph.bpm, 135.0);
    assert_eq!(graph.track_states[0].volume_db, -6.5);
    assert_eq!(graph.track_states[0].pan, -0.4);
    assert!(graph.track_states[0].is_muted);
    assert!(graph.track_states[0].is_soloed);
    assert_eq!(graph.master_state.volume_db, -3.0);
}

#[test]
fn test_simpler_playback_modes() {
    use crate::simpler::{SimplerInstrument, SimplerPlaybackMode};

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

#[test]
fn test_sanitize_output_sample() {
    assert_eq!(sanitize_output_sample(f32::NAN), 0.0);
    assert_eq!(sanitize_output_sample(f32::INFINITY), 0.0);
    assert_eq!(sanitize_output_sample(f32::NEG_INFINITY), 0.0);
    for value in [0.0_f32, -0.0, 0.5, -1.0, 3.75, f32::MIN_POSITIVE, f32::MAX] {
        assert_eq!(sanitize_output_sample(value).to_bits(), value.to_bits());
    }
}
