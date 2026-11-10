//! Headless egui integration tests covering M11 UI features:
//! - Audio Clip View waveform overview (`.lcanalysis` pyramid), `RAM` toggle (`SetClipRamMode`) + undo
//! - `Replace Sample...` dialog preserving settings while obeying warp marker length rules
//! - `Reload` external edit refresh & Library analysis cache preferences UI
//! - `"Disk"` overload indicator helper and `render_transport_bar` rendering
//! - Drag-and-drop audio files (`.flac` / `.ogg` / `.wav`) replacing or inserting audio clips

use eframe::egui::{self, Key, Modifiers, PointerButton, Pos2, Rect, Shape, Vec2};
use livecraft_model::clip::{WarpMarker, WarpMode};
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::{Clip, ClipPayload, Project, TrackType};
use livecraft_session::Session;
use livecraft_ui::LiveCraftApp;
use livecraft_ui::file_manager::FileManagerTab;
use livecraft_ui::pianoroll::{PyramidLevel, choose_pyramid_level};
use livecraft_ui::style::LivePalette;
use livecraft_ui::transport::{ViewMode, is_disk_overloaded, render_transport_bar};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("m11_ui")
            .join(format!("{label}_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp test dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct TestDroppedFile(PathBuf);

impl egui::DroppedFile for TestDroppedFile {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn fixtures_audio_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("audio")
}

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
        let sample = if i % 2 == 0 { amplitude } else { -amplitude };
        for _ in 0..channels {
            out.extend_from_slice(&sample.to_le_bytes());
        }
    }
    fs::write(path, out).expect("write wav");
}

fn raw_input_1280x800() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        ..Default::default()
    }
}

fn run_frame(
    ctx: &egui::Context,
    app: &mut LiveCraftApp,
    input: egui::RawInput,
) -> egui::FullOutput {
    let mut out = ctx.run_ui(input, |ui| app.render_ui(ui));
    out.textures_delta.clear();
    out
}

fn step_frames(ctx: &egui::Context, app: &mut LiveCraftApp, count: usize) -> egui::FullOutput {
    let mut last = run_frame(ctx, app, raw_input_1280x800());
    for _ in 1..count {
        last = run_frame(ctx, app, raw_input_1280x800());
    }
    last
}

fn collect_shape_texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(text_shape) => {
            let text = text_shape.galley.text().to_owned();
            let rect = text_shape.visual_bounding_rect();
            out.push((text, rect));
        }
        Shape::Vec(vec) => {
            for child in vec {
                collect_shape_texts(child, out);
            }
        }
        _ => {}
    }
}

fn all_texts(output: &egui::FullOutput) -> Vec<(String, Rect)> {
    let mut out = Vec::new();
    for clipped in &output.shapes {
        collect_shape_texts(&clipped.shape, &mut out);
    }
    out
}

fn has_text(output: &egui::FullOutput, needle: &str) -> bool {
    all_texts(output)
        .iter()
        .any(|(text, _)| text.contains(needle))
}

fn find_exact_text_center(output: &egui::FullOutput, exact: &str) -> Option<Pos2> {
    all_texts(output)
        .into_iter()
        .find(|(text, _)| text == exact)
        .map(|(_, rect)| rect.center())
}

fn click_pos(ctx: &egui::Context, app: &mut LiveCraftApp, pos: Pos2) -> egui::FullOutput {
    let mut press = raw_input_1280x800();
    press.events = vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    ];
    let _ = run_frame(ctx, app, press);

    let mut release = raw_input_1280x800();
    release.events = vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ];
    let _ = run_frame(ctx, app, release);
    step_frames(ctx, app, 2)
}

fn click_exact(
    ctx: &egui::Context,
    app: &mut LiveCraftApp,
    output: &egui::FullOutput,
    exact: &str,
) -> egui::FullOutput {
    let pos = find_exact_text_center(output, exact)
        .unwrap_or_else(|| panic!("expected button/label '{exact}' in UI"));
    click_pos(ctx, app, pos)
}

fn count_vertical_segments_in_bottom_panel(shape: &Shape, min_y: f32) -> usize {
    match shape {
        Shape::LineSegment { points, .. } => {
            if points[0].y >= min_y
                && points[1].y >= min_y
                && (points[0].x - points[1].x).abs() < 0.01
                && (points[0].y - points[1].y).abs() > 1.0
            {
                1
            } else {
                0
            }
        }
        Shape::Vec(vec) => vec
            .iter()
            .map(|child| count_vertical_segments_in_bottom_panel(child, min_y))
            .sum(),
        _ => 0,
    }
}

fn total_waveform_vertical_bars(output: &egui::FullOutput) -> usize {
    output
        .shapes
        .iter()
        .map(|clipped| count_vertical_segments_in_bottom_panel(&clipped.shape, 640.0))
        .sum()
}

fn make_configured_audio_clip(id: ClipId, name: &str, path: &Path) -> Clip {
    let mut clip = Clip::new_audio(id, name, path.to_string_lossy().replace('\\', "/"), 4.0);
    if let ClipPayload::Audio(ref mut audio) = clip.payload {
        audio.sample_rate = 44_100;
        audio.channels = 2;
        audio.warp_mode = WarpMode::ComplexPro;
        audio.is_warped = true;
        audio.gain_db = -3.5;
        audio.pitch_cents = 250;
        audio.ram_mode = false;
        audio.warp_markers = vec![
            WarpMarker {
                sample_time_sec: 0.0,
                beat_time: 0.0,
            },
            WarpMarker {
                sample_time_sec: 0.05,
                beat_time: 0.5,
            },
        ];
    }
    clip
}

#[test]
fn audio_clip_view_renders_waveform_overview_toggles_ram_mode_and_undoes() {
    let dir = TempDirGuard::new("audio_clip_view_ram");
    let wav_path = dir.path().join("bass_loop.wav");
    write_pcm_wav(&wav_path, 44_100, 2, 4_096, 20_000);

    let mut project = Project::new_default("AudioViewTest");
    let clip_id = ClipId(701);
    project.tracks[2].clip_slots[0].clip =
        Some(make_configured_audio_clip(clip_id, "BassLoop", &wav_path));

    // Also place a missing/offline audio clip in slot 1.
    let missing_id = ClipId(702);
    project.tracks[2].clip_slots[1].clip = Some(Clip::new_audio(
        missing_id,
        "MissingClip",
        dir.path().join("does_not_exist.wav").to_string_lossy(),
        4.0,
    ));

    let mut app = LiveCraftApp::from_session(Session::new(project, None), None);
    app.selected_track = TrackId(3);
    app.selected_clip = Some((TrackId(3), 0));
    app.bottom_tab = 1;

    let ctx = egui::Context::default();
    let out1 = step_frames(&ctx, &mut app, 2);

    // Header controls & metadata rendered.
    assert!(has_text(&out1, "BassLoop"));
    assert!(has_text(&out1, "Gain: -3.5 dB"));
    assert!(has_text(&out1, "Pitch: +2.50 st (+250 ct)"));
    assert!(has_text(&out1, "RAM"));
    assert!(has_text(&out1, "Replace Sample..."));
    assert!(has_text(&out1, "Reload"));
    assert!(has_text(&out1, "Save .alc"));
    assert!(has_text(&out1, "Pyramid: 256 (4096 frames)"));

    // .lcanalysis cache was built and waveform min/max vertical bars + playhead painted.
    let mut cache_os = wav_path.as_os_str().to_os_string();
    cache_os.push(".lcanalysis");
    let cache_path = PathBuf::from(cache_os);
    assert!(cache_path.is_file());
    let bars = total_waveform_vertical_bars(&out1);
    assert!(
        bars > 100,
        "expected waveform overview vertical bars + playhead, got {bars}"
    );
    assert!(!has_text(&out1, "Sample Offline"));

    // Verify LiveCraftApp caches analysis across frames rather than regenerating .lcanalysis every frame.
    fs::remove_file(&cache_path).expect("remove sidecar to verify in-memory cache");
    let out_cached = step_frames(&ctx, &mut app, 3);
    assert!(
        !cache_path.exists(),
        "cached analysis must not re-read/re-create .lcanalysis on unchanged frames"
    );
    assert!(total_waveform_vertical_bars(&out_cached) > 100);

    // Toggle RAM mode via UI button (SetClipRamMode).
    let _out_ram = click_exact(&ctx, &mut app, &out_cached, "RAM");
    let audio_after_toggle = match &app.session.project().tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    {
        ClipPayload::Audio(a) => a,
        ClipPayload::Midi(_) => panic!("expected audio clip"),
    };
    assert!(audio_after_toggle.ram_mode);

    // Ctrl+Z undoes SetClipRamMode back to false.
    let mut undo_in = raw_input_1280x800();
    undo_in.events = vec![
        egui::Event::ModifiersChanged(Modifiers::COMMAND),
        egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        },
    ];
    let _ = run_frame(&ctx, &mut app, undo_in);
    let audio_after_undo = match &app.session.project().tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    {
        ClipPayload::Audio(a) => a,
        ClipPayload::Midi(_) => panic!("expected audio clip"),
    };
    assert!(!audio_after_undo.ram_mode);

    // Offline / missing clip displays clean placeholder.
    app.selected_clip = Some((TrackId(3), 1));
    let out_offline = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out_offline, "Sample Offline"));
}

#[test]
fn replace_sample_dialog_preserves_pitch_gain_and_obeys_warp_marker_length_rule() {
    let dir = TempDirGuard::new("replace_sample_dialog");
    let old_wav = dir.path().join("old.wav");
    let same_len_wav = dir.path().join("same_len.wav");
    let diff_len_wav = dir.path().join("diff_len.wav");

    write_pcm_wav(&old_wav, 44_100, 1, 4_096, 12_000);
    write_pcm_wav(&same_len_wav, 48_000, 2, 4_096, 16_000);
    write_pcm_wav(&diff_len_wav, 44_100, 2, 8_192, 14_000);

    let mut project = Project::new_default("ReplaceDialogUI");
    let clip_id = ClipId(710);
    let mut clip = make_configured_audio_clip(clip_id, "Guitar", &old_wav);
    if let ClipPayload::Audio(ref mut a) = clip.payload {
        a.channels = 1;
        a.gain_db = -4.5;
        a.pitch_cents = -300;
        a.ram_mode = true;
    }
    project.tracks[2].clip_slots[0].clip = Some(clip);

    let mut app = LiveCraftApp::from_session(Session::new(project, None), None);
    app.selected_track = TrackId(3);
    app.selected_clip = Some((TrackId(3), 0));
    app.bottom_tab = 1;

    let ctx = egui::Context::default();
    let out1 = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out1, "Pitch: -3 st"));

    // 1. Open Replace Sample... dialog via Clip View button and submit invalid path -> shows error.
    let _out_modal = click_exact(&ctx, &mut app, &out1, "Replace Sample...");
    assert!(app.file_manager.prompt.is_some());
    app.file_manager
        .prompt
        .as_mut()
        .expect("replace prompt")
        .path = dir
        .path()
        .join("missing_bad.wav")
        .to_string_lossy()
        .into_owned();
    let out_err_in = step_frames(&ctx, &mut app, 2);
    let out_err = click_exact(&ctx, &mut app, &out_err_in, "Replace");
    assert!(app.file_manager.prompt.is_some());
    assert!(has_text(&out_err, "Replace Sample failed:"));

    // 2. Replace with same-length sample -> preserves gain, pitch, warp mode, RAM, and warp markers.
    app.file_manager
        .prompt
        .as_mut()
        .expect("replace prompt")
        .path = same_len_wav.to_string_lossy().into_owned();
    let out_same_in = step_frames(&ctx, &mut app, 2);
    let out_same = click_exact(&ctx, &mut app, &out_same_in, "Replace");
    assert!(app.file_manager.prompt.is_none());

    {
        let ClipPayload::Audio(ref a) = app.session.project().tracks[2].clip_slots[0]
            .clip
            .as_ref()
            .expect("clip")
            .payload
        else {
            panic!("expected audio");
        };
        assert_eq!(a.sample_rate, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(a.gain_db, -4.5);
        assert_eq!(a.pitch_cents, -300);
        assert_eq!(a.warp_mode, WarpMode::ComplexPro);
        assert!(a.is_warped);
        assert!(a.ram_mode);
        assert_eq!(a.warp_markers.len(), 2, "same length retains warp markers");
    }
    assert!(has_text(&out_same, "48.0 kHz | 2 ch"));

    // 3. Replace with different-length sample -> preserves gain/pitch, clears warp markers.
    let _out_modal2 = click_exact(&ctx, &mut app, &out_same, "Replace Sample...");
    app.file_manager
        .prompt
        .as_mut()
        .expect("replace prompt 2")
        .path = diff_len_wav.to_string_lossy().into_owned();
    let out_diff_in = step_frames(&ctx, &mut app, 2);
    let _out_diff = click_exact(&ctx, &mut app, &out_diff_in, "Replace");
    assert!(app.file_manager.prompt.is_none());

    let ClipPayload::Audio(ref a2) = app.session.project().tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    else {
        panic!("expected audio");
    };
    assert_eq!(a2.sample_rate, 44_100);
    assert_eq!(a2.channels, 2);
    assert_eq!(a2.gain_db, -4.5);
    assert_eq!(a2.pitch_cents, -300);
    assert!(a2.ram_mode);
    assert!(
        a2.warp_markers.is_empty(),
        "different frame length must clear warp markers"
    );
}

#[test]
fn audio_clip_view_reload_and_analysis_preferences_ui_work() {
    let dir = TempDirGuard::new("reload_and_prefs");
    let wav_path = dir.path().join("editable.wav");
    write_pcm_wav(&wav_path, 44_100, 1, 1_024, 400);

    let mut project = Project::new_default("ReloadPrefs");
    let clip_id = ClipId(720);
    let mut clip = make_configured_audio_clip(clip_id, "Editable", &wav_path);
    if let ClipPayload::Audio(ref mut a) = clip.payload {
        a.channels = 1;
    }
    project.tracks[2].clip_slots[0].clip = Some(clip);

    let mut app = LiveCraftApp::from_session(Session::new(project, None), None);
    app.selected_track = TrackId(3);
    app.selected_clip = Some((TrackId(3), 0));
    app.bottom_tab = 1;

    let ctx = egui::Context::default();
    let out1 = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out1, "44.1 kHz | 1 ch"));

    // Modify audio file on disk (new rate & channels) and click Reload in Audio Clip View.
    write_pcm_wav(&wav_path, 48_000, 2, 2_048, 22_000);
    let out_reload = click_exact(&ctx, &mut app, &out1, "Reload");
    assert!(has_text(&out_reload, "48.0 kHz | 2 ch"));
    assert!(has_text(&out_reload, "2048 frames"));

    // Open Manage Project -> External Files -> Library & Export Preferences, toggle analysis settings.
    let cache_dir = dir.path().join("CentralCache");
    app.file_manager
        .open_manage_files(FileManagerTab::ExternalFiles);
    let out_fm1 = step_frames(&ctx, &mut app, 2);
    let _out_fm2_anim = click_exact(&ctx, &mut app, &out_fm1, "Library & Export Preferences");
    let out_fm2 = step_frames(&ctx, &mut app, 10);
    assert!(has_text(&out_fm2, "Create Analysis Files (.lcanalysis)"));
    assert!(has_text(&out_fm2, "Analysis Cache Folder:"));

    // Uncheck Create Analysis Files (.lcanalysis) and fill Analysis Cache Folder input.
    let out_fm3 = click_exact(
        &ctx,
        &mut app,
        &out_fm2,
        "Create Analysis Files (.lcanalysis)",
    );
    // Focus the Analysis Cache Folder field by clicking its hint text, then type path on next frame.
    let _out_focused = click_exact(&ctx, &mut app, &out_fm3, "Blank = beside each sample");
    let mut type_in = raw_input_1280x800();
    type_in.events = vec![egui::Event::Text(cache_dir.to_string_lossy().into_owned())];
    let out_typed = run_frame(&ctx, &mut app, type_in);
    let _out_applied = click_exact(&ctx, &mut app, &out_typed, "Apply Library Preferences");

    let prefs = &app.session.preferences().library;
    assert!(!prefs.create_analysis_files);
    assert_eq!(
        prefs.analysis_cache_folder.as_deref(),
        Some(cache_dir.as_path())
    );
}

#[test]
fn disk_overload_indicator_helper_and_rendering() {
    // Pure helper threshold checks (manual 38.3 p. 988).
    assert!(!is_disk_overloaded(0.0, 0));
    assert!(!is_disk_overloaded(0.99, 0));
    assert!(!is_disk_overloaded(f32::NAN, 0));
    assert!(is_disk_overloaded(1.0, 0));
    assert!(is_disk_overloaded(1.4, 0));
    assert!(is_disk_overloaded(0.0, 1));
    assert!(is_disk_overloaded(0.25, 3));

    // Pyramid level helper sanity.
    assert_eq!(choose_pyramid_level(44_100, 800.0), PyramidLevel::Block256);
    assert_eq!(
        choose_pyramid_level(6_000_000, 800.0),
        PyramidLevel::Block4096
    );
    assert_eq!(
        choose_pyramid_level(100_000_000, 800.0),
        PyramidLevel::Block65536
    );

    let ctx = egui::Context::default();
    let project = Project::new_default("DiskMeterUI");
    let palette = LivePalette::dark();

    let render_bar_with = |disk_load: f32, disk_underruns: u64| -> egui::FullOutput {
        let mut out = ctx.run_ui(raw_input_1280x800(), |ui| {
            egui::Panel::top("test_transport")
                .exact_size(34.0)
                .show(ui, |ui| {
                    let _ = render_transport_bar(
                        ui,
                        &project,
                        ViewMode::Session,
                        &palette,
                        3.0,
                        disk_load,
                        disk_underruns,
                        true,
                    );
                });
        });
        out.textures_delta.clear();
        out
    };

    // Normal disk state must not show "Disk" overload pill.
    let out_idle = render_bar_with(0.2, 0);
    assert!(!has_text(&out_idle, "Disk"));

    // Underrun > 0 shows "Disk" overload pill.
    let out_underrun = render_bar_with(0.2, 1);
    assert!(has_text(&out_underrun, "Disk"));

    // Load >= 1.0 shows "Disk" overload pill.
    let out_high_load = render_bar_with(1.0, 0);
    assert!(has_text(&out_high_load, "Disk"));

    // Healthy LiveCraftApp frame renders cleanly without "Disk" overload pill.
    let mut app = LiveCraftApp::from_session(Session::new(project, None), None);
    let out_app = step_frames(&ctx, &mut app, 2);
    assert!(!has_text(&out_app, "Disk"));
}

#[test]
fn dropping_audio_files_replaces_selected_or_inserts_onto_audio_track() {
    let dir = TempDirGuard::new("drop_audio_files");
    let src_dir = fixtures_audio_dir();
    let flac_path = dir.path().join("loop_fixture.flac");
    let ogg_path = dir.path().join("replacement_fixture.ogg");
    let wav_path = dir.path().join("extra_fixture.wav");
    fs::copy(src_dir.join("sine_440.flac"), &flac_path).expect("copy flac fixture");
    fs::copy(src_dir.join("sine_440.ogg"), &ogg_path).expect("copy ogg fixture");
    fs::copy(src_dir.join("sine_440.wav"), &wav_path).expect("copy wav fixture");

    let mut app = LiveCraftApp::from_session(
        Session::new(Project::new_default("AudioDropHost"), None),
        None,
    );
    let ctx = egui::Context::default();

    // 1. No audio clip selected (default points to MIDI Track 1 empty slot 0):
    // dropping a .flac inserts an audio clip on Audio track (Track 3 slot 0), selects it, and draws waveform.
    let mut drop_flac = raw_input_1280x800();
    drop_flac.dropped_files = vec![Arc::new(TestDroppedFile(flac_path.clone()))];
    let out_flac = run_frame(&ctx, &mut app, drop_flac);

    assert_eq!(app.selected_track, TrackId(3));
    assert_eq!(app.selected_clip, Some((TrackId(3), 0)));
    assert_eq!(app.bottom_tab, 1);
    let slot0_clip = app
        .session
        .project()
        .track(TrackId(3))
        .expect("t3")
        .clip_slots[0]
        .clip
        .as_ref()
        .expect("inserted flac clip");
    assert_eq!(slot0_clip.name, "loop_fixture");
    let ClipPayload::Audio(ref a1) = slot0_clip.payload else {
        panic!("expected audio clip payload");
    };
    assert!(a1.file_path.ends_with("loop_fixture.flac"));
    assert_eq!(a1.sample_rate, 44_100);
    assert_eq!(a1.channels, 2);
    assert!(total_waveform_vertical_bars(&out_flac) > 100);

    // 2. Now that audio clip in Track 3 slot 0 IS selected, dropping an .ogg replaces its sample!
    let mut drop_ogg = raw_input_1280x800();
    drop_ogg.dropped_files = vec![Arc::new(TestDroppedFile(ogg_path.clone()))];
    let _out_ogg = run_frame(&ctx, &mut app, drop_ogg);

    let t3 = app.session.project().track(TrackId(3)).expect("t3");
    assert!(
        t3.clip_slots[1].clip.is_none(),
        "replacing selected audio clip must not create a second clip"
    );
    let replaced_clip = t3.clip_slots[0].clip.as_ref().expect("replaced clip");
    let ClipPayload::Audio(ref a2) = replaced_clip.payload else {
        panic!("expected audio clip payload");
    };
    assert!(a2.file_path.ends_with("replacement_fixture.ogg"));

    // 3. Selecting an empty slot on Track 3 and dropping a .wav inserts a new clip there.
    app.selected_clip = Some((TrackId(3), 1));
    let mut drop_wav = raw_input_1280x800();
    drop_wav.dropped_files = vec![Arc::new(TestDroppedFile(wav_path.clone()))];
    let _out_wav = run_frame(&ctx, &mut app, drop_wav);

    let t3_after = app.session.project().track(TrackId(3)).expect("t3");
    let slot1_clip = t3_after.clip_slots[1].clip.as_ref().expect("slot 1 wav");
    let ClipPayload::Audio(ref a3) = slot1_clip.payload else {
        panic!("expected audio clip payload");
    };
    assert!(a3.file_path.ends_with("extra_fixture.wav"));

    // 4. Project with NO audio tracks creates a new Audio track on drop and places clip in slot 0.
    let mut midi_only = Project::new_default("MidiOnly");
    midi_only
        .tracks
        .retain(|t| t.track_type != TrackType::Audio);
    let tracks_before = midi_only.tracks.len();
    let mut midi_app = LiveCraftApp::from_session(Session::new(midi_only, None), None);
    let mut drop_new_trk = raw_input_1280x800();
    drop_new_trk.dropped_files = vec![Arc::new(TestDroppedFile(wav_path))];
    let _ = run_frame(&ctx, &mut midi_app, drop_new_trk);

    assert_eq!(midi_app.session.project().tracks.len(), tracks_before + 1);
    let created_trk = midi_app
        .session
        .project()
        .tracks
        .last()
        .expect("created audio track");
    assert_eq!(created_trk.track_type, TrackType::Audio);
    assert!(created_trk.clip_slots[0].clip.is_some());
}
