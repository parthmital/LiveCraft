pub mod biquad;
pub mod bouncer;
pub mod compressor;
pub mod delay;
pub mod engine;
pub mod eq8;
pub mod graph;
pub mod limiter;
pub mod operator;
pub mod reverb;
pub mod simpler;

pub use biquad::*;
pub use compressor::*;
pub use delay::*;
pub use eq8::*;
pub use limiter::*;
pub use operator::*;
pub use reverb::*;
pub mod instruments {
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
        let mut graph = AudioGraph::new(&project, 44100.0);

        // Put active notes on track 0
        let mut clip = Clip::new_midi(1, "Test", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(60, 0.0, 4.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);
        graph.trigger_clip(0, 0);
        graph.is_playing = true;

        let mut buf_l = vec![0.0f32; 256];
        let mut buf_r = vec![0.0f32; 256];
        graph.process_buffer(&mut buf_l, &mut buf_r, &project);

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
}
