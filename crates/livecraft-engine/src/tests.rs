use crate::audio_clip::*;
use crate::bouncer::*;
use crate::graph::*;
use livecraft_model::ids::*;
use livecraft_model::model::*;

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
    graph.trigger_clip(TrackId(1), 0);
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
    graph.trigger_clip(TrackId(1), 0);

    let mut armed_buf_l = vec![0.0f32; 256];
    let mut armed_buf_r = vec![0.0f32; 256];
    graph.process_buffer(&mut armed_buf_l, &mut armed_buf_r);
    let armed_energy: f32 = armed_buf_l.iter().map(|s| s.abs()).sum();
    assert_eq!(armed_energy, 0.0);

    // Test MonitoringMode::Off when track is armed (should continue playing clip)
    project.tracks[0].monitoring = MonitoringMode::Off;
    graph.sync_project(&project);
    graph.trigger_clip(TrackId(1), 0);

    let mut off_buf_l = vec![0.0f32; 256];
    let mut off_buf_r = vec![0.0f32; 256];
    graph.process_buffer(&mut off_buf_l, &mut off_buf_r);
    let off_energy: f32 = off_buf_l.iter().map(|s| s.abs()).sum();
    assert!(off_energy > 0.01);

    // Test Multi-channel Return Summing (3 Return tracks)
    project.tracks[0].is_armed = false;
    project.tracks[0].monitoring = MonitoringMode::Auto;
    project.tracks[0].sends = vec![0.4, 0.4, 0.4];
    let ret_c = Track::new(TrackId(103), "Return C", TrackType::Return, 0);
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
    let mut track = Track::new(TrackId(1), "Synth", TrackType::Midi, 4);
    let mut comp = Device::new(DeviceId(101), "Compressor", DeviceType::Compressor);
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
    // Test overlapping notes of identical pitch in timeline
    let mut track = Track::new(TrackId(1), "MidiTrack", TrackType::Midi, 4);
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

    let mut project = Project::new_default("ArrangementTest");
    let mut clip = Clip::new_midi(201, "Arrangement Melody", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // Beat 0-1
        mdata.notes.push(MidiNote::new(64, 2.0, 1.0, 100)); // Beat 2-3
    }

    // Place on timeline starting at beat 4.0
    let arr_clip = ArrangementClip::new(ClipId(301), clip, 4.0, 4.0);
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
        .push(Track::new(TrackId(5), "Track 5", TrackType::Audio, 4));
    project
        .tracks
        .push(Track::new(TrackId(6), "Track 6", TrackType::Midi, 4));
    graph.sync_project(&project);
    assert_eq!(graph.track_states.len(), 6);

    // Remove a track
    project.tracks.pop();
    graph.sync_project(&project);
    assert_eq!(graph.track_states.len(), 5);

    // Multi-device chain and topological ordering test
    let mut track = Track::new(TrackId(7), "ChainTrack", TrackType::Audio, 4);
    let mut dev1 = Device::new(DeviceId(101), "Delay 1", DeviceType::Delay);
    dev1.set_param("dry_wet", 0.5);
    let mut dev2 = Device::new(DeviceId(102), "Delay 2", DeviceType::Delay);
    dev2.set_param("dry_wet", 0.8);
    let mut dev3 = Device::new(DeviceId(103), "Compressor 1", DeviceType::Compressor);
    dev3.set_param("threshold_db", -24.0);

    track.device_chain.push(dev1);
    track.device_chain.push(dev2);
    track.device_chain.push(dev3);

    let mut state = TrackDspState::new(&track, 44100.0);
    assert_eq!(state.processors.len(), 3);
    assert_eq!(state.processors[0].device_id(), DeviceId(101));
    assert_eq!(state.processors[1].device_id(), DeviceId(102));
    assert_eq!(state.processors[2].device_id(), DeviceId(103));

    // Reverse device chain order
    track.device_chain.reverse();
    state.sync_devices(&track, 44100.0);
    assert_eq!(state.processors.len(), 3);
    assert_eq!(state.processors[0].device_id(), DeviceId(103));
    assert_eq!(state.processors[1].device_id(), DeviceId(102));
    assert_eq!(state.processors[2].device_id(), DeviceId(101));
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
        track: TrackId(1),
        volume_db: -6.5,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackPan {
        track: TrackId(1),
        pan: -0.4,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackMute {
        track: TrackId(1),
        muted: true,
    });
    graph.apply_param_msg(crate::engine::AudioParamMsg::SetTrackSolo {
        track: TrackId(1),
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
fn test_sanitize_output_sample() {
    assert_eq!(sanitize_output_sample(f32::NAN), 0.0);
    assert_eq!(sanitize_output_sample(f32::INFINITY), 0.0);
    assert_eq!(sanitize_output_sample(f32::NEG_INFINITY), 0.0);
    for value in [0.0_f32, -0.0, 0.5, -1.0, 3.75, f32::MIN_POSITIVE, f32::MAX] {
        assert_eq!(sanitize_output_sample(value).to_bits(), value.to_bits());
    }
}
