//! M11 session integration tests:
//! - `SetClipRamMode` + exact undo/redo + `.als` `<Ram>` round-trip
//! - `ReplaceClipSample` & `Session::replace_clip_sample` (same-length retains warp markers,
//!   different-length clears warp markers, preserves clip settings, exact undo/redo, `missing_files`)
//! - `Session::sample_analysis` beside sample vs custom cache folder vs `create_analysis_files = false`
//!   + `.asd` preservation
//! - `Session::refresh_clip_sample` external edit reload (`.lcanalysis`, `sample_rate`, `channels`, engine)
//! - `EngineStatus` surfaces `disk_load` and `disk_underruns`
//! - `Session::poll_background` ingests completed RAM decodes and recompiles graph

use livecraft_engine::compiled::MAX_BLOCK;
use livecraft_engine::driver::{AudioDriver, DriverConfig, DriverError, NullDriver};
use livecraft_engine::output::Playback;
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_formats::analysis_cache::analysis_cache_path;
use livecraft_formats::project_dir::{ProjectContext, create_project_layout};
use livecraft_model::clip::{ArrangementClip, WarpMarker, WarpMode};
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::{Clip, ClipPayload, Project};
use livecraft_session::{
    AnalysisCacheLocation, CommandError, DriverFactory, LibraryPreferences, ReplaceClipSample,
    Session, SessionFormatError, SetClipRamMode,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn test_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m11_session")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create m11 test dir");
    dir
}

fn null_factory() -> DriverFactory {
    Box::new(
        |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            Ok(Box::new(NullDriver::new(
                config.sample_rate.unwrap_or(44_100),
                config.buffer_frames.unwrap_or(512),
                2,
            )))
        },
    )
}

/// Writes a valid 16-bit PCM WAV file with alternating +/- `amplitude` samples.
fn write_pcm_wav(path: &Path, sample_rate: u32, channels: u16, frames: usize, amplitude: i16) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create wav parent");
    }
    let block_align = channels * 2;
    let data_bytes = (frames * usize::from(block_align)) as u32;
    let riff_size = 36 + data_bytes;
    let byte_rate = sample_rate * u32::from(block_align);
    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..frames {
        let t = i as f64 / f64::from(sample_rate);
        let sample =
            ((std::f64::consts::TAU * 440.0 * t).sin() * f64::from(amplitude)).round() as i16;
        for _ in 0..channels {
            out.extend_from_slice(&sample.to_le_bytes());
        }
    }
    fs::write(path, out).expect("write wav");
}

fn make_audio_clip(id: ClipId, path: &Path, rel: Option<&str>, ram_mode: bool) -> Clip {
    let mut clip = Clip::new_audio(id, "Sample", path.to_string_lossy().replace('\\', "/"), 4.0);
    clip.loop_start_beat = 0.5;
    clip.loop_end_beat = 3.5;
    clip.is_looping = true;
    clip.start_marker_beat = 0.25;
    clip.end_marker_beat = 3.75;
    if let ClipPayload::Audio(ref mut audio) = clip.payload {
        audio.relative_path = rel.map(str::to_owned);
        audio.sample_rate = 44_100;
        audio.channels = 1;
        audio.warp_mode = WarpMode::ComplexPro;
        audio.is_warped = true;
        audio.gain_db = -3.5;
        audio.pitch_cents = 250;
        audio.ram_mode = ram_mode;
        audio.warp_markers = vec![
            WarpMarker {
                sample_time_sec: 0.0,
                beat_time: 0.0,
            },
            WarpMarker {
                sample_time_sec: 0.25,
                beat_time: 0.5,
            },
        ];
    }
    clip
}

fn audio_data(session: &Session, clip_id: ClipId) -> &livecraft_model::model::AudioClipData {
    for track in session.project().all_tracks() {
        for slot in &track.clip_slots {
            if let Some(ref clip) = slot.clip
                && clip.id == clip_id
                && let ClipPayload::Audio(ref audio) = clip.payload
            {
                return audio;
            }
        }
        for arr in &track.arrangement_clips {
            if arr.clip.id == clip_id
                && let ClipPayload::Audio(ref audio) = arr.clip.payload
            {
                return audio;
            }
        }
    }
    panic!("audio clip {clip_id} not found");
}

#[test]
fn set_clip_ram_mode_undo_redo_and_als_ram_round_trip() {
    let dir = test_dir("ram_mode_als");
    let als_in = dir.join("in.als");
    let als_on = dir.join("ram_on.als");
    let als_off = dir.join("ram_off.als");

    let no_ram_xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<Ableton MajorVersion=\"5\" MinorVersion=\"12.0_12117\">\n",
        "\t<LiveSet>\n",
        "\t\t<Tracks>\n",
        "\t\t\t<AudioTrack Id=\"1\">\n",
        "\t\t\t\t<Name><EffectiveName Value=\"Audio\" /></Name>\n",
        "\t\t\t\t<DeviceChain><MainSequencer><ClipSlotList>\n",
        "\t\t\t\t\t<ClipSlot Id=\"0\"><ClipSlot><Value>\n",
        "\t\t\t\t\t\t<AudioClip Id=\"10\">\n",
        "\t\t\t\t\t\t\t<Name Value=\"Loop\" />\n",
        "\t\t\t\t\t\t\t<SampleRef><FileRef><Path Value=\"D:/Audio/Loop.wav\" /></FileRef></SampleRef>\n",
        "\t\t\t\t\t\t</AudioClip>\n",
        "\t\t\t\t\t</Value></ClipSlot></ClipSlot>\n",
        "\t\t\t\t</ClipSlotList></MainSequencer></DeviceChain>\n",
        "\t\t\t</AudioTrack>\n",
        "\t\t</Tracks>\n",
        "\t</LiveSet>\n",
        "</Ableton>\n"
    );
    AlsDocument::from_xml(no_ram_xml)
        .expect("parse xml")
        .save_als(&als_in)
        .expect("save input als");

    let mut session = Session::open_als(&als_in).expect("open als");
    let clip_id = session.project().tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("audio clip")
        .id;
    assert!(!audio_data(&session, clip_id).ram_mode);

    // Toggle on via Session::set_clip_ram_mode, undo, redo.
    session
        .set_clip_ram_mode(clip_id, true)
        .expect("enable ram_mode");
    assert!(audio_data(&session, clip_id).ram_mode);
    assert!(session.undo());
    assert!(!audio_data(&session, clip_id).ram_mode);
    assert!(session.redo().expect("redo ram_mode"));
    assert!(audio_data(&session, clip_id).ram_mode);

    // Save to .als and verify <Ram Value="true" /> round-trips cleanly.
    session.save(&als_on).expect("save ram_on.als");
    let xml_on = AlsParser::decompress_to_xml(&als_on).expect("decompress ram_on");
    assert!(xml_on.contains("<Ram Value=\"true\""), "{xml_on}");

    let mut reopened = Session::open_als(&als_on).expect("reopen ram_on.als");
    let reopened_id = reopened.project().tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .id;
    assert!(audio_data(&reopened, reopened_id).ram_mode);

    // Toggle back off via SetClipRamMode command and verify <Ram Value="false" /> round-trips.
    reopened
        .execute(SetClipRamMode::new(reopened_id, false))
        .expect("disable ram_mode");
    assert!(!audio_data(&reopened, reopened_id).ram_mode);
    reopened.save(&als_off).expect("save ram_off.als");
    let xml_off = AlsParser::decompress_to_xml(&als_off).expect("decompress ram_off");
    assert!(xml_off.contains("<Ram Value=\"false\""), "{xml_off}");
    let reopened_off = Session::open_als(&als_off).expect("reopen ram_off.als");
    let off_id = reopened_off.project().tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .id;
    assert!(!audio_data(&reopened_off, off_id).ram_mode);

    // Arrangement clips and MIDI/missing clip refusals.
    let mut proj = Project::new_default("RamValidation");
    let midi_clip = Clip::new_midi(ClipId(500), "Midi", 4.0);
    proj.tracks[0].clip_slots[0].clip = Some(midi_clip);
    let arr_audio = make_audio_clip(ClipId(501), Path::new("D:/Audio/Arr.wav"), None, false);
    proj.tracks[2]
        .arrangement_clips
        .push(ArrangementClip::new(ClipId(501), arr_audio, 0.0, 4.0));
    let mut val_session = Session::new(proj, None);

    assert_eq!(
        val_session.set_clip_ram_mode(ClipId(500), true),
        Err(CommandError::NotAudioClip(ClipId(500)))
    );
    assert_eq!(
        val_session.set_clip_ram_mode(ClipId(9999), true),
        Err(CommandError::NoSuchClip(ClipId(9999)))
    );
    val_session
        .set_clip_ram_mode(ClipId(501), true)
        .expect("ram_mode on arrangement clip");
    assert!(audio_data(&val_session, ClipId(501)).ram_mode);
    assert!(val_session.undo());
    assert!(!audio_data(&val_session, ClipId(501)).ram_mode);
}

#[test]
fn replace_clip_sample_same_length_retains_markers_vs_diff_length_clears_and_undoes() {
    let dir = test_dir("replace_sample");
    let proj_root = dir.join("Beat Project");
    create_project_layout(&proj_root).expect("create project layout");
    let set_path = proj_root.join("Beat.als");

    let old_wav = proj_root.join("Samples/Imported/old.wav");
    let same_len_wav = proj_root.join("Samples/Imported/same_len.wav");
    let diff_len_wav = dir.join("outside_diff_len.wav");

    write_pcm_wav(&old_wav, 44_100, 1, 4_096, 12_000);
    write_pcm_wav(&same_len_wav, 48_000, 2, 4_096, 16_000);
    write_pcm_wav(&diff_len_wav, 44_100, 2, 8_192, 14_000);

    let mut project = Project::new_default("ReplaceTest");
    let clip_id = ClipId(901);
    let clip = make_audio_clip(clip_id, &old_wav, Some("Samples/Imported/old.wav"), true);
    project.tracks[2].clip_slots[0].clip = Some(clip);
    AlsDocument::from_project(&project)
        .expect("doc")
        .save_als(&set_path)
        .expect("save set");

    let mut session = Session::open_als(&set_path).expect("open set");
    assert!(session.missing_files().is_empty());
    let expected_gain_db = audio_data(&session, clip_id).gain_db;
    assert!((expected_gain_db - (-3.5)).abs() < 1e-4);

    // 1. Same-length replacement inside project_root retains warp markers, sets relative_path,
    //    preserves gain/pitch/warp/ram/loop/markers, and builds .lcanalysis.
    session
        .replace_clip_sample(clip_id, &same_len_wav)
        .expect("replace with same length");
    {
        let clip = &session.project().tracks[2].clip_slots[0]
            .clip
            .as_ref()
            .expect("clip");
        assert_eq!(clip.loop_start_beat, 0.5);
        assert_eq!(clip.loop_end_beat, 3.5);
        assert!(clip.is_looping);
        assert_eq!(clip.start_marker_beat, 0.25);
        assert_eq!(clip.end_marker_beat, 3.75);
        let ClipPayload::Audio(ref a) = clip.payload else {
            panic!("expected audio");
        };
        assert_eq!(a.sample_rate, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(
            a.relative_path.as_deref(),
            Some("Samples/Imported/same_len.wav")
        );
        assert_eq!(a.warp_markers.len(), 2, "same length retains warp markers");
        assert_eq!(a.gain_db, expected_gain_db);
        assert_eq!(a.pitch_cents, 250);
        assert_eq!(a.warp_mode, WarpMode::ComplexPro);
        assert!(a.is_warped);
        assert!(a.ram_mode);
    }
    let same_cache = analysis_cache_path(&same_len_wav, &AnalysisCacheLocation::BesideSample);
    assert!(
        same_cache.is_file(),
        "replace_clip_sample builds .lcanalysis when create_analysis_files=true"
    );

    // Undo restores old path, relative_path, rate, channels, and markers.
    assert!(session.undo());
    {
        let a = audio_data(&session, clip_id);
        assert_eq!(a.sample_rate, 44_100);
        assert_eq!(a.channels, 1);
        assert_eq!(a.relative_path.as_deref(), Some("Samples/Imported/old.wav"));
        assert_eq!(a.warp_markers.len(), 2);
    }
    assert!(session.redo().expect("redo same-length replace"));
    assert_eq!(audio_data(&session, clip_id).sample_rate, 48_000);

    // 2. Different-length replacement outside project_root clears warp markers and relative_path,
    //    preserves settings, and exact undo restores warp markers.
    session
        .replace_clip_sample(clip_id, &diff_len_wav)
        .expect("replace with different length");
    {
        let a = audio_data(&session, clip_id);
        assert_eq!(a.sample_rate, 44_100);
        assert_eq!(a.channels, 2);
        assert_eq!(a.relative_path, None);
        assert!(
            a.warp_markers.is_empty(),
            "different length clears warp markers"
        );
        assert_eq!(a.gain_db, expected_gain_db);
        assert_eq!(a.pitch_cents, 250);
        assert_eq!(a.warp_mode, WarpMode::ComplexPro);
        assert!(a.is_warped);
        assert!(a.ram_mode);
    }

    assert!(session.undo());
    {
        let a = audio_data(&session, clip_id);
        assert_eq!(a.sample_rate, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(
            a.relative_path.as_deref(),
            Some("Samples/Imported/same_len.wav")
        );
        assert_eq!(
            a.warp_markers.len(),
            2,
            "undo restores cleared warp markers"
        );
    }
    assert!(session.redo().expect("redo diff-length replace"));
    assert!(audio_data(&session, clip_id).warp_markers.is_empty());

    // 3. Replacing a missing sample clears missing_files and clears warp markers (old_frames is None).
    let missing_id = ClipId(902);
    let mut missing_clip = make_audio_clip(
        missing_id,
        Path::new("C:/nonexistent_drive_xyz/missing_snare.wav"),
        None,
        false,
    );
    missing_clip.id = missing_id;
    let mut proj2 = Project::new_default("MissingReplace");
    proj2.tracks[2].clip_slots[0].clip = Some(missing_clip);
    let mut s2 = Session::new(proj2, None);
    assert_eq!(s2.missing_files().len(), 1);

    let err = s2
        .replace_clip_sample(missing_id, &dir.join("does_not_exist.wav"))
        .err();
    assert!(matches!(err, Some(SessionFormatError::AudioDecode(_))));
    assert_eq!(s2.missing_files().len(), 1);

    s2.replace_clip_sample(missing_id, &diff_len_wav)
        .expect("replace missing sample");
    assert!(s2.missing_files().is_empty());
    assert!(audio_data(&s2, missing_id).warp_markers.is_empty());

    // Direct ReplaceClipSample command validation.
    let zero_rate = s2.execute(ReplaceClipSample::new(
        missing_id, "x.wav", None, 0, 2, true,
    ));
    assert!(matches!(
        zero_rate,
        Err(CommandError::NotPositive {
            what: "sample rate",
            ..
        })
    ));
}

#[test]
fn sample_analysis_cache_modes_and_asd_preservation() {
    let dir = test_dir("sample_analysis");
    let wav_path = dir.join("loop.wav");
    write_pcm_wav(&wav_path, 44_100, 2, 2_048, 20_000);

    // Live .asd files beside the sample must never be modified or deleted.
    let asd_a = dir.join("loop.wav.asd");
    let asd_b = dir.join("loop.asd");
    fs::write(&asd_a, b"ASD-KEEP-A").expect("write asd_a");
    fs::write(&asd_b, b"ASD-KEEP-B").expect("write asd_b");

    let clip_id = ClipId(910);
    let clip = make_audio_clip(clip_id, &wav_path, None, false);
    let midi_clip = Clip::new_midi(ClipId(911), "Midi", 4.0);
    let missing_clip = make_audio_clip(ClipId(912), &dir.join("no_such_sample.wav"), None, false);

    let mut project = Project::new_default("AnalysisTest");
    project.tracks[2].clip_slots[0].clip = Some(clip.clone());
    project.tracks[0].clip_slots[0].clip = Some(midi_clip.clone());
    project.tracks[2].clip_slots[1].clip = Some(missing_clip.clone());
    let mut session = Session::new(project, None);

    // MIDI & missing clip return None.
    assert!(session.sample_analysis(&midi_clip).is_none());
    assert!(session.sample_analysis(&missing_clip).is_none());

    // 1. Default: BesideSample with create_analysis_files = true.
    let analysis_beside = session
        .sample_analysis(&clip)
        .expect("analysis beside sample");
    assert_eq!(analysis_beside.sample_rate, 44_100);
    assert_eq!(analysis_beside.channels, 2);
    assert_eq!(analysis_beside.frames, 2_048);
    let beside_cache = analysis_cache_path(&wav_path, &AnalysisCacheLocation::BesideSample);
    assert!(beside_cache.is_file());

    // 2. Custom cache folder writes into the folder rather than beside a new sample.
    let custom_folder = dir.join("central_cache");
    let wav2_path = dir.join("sub").join("snare.wav");
    write_pcm_wav(&wav2_path, 48_000, 1, 1_024, 15_000);
    let clip2 = make_audio_clip(ClipId(913), &wav2_path, None, false);

    session
        .set_library_preferences(LibraryPreferences {
            create_analysis_files: true,
            analysis_cache_folder: Some(custom_folder.clone()),
            ..LibraryPreferences::default()
        })
        .expect("set custom folder");

    let analysis_folder = session
        .sample_analysis(&clip2)
        .expect("analysis in custom folder");
    assert_eq!(analysis_folder.sample_rate, 48_000);
    assert_eq!(analysis_folder.channels, 1);
    let folder_cache = analysis_cache_path(
        &wav2_path,
        &AnalysisCacheLocation::Folder(custom_folder.clone()),
    );
    let wav2_beside = analysis_cache_path(&wav2_path, &AnalysisCacheLocation::BesideSample);
    assert!(folder_cache.is_file());
    assert!(!wav2_beside.exists());

    // 3. Switching create_analysis_files = false does NOT delete existing caches and writes no new file.
    let wav3_path = dir.join("nocache.wav");
    write_pcm_wav(&wav3_path, 44_100, 1, 512, 10_000);
    let clip3 = make_audio_clip(ClipId(914), &wav3_path, None, false);

    session
        .set_library_preferences(LibraryPreferences {
            create_analysis_files: false,
            analysis_cache_folder: Some(custom_folder),
            ..LibraryPreferences::default()
        })
        .expect("disable create_analysis_files");

    assert!(beside_cache.is_file(), "existing beside cache preserved");
    assert!(folder_cache.is_file(), "existing folder cache preserved");

    let analysis_no_write = session
        .sample_analysis(&clip3)
        .expect("in-memory analysis when create_analysis_files=false");
    assert_eq!(analysis_no_write.frames, 512);
    let wav3_beside = analysis_cache_path(&wav3_path, &AnalysisCacheLocation::BesideSample);
    let wav3_folder = analysis_cache_path(
        &wav3_path,
        &session.preferences().library.analysis_cache_location(),
    );
    assert!(!wav3_beside.exists());
    assert!(!wav3_folder.exists());

    // .asd files remain untouched byte-for-byte.
    assert_eq!(fs::read(&asd_a).expect("read asd_a"), b"ASD-KEEP-A");
    assert_eq!(fs::read(&asd_b).expect("read asd_b"), b"ASD-KEEP-B");
}

#[test]
fn refresh_clip_sample_recomputes_analysis_metadata_and_reloads_engine() {
    let dir = test_dir("refresh_sample");
    let wav_path = dir.join("editable.wav");
    // Initial sample: 44.1 kHz mono, very quiet (~0.01 amplitude), 1024 frames.
    write_pcm_wav(&wav_path, 44_100, 1, 1_024, 300);

    let clip_id = ClipId(920);
    let clip = make_audio_clip(clip_id, &wav_path, None, false);
    let mut project = Project::new_default("RefreshTest");
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    project.tracks[2].clip_slots[0].clip = Some(clip.clone());

    let mut session = Session::with_driver_factory(project, null_factory(), None);
    let first_analysis = session
        .sample_analysis(&clip)
        .expect("initial sample analysis");
    let first_peak = first_analysis.peak_overview[0].max;
    assert!(
        first_peak < 0.05,
        "initial peak expected quiet, got {first_peak}"
    );

    // 1. External edit with SAME byte length (same rate/channels/frames) in the same second, louder!
    write_pcm_wav(&wav_path, 44_100, 1, 1_024, 24_000);
    session
        .refresh_clip_sample(clip_id)
        .expect("refresh same-size modified sample");
    let refreshed_same_len = session
        .sample_analysis(
            session.project().tracks[2].clip_slots[0]
                .clip
                .as_ref()
                .expect("clip"),
        )
        .expect("refreshed analysis");
    assert!(
        refreshed_same_len.peak_overview[0].max > 0.5,
        "refresh must recompute .lcanalysis even on same-size/same-second overwrite, got {}",
        refreshed_same_len.peak_overview[0].max
    );

    // 2. External edit changing sample_rate & channels (48 kHz stereo, 1 second long).
    write_pcm_wav(&wav_path, 48_000, 2, 48_000, 24_000);
    session
        .refresh_clip_sample(clip_id)
        .expect("refresh rate/channel changed sample");
    let updated_audio = audio_data(&session, clip_id);
    assert_eq!(updated_audio.sample_rate, 48_000);
    assert_eq!(updated_audio.channels, 2);

    // Allow NullDriver ~2 callbacks to apply flush_graph + updated graph, then trigger and play.
    std::thread::sleep(Duration::from_millis(60));
    session.trigger_clip(TrackId(3), 0);
    session.play();
    let deadline = Instant::now() + Duration::from_millis(800);
    let mut heard_audio = false;
    while Instant::now() < deadline {
        if session.status().master_meter.peak_l > 0.05 {
            heard_audio = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(heard_audio, "reloaded sample must play in engine");

    // 3. With create_analysis_files = false, refresh updates metadata without writing/deleting caches.
    let no_cache_wav = dir.join("no_cache_refresh.wav");
    write_pcm_wav(&no_cache_wav, 44_100, 1, 1_024, 10_000);
    let asd_path = dir.join("no_cache_refresh.wav.asd");
    fs::write(&asd_path, b"ASD-REFRESH").expect("write asd");
    let no_cache_id = ClipId(921);
    let no_cache_clip = make_audio_clip(no_cache_id, &no_cache_wav, None, false);
    let mut proj_nc = Project::new_default("RefreshNoCache");
    proj_nc.tracks[2].clip_slots[0].clip = Some(no_cache_clip);
    let mut s_nc = Session::new(proj_nc, None);
    s_nc.set_library_preferences(LibraryPreferences {
        create_analysis_files: false,
        ..LibraryPreferences::default()
    })
    .expect("disable analysis files");
    write_pcm_wav(&no_cache_wav, 48_000, 2, 2_048, 12_000);
    s_nc.refresh_clip_sample(no_cache_id)
        .expect("refresh with create_analysis_files=false");
    let nc_audio = audio_data(&s_nc, no_cache_id);
    assert_eq!(nc_audio.sample_rate, 48_000);
    assert_eq!(nc_audio.channels, 2);
    assert!(!analysis_cache_path(&no_cache_wav, &AnalysisCacheLocation::BesideSample).exists());
    assert_eq!(fs::read(&asd_path).expect("read asd"), b"ASD-REFRESH");

    // 4. Missing clip / MIDI clip refusals.
    assert!(matches!(
        session.refresh_clip_sample(ClipId(9999)),
        Err(SessionFormatError::Command(CommandError::NoSuchClip(_)))
    ));
}

#[test]
fn engine_status_surfaces_disk_load_and_underruns_and_poll_background_works() {
    let dir = test_dir("disk_metrics_and_bg");
    let wav_path = dir.join("long_3s.wav");
    write_pcm_wav(&wav_path, 44_100, 2, 44_100 * 3, 12_000);

    // 1. Headless session (no engine) defaults disk_load=0.0, disk_underruns=0.
    let mut headless = Session::new(Project::new_default("Headless"), None);
    let headless_status = headless.status();
    assert_eq!(headless_status.disk_load, 0.0);
    assert_eq!(headless_status.disk_underruns, 0);

    // 2. Deterministic Manual streamer reveals real disk_load and disk_underruns in EngineStatus.
    let mut project = Project::new_default("StarveSession");
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    let clip_id = ClipId(930);
    let mut clip = make_audio_clip(clip_id, &wav_path, None, false);
    clip.length_beats = 6.0;
    clip.loop_end_beat = 6.0;
    if let ClipPayload::Audio(ref mut a) = clip.payload {
        a.is_warped = false;
    }
    project.tracks[2].clip_slots[0].clip = Some(clip);

    let (playback, mut engine) =
        Playback::start_render_in_context(&project, 44_100.0, ProjectContext::default())
            .expect("start render playback");
    let mut session = Session::new(project, Some(playback));
    session.trigger_clip(TrackId(3), 0);
    session.play();

    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    let blocks = (44_100 * 3 / 2) / MAX_BLOCK;
    for _ in 0..blocks {
        engine.process_offline(&mut out, 2);
    }

    let starved_status = session.status();
    assert!(
        starved_status.disk_underruns > 0,
        "EngineStatus must surface disk_underruns > 0"
    );
    assert!(
        starved_status.disk_load > 0.0,
        "EngineStatus must surface disk_load > 0.0 (got {})",
        starved_status.disk_load
    );

    // 3. poll_background ingests completed RAM decodes and submits recompiled graph.
    let ram_wav = dir.join("ram_48k.wav");
    write_pcm_wav(&ram_wav, 48_000, 2, 48_000, 18_000);
    let mut ram_proj = Project::new_default("BgPoll");
    ram_proj.transport.global_quantization = livecraft_model::model::Quantization::None;
    ram_proj.tracks[2].device_chain.clear();
    ram_proj.main_track.device_chain.clear();
    let ram_clip = make_audio_clip(ClipId(931), &ram_wav, None, true);
    ram_proj.tracks[2].clip_slots[0].clip = Some(ram_clip);

    let (mut ram_playback, mut ram_engine) = Playback::start_render_in_context(
        &Project::new_default("Empty"),
        44_100.0,
        ProjectContext::default(),
    )
    .expect("start ram playback");
    let queued = ram_playback.enqueue_ram_decode(&ram_wav.to_string_lossy(), None);
    assert!(queued.is_some());

    let mut ram_session = Session::new(ram_proj, Some(ram_playback));
    // Wait until background worker completes, poll_background submits the recompiled graph,
    // and offline rendering outputs non-zero audio from the RAM-decoded clip.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut heard_ram_audio = false;
    while Instant::now() < deadline {
        ram_session.poll_background();
        let status = ram_session.status();
        if status.master_meter.peak_l > 0.05 {
            heard_ram_audio = true;
            break;
        }
        if status.active_slots.get(2).copied().flatten() != Some(0) {
            ram_session.trigger_clip(TrackId(3), 0);
        }
        ram_engine.process_offline(&mut out, 2);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        heard_ram_audio,
        "poll_background must ingest RAM decode, submit recompiled graph, and produce audio"
    );
}
