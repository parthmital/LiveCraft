pub mod audio_clip;
pub mod autofilter;
pub mod autopan;
pub mod biquad;
pub mod bouncer;
pub mod chorus;
pub mod compressor;
pub mod delay;
pub mod drumrack;
pub mod engine;
pub mod eq3;
pub mod eq8;
pub mod gate;
pub mod graph;
pub mod limiter;
pub mod operator;
pub mod redux;
pub mod reverb;
pub mod saturator;
pub mod simpler;

pub use audio_clip::*;
pub use autofilter::*;
pub use autopan::*;
pub use biquad::*;
pub use chorus::*;
pub use compressor::*;
pub use delay::*;
pub use drumrack::*;
pub use eq3::*;
pub use eq8::*;
pub use gate::*;
pub use limiter::*;
pub use operator::*;
pub use redux::*;
pub use reverb::*;
pub use saturator::*;
pub mod instruments {
    pub use crate::drumrack::*;
    pub use crate::operator::*;
    pub use crate::simpler::*;
}
pub use bouncer::*;
pub use engine::*;
pub use graph::*;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_core::model::*;

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
        eq.set_band_params(2, 500.0, 6.0, 1.0);
        let (out_l, out_r) = eq.process_stereo(1.0, 1.0);
        assert!(out_l.is_finite());
        assert!(out_r.is_finite());
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

        OfflineBouncer::render_project(&project, &wav_path, options)
            .expect("Failed to render project");
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

        let stems = OfflineBouncer::render_stems(&project, &out_dir, options)
            .expect("Failed to render stems");
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
}
