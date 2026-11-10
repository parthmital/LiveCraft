//! Headless egui integration tests driving `LiveCraftApp` (`eframe::App::ui` -> `render_ui`)
//! and `FileManagerWindow` across all M10 workflows.

use eframe::egui::{self, Key, Modifiers, PointerButton, Pos2, Rect, Shape, Vec2};
use livecraft_model::device::{Device, DeviceType};
use livecraft_model::ids::{ClipId, DeviceId, TrackId};
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};
use livecraft_session::{
    AlsDocument, CollectFilesOnExport, LibraryPreferences, SearchCandidateOutcome, Session,
};
use livecraft_ui::LiveCraftApp;
use livecraft_ui::browser::BrowserCategory;
use livecraft_ui::file_manager::FileManagerTab;
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
            .join("m10_ui")
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

fn write_pcm_wav(path: &Path, sample_rate: u32, frames: usize, amplitude: i16) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create wav parent");
    }
    let data_bytes = (frames * 2) as u32;
    let riff_size = 36 + data_bytes;
    let byte_rate = sample_rate * 2;
    let mut out = Vec::with_capacity(44 + frames * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..frames {
        let sample = if i % 2 == 0 { amplitude } else { -amplitude };
        out.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, out).expect("write wav");
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320u32 & mask);
        }
    }
    !crc
}

fn write_store_zip(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create zip parent");
    }
    let mut data = Vec::new();
    let mut central = Vec::new();
    for &(name, payload) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32(payload);
        let size = payload.len() as u32;
        let offset = data.len() as u32;

        data.extend_from_slice(b"PK\x03\x04");
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&crc.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = data.len() as u32;
    let cd_size = central.len() as u32;
    let count = entries.len() as u16;
    data.extend_from_slice(&central);
    data.extend_from_slice(b"PK\x05\x06");
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&cd_size.to_le_bytes());
    data.extend_from_slice(&cd_offset.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    fs::write(path, data).expect("write zip");
}

fn create_project_layout_on_disk(root: &Path) {
    fs::create_dir_all(root.join("Ableton Project Info")).expect("project info dir");
    fs::create_dir_all(root.join("Samples/Imported")).expect("imported dir");
    fs::create_dir_all(root.join("Samples/Recorded")).expect("recorded dir");
    fs::create_dir_all(root.join("Samples/Processed")).expect("processed dir");
}

fn make_audio_set(name: &str, refs: &[(&str, Option<&str>)]) -> Project {
    let mut project = Project::new_default(name);
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    for (idx, &(path, rel)) in refs.iter().enumerate() {
        let mut clip = Clip::new_audio(ClipId(900 + idx as u64), format!("Clip_{idx}"), path, 4.0);
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = rel.map(str::to_owned);
        }
        project.tracks[2].clip_slots[idx].clip = Some(clip);
    }
    project
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

fn find_containing_text_center(output: &egui::FullOutput, needle: &str) -> Option<Pos2> {
    all_texts(output)
        .into_iter()
        .find(|(text, _)| text.contains(needle))
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

fn click_containing(
    ctx: &egui::Context,
    app: &mut LiveCraftApp,
    output: &egui::FullOutput,
    needle: &str,
) -> egui::FullOutput {
    let pos = find_containing_text_center(output, needle)
        .unwrap_or_else(|| panic!("expected text containing '{needle}' in UI"));
    click_pos(ctx, app, pos)
}

#[test]
fn status_bar_warning_opens_missing_files_tab_and_repairs_auto_and_manual() {
    let dir = TempDirGuard::new("missing_repair");
    let proj_root = dir.path().join("Broken Project");
    create_project_layout_on_disk(&proj_root);

    let search_dir = dir.path().join("SearchPool");
    write_pcm_wav(&search_dir.join("One/unique_kick.wav"), 44_100, 1024, 9_000);
    write_pcm_wav(&search_dir.join("A/ambig_snare.wav"), 44_100, 1024, 9_000);
    write_pcm_wav(&search_dir.join("B/ambig_snare.wav"), 44_100, 1024, 9_000);

    let manual_hat = dir.path().join("ManualPool/hat_replacement.wav");
    write_pcm_wav(&manual_hat, 44_100, 1024, 9_000);

    let proj = make_audio_set(
        "MissingUI",
        &[
            ("C:/missing/unique_kick.wav", None),
            ("C:/missing/ambig_snare.wav", None),
            ("C:/missing/manual_hat.wav", None),
        ],
    );
    let als_path = proj_root.join("MissingUI.als");
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save als");

    let session = Session::open_als(&als_path).expect("open als");
    assert_eq!(session.missing_files().len(), 3);

    let mut app = LiveCraftApp::from_session(session, Some(als_path));
    let ctx = egui::Context::default();

    // 1. Status bar displays amber warning button and clicking it opens Missing Files tab.
    let out1 = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out1, "Media files are missing (3) — Locate"));
    assert!(!app.file_manager.open);

    let out2 = click_containing(&ctx, &mut app, &out1, "Media files are missing (3)");
    assert!(app.file_manager.open);
    assert_eq!(app.file_manager.tab, FileManagerTab::MissingFiles);
    assert!(has_text(&out2, "unique_kick.wav"));
    assert!(has_text(&out2, "ambig_snare.wav"));
    assert!(has_text(&out2, "manual_hat.wav"));
    assert!(has_text(&out2, "Automatic Search"));

    // 2. Run Automatic Search via Search Folder + Go button.
    app.file_manager.search_folder = search_dir.to_string_lossy().into_owned();
    let out3 = step_frames(&ctx, &mut app, 1);
    let out4 = click_exact(&ctx, &mut app, &out3, "Go");

    assert_eq!(app.session.missing_files().len(), 2);
    let auto_rep = app
        .file_manager
        .last_auto_report
        .as_ref()
        .expect("auto report recorded");
    assert_eq!(auto_rep.repaired.len(), 1);
    assert_eq!(auto_rep.unresolved.len(), 2);
    assert!(
        auto_rep
            .unresolved
            .iter()
            .any(|(_, o)| matches!(o, SearchCandidateOutcome::NoCandidate))
    );
    assert!(
        auto_rep.unresolved.iter().any(
            |(_, o)| matches!(o, SearchCandidateOutcome::MultipleCandidates(c) if c.len() == 2)
        )
    );
    assert!(has_text(&out4, "NoCandidate"));
    assert!(has_text(&out4, "MultipleCandidates"));

    // 3. Click one of the MultipleCandidates buttons to manually repair ambig_snare.wav.
    let _out5 = click_containing(&ctx, &mut app, &out4, "Use candidate:");
    assert_eq!(app.session.missing_files().len(), 1);
    assert_eq!(app.session.missing_files()[0].file_name, "manual_hat.wav");

    // 4. Enter manual replacement path for remaining missing file and click Replace.
    app.file_manager.manual_paths.insert(
        "C:/missing/manual_hat.wav".to_owned(),
        manual_hat.to_string_lossy().into_owned(),
    );
    let out6 = step_frames(&ctx, &mut app, 1);
    let out7 = click_exact(&ctx, &mut app, &out6, "Replace");

    assert!(app.session.missing_files().is_empty());
    assert!(!has_text(&out7, "Media files are missing"));
    assert!(has_text(&out7, "No missing media files detected."));
}

#[test]
fn collect_all_and_save_and_unused_files_dialogs_work_end_to_end() {
    let dir = TempDirGuard::new("collect_and_unused");
    let proj_root = dir.path().join("Main Project");
    create_project_layout_on_disk(&proj_root);
    let user_lib = dir.path().join("UserLibrary");

    let ext_elsewhere = dir.path().join("External/kick.wav");
    let ext_user_lib = user_lib.join("hats.wav");
    let orphan_imported = proj_root.join("Samples/Imported/orphan.wav");
    write_pcm_wav(&ext_elsewhere, 44_100, 1024, 8_000);
    write_pcm_wav(&ext_user_lib, 44_100, 1024, 8_000);
    write_pcm_wav(&orphan_imported, 44_100, 512, 4_000);

    let proj = make_audio_set(
        "CollectUI",
        &[
            (&ext_elsewhere.to_string_lossy(), None),
            (&ext_user_lib.to_string_lossy(), None),
        ],
    );
    let als_path = proj_root.join("CollectUI.als");
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save als");

    let mut session = Session::open_als(&als_path).expect("open als");
    session
        .set_library_preferences(LibraryPreferences {
            user_library: Some(user_lib),
            factory_packs_roots: Vec::new(),
            collect_files_on_export: CollectFilesOnExport::Always,
            ..LibraryPreferences::default()
        })
        .expect("set lib prefs");

    let mut app = LiveCraftApp::from_session(session, Some(als_path));
    app.file_manager
        .open_manage_files(FileManagerTab::ExternalFiles);
    let ctx = egui::Context::default();

    let out1 = step_frames(&ctx, &mut app, 2);
    let out2 = click_exact(&ctx, &mut app, &out1, "Collect and Save");

    let report = app
        .file_manager
        .last_collect_report
        .as_ref()
        .expect("collect report");
    assert_eq!(report.collected.len(), 1);
    assert_eq!(report.skipped_by_policy.len(), 1);
    assert!(proj_root.join("Samples/Imported/kick.wav").is_file());
    assert!(ext_elsewhere.is_file());
    assert!(has_text(&out2, "CollectReport Summary"));

    // Switch to Unused Files tab, scan, and perform two-step delete.
    let out3 = click_exact(&ctx, &mut app, &out2, "Unused Files");
    let out4 = click_exact(&ctx, &mut app, &out3, "Scan Unused Files");
    assert!(has_text(&out4, "Samples/Imported/orphan.wav"));
    assert!(has_text(&out4, "Imported (1)"));

    let out5 = click_exact(&ctx, &mut app, &out4, "Delete");
    assert!(orphan_imported.is_file(), "step 1 must not delete yet");
    assert!(has_text(&out5, "Confirm Delete"));

    let out6 = click_exact(&ctx, &mut app, &out5, "Confirm Delete");
    assert!(!orphan_imported.exists(), "step 2 must delete unused file");
    assert!(!has_text(&out6, "Samples/Imported/orphan.wav"));
}

#[test]
fn save_as_into_project_via_shortcut_and_dialog_updates_project_path() {
    let dir = TempDirGuard::new("save_as");
    let session = Session::new(Project::new_default("Untracked"), None);
    let mut app = LiveCraftApp::from_session(session, None);
    let ctx = egui::Context::default();

    let _ = step_frames(&ctx, &mut app, 1);

    // Ctrl+Shift+S opens Save Live Set As... without triggering Ctrl+S.
    let mut shortcut_in = raw_input_1280x800();
    let shift_cmd = Modifiers::COMMAND | Modifiers::SHIFT;
    shortcut_in.events = vec![
        egui::Event::ModifiersChanged(shift_cmd),
        egui::Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: shift_cmd,
        },
    ];
    let _ = run_frame(&ctx, &mut app, shortcut_in);
    assert!(app.file_manager.save_as_open);
    assert!(app.project_path.is_none());

    let target_als = dir.path().join("BrandNew.als");
    app.file_manager.save_as_path = target_als.to_string_lossy().into_owned();
    let out1 = step_frames(&ctx, &mut app, 2);
    let _out2 = click_exact(&ctx, &mut app, &out1, "Save Into Project");

    let expected_root = dir.path().join("BrandNew Project");
    let expected_als = expected_root.join("BrandNew.als");
    assert!(!app.file_manager.save_as_open);
    assert_eq!(app.project_path.as_deref(), Some(expected_als.as_path()));
    assert!(expected_als.is_file());
    assert!(expected_root.join("Ableton Project Info").is_dir());
    assert_eq!(
        app.session.project_context().project_root.as_deref(),
        Some(expected_root.as_path())
    );
}

#[test]
fn health_report_dialog_displays_issues_and_exports_json() {
    let dir = TempDirGuard::new("health_dialog");
    let proj_root = dir.path().join("Health Project");
    create_project_layout_on_disk(&proj_root);

    let unused_wav = proj_root.join("Samples/Imported/unused.wav");
    let ext_wav = dir.path().join("Outside/ext.wav");
    write_pcm_wav(&unused_wav, 44_100, 1024, 5_000);
    write_pcm_wav(&ext_wav, 44_100, 1024, 5_000);

    let mut proj = make_audio_set(
        "HealthUI",
        &[
            (&ext_wav.to_string_lossy(), None),
            ("C:/missing/gone.wav", None),
        ],
    );
    proj.tracks[0].device_chain.push(Device::new(
        DeviceId(999),
        "MissingVST",
        DeviceType::Custom("Vst3PluginInfo".to_owned()),
    ));
    let als_path = proj_root.join("HealthUI.als");
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save als");

    let session = Session::open_als(&als_path).expect("open als");
    let mut app = LiveCraftApp::from_session(session, Some(als_path));
    let ctx = egui::Context::default();

    // Open via Manage -> Project Health... menu bar flow.
    let out1 = step_frames(&ctx, &mut app, 2);
    let out2 = click_exact(&ctx, &mut app, &out1, "Manage");
    let out3 = click_exact(&ctx, &mut app, &out2, "Project Health...");

    assert!(app.file_manager.health_open);
    assert!(has_text(&out3, "ISSUES FOUND (4)"));
    assert!(has_text(&out3, "Missing Files (1)"));
    assert!(has_text(&out3, "Unused Files (1)"));
    assert!(has_text(&out3, "Missing Plug-ins (1)"));
    assert!(has_text(&out3, "External Files (1)"));

    let json_path = dir.path().join("out/health.json");
    app.file_manager.health_export_path = json_path.to_string_lossy().into_owned();
    let out4 = step_frames(&ctx, &mut app, 1);
    let _out5 = click_exact(&ctx, &mut app, &out4, "Export JSON");

    let exported = fs::read_to_string(&json_path).expect("read exported json");
    assert!(exported.contains("\"missing_files\""));
    assert!(exported.contains("\"unused_files\""));
    assert!(exported.contains("\"missing_plugins\""));
    assert!(exported.contains("\"external_files\""));
    assert!(exported.contains("gone.wav"));
}

#[test]
fn drag_and_drop_mid_alc_adv_adg_alp_als_obeys_manual_rules() {
    let dir = TempDirGuard::new("drag_drop");

    // Prepare fixture .mid, .alc (clip + 2 devices), .adv, .adg, .alp, and .als files.
    let mut src_proj = Project::new_default("Fixtures");
    let mut midi_clip = Clip::new_midi(ClipId(500), "BassRiff", 4.0);
    if let ClipPayload::Midi(ref mut m) = midi_clip.payload {
        m.notes.push(MidiNote::new(36, 0.0, 0.5, 110));
        m.notes.push(MidiNote::new(43, 1.0, 0.5, 105));
        m.notes.push(MidiNote::new(48, 2.0, 1.0, 100));
    }
    src_proj.tracks[1].clip_slots[0].clip = Some(midi_clip);
    let mut op = Device::new(DeviceId(600), "BassOp", DeviceType::Operator);
    op.set_param("filter_cutoff", 980.0);
    let mut sat = Device::new(DeviceId(601), "BassDrive", DeviceType::Saturator);
    sat.set_param("drive_db", 6.0);
    src_proj.tracks[1].device_chain.push(op);
    src_proj.tracks[1].device_chain.push(sat);

    let fixture_session = Session::new(src_proj, None);
    let mid_path = dir.path().join("riff.mid");
    let alc_path = dir.path().join("BassRiff.alc");
    let adv_path = dir.path().join("BassOp.adv");
    let adg_path = dir.path().join("BassRack.adg");
    fixture_session
        .export_midi_clip(ClipId(500), &mid_path)
        .expect("export mid");
    fixture_session
        .export_live_clip(TrackId(2), ClipId(500), &alc_path, None)
        .expect("export alc");
    fixture_session
        .save_device_preset(TrackId(2), DeviceId(600), &adv_path)
        .expect("save adv");
    fixture_session
        .save_rack_preset(TrackId(2), "BassRack", [0.0; 8], &adg_path)
        .expect("save adg");

    let alp_path = dir.path().join("Demo.alp");
    write_store_zip(&alp_path, &[("Demo/Sets/PackSet.als", b"placeholder")]);

    let mut alt_proj = Project::new_default("OpenedViaDrop");
    alt_proj.tracks[0].name = "DroppedTrack".to_owned();
    let als_path = dir.path().join("DroppedSet.als");
    AlsDocument::from_project(&alt_proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save als");

    let mut app =
        LiveCraftApp::from_session(Session::new(Project::new_default("DndHost"), None), None);
    let ctx = egui::Context::default();

    // 1. Drop .alc onto OCCUPIED track (Track 1 already has Operator device):
    // clip is imported into first empty slot, devices are NOT added.
    app.selected_track = TrackId(1);
    let t1_devices_before = app
        .session
        .project()
        .track(TrackId(1))
        .expect("t1")
        .device_chain
        .len();
    assert!(t1_devices_before > 0);
    let mut drop_alc_occ = raw_input_1280x800();
    drop_alc_occ.dropped_files = vec![Arc::new(TestDroppedFile(alc_path.clone()))];
    let _ = run_frame(&ctx, &mut app, drop_alc_occ);

    let t1_after = app.session.project().track(TrackId(1)).expect("t1");
    assert!(t1_after.clip_slots[0].clip.is_some());
    assert_eq!(t1_after.device_chain.len(), t1_devices_before);

    // 2. Drop .alc onto EMPTY track (Track 2 has 0 clips, 0 devices):
    // clip AND both devices from .alc are restored.
    app.selected_track = TrackId(2);
    assert!(
        app.session
            .project()
            .track(TrackId(2))
            .expect("t2")
            .device_chain
            .is_empty()
    );
    let mut drop_alc_empty = raw_input_1280x800();
    drop_alc_empty.dropped_files = vec![Arc::new(TestDroppedFile(alc_path))];
    let _ = run_frame(&ctx, &mut app, drop_alc_empty);

    let t2_after = app.session.project().track(TrackId(2)).expect("t2");
    assert!(t2_after.clip_slots[0].clip.is_some());
    assert_eq!(t2_after.device_chain.len(), 2);

    // 3. Drop .mid when an Audio track (Track 3) is selected -> creates a new MIDI track with 3 notes.
    app.selected_track = TrackId(3);
    let tracks_before = app.session.project().tracks.len();
    let mut drop_mid = raw_input_1280x800();
    drop_mid.dropped_files = vec![Arc::new(TestDroppedFile(mid_path))];
    let _ = run_frame(&ctx, &mut app, drop_mid);

    assert_eq!(app.session.project().tracks.len(), tracks_before + 1);
    let new_trk = app.session.project().tracks.last().expect("new midi track");
    let imported_clip = new_trk.clip_slots[0]
        .clip
        .as_ref()
        .expect("imported mid clip");
    let ClipPayload::Midi(ref midi_data) = imported_clip.payload else {
        panic!("expected MIDI clip payload");
    };
    assert_eq!(midi_data.notes.len(), 3);

    // 4. Drop .adv and .adg onto Track 4.
    app.selected_track = TrackId(4);
    let mut drop_adv = raw_input_1280x800();
    drop_adv.dropped_files = vec![Arc::new(TestDroppedFile(adv_path))];
    let _ = run_frame(&ctx, &mut app, drop_adv);
    assert_eq!(
        app.session
            .project()
            .track(TrackId(4))
            .expect("t4")
            .device_chain
            .len(),
        1
    );

    let mut drop_adg = raw_input_1280x800();
    drop_adg.dropped_files = vec![Arc::new(TestDroppedFile(adg_path))];
    let _ = run_frame(&ctx, &mut app, drop_adg);
    assert_eq!(
        app.session
            .project()
            .track(TrackId(4))
            .expect("t4")
            .device_chain
            .len(),
        3
    );

    // 5. Drop .alp -> opens Install Pack dialog pre-filled, and clicking Install Pack extracts it.
    let pack_dest = dir.path().join("PackDest");
    let mut drop_alp = raw_input_1280x800();
    drop_alp.dropped_files = vec![Arc::new(TestDroppedFile(alp_path))];
    let _ = run_frame(&ctx, &mut app, drop_alp);
    assert!(app.file_manager.install_pack_open);
    app.file_manager.pack_dest_dir = pack_dest.to_string_lossy().into_owned();
    let out_pack = step_frames(&ctx, &mut app, 2);
    let _ = click_exact(&ctx, &mut app, &out_pack, "Install Pack");
    assert!(pack_dest.join("Demo/Sets/PackSet.als").is_file());

    // 6. Drop .als -> opens the Set and updates app.project_path.
    let mut drop_als = raw_input_1280x800();
    drop_als.dropped_files = vec![Arc::new(TestDroppedFile(als_path.clone()))];
    let _ = run_frame(&ctx, &mut app, drop_als);
    assert_eq!(app.project_path.as_deref(), Some(als_path.as_path()));
    assert_eq!(app.session.project().name, "DroppedSet");
    assert_eq!(app.session.project().tracks[0].name, "DroppedTrack");
}

#[test]
fn collect_files_on_export_ask_modal_and_view_buttons_work() {
    let dir = TempDirGuard::new("ask_and_views");
    let ext_wav = dir.path().join("External/vox.wav");
    write_pcm_wav(&ext_wav, 44_100, 1024, 11_000);

    let mut proj = Project::new_default("AskPolicy");
    let mut midi_clip = Clip::new_midi(ClipId(809), "LeadLine", 4.0);
    if let ClipPayload::Midi(ref mut m) = midi_clip.payload {
        m.notes.push(MidiNote::new(60, 0.0, 1.0, 100));
    }
    proj.tracks[0].clip_slots[0].clip = Some(midi_clip);
    proj.tracks[2].clip_slots[0].clip = Some(Clip::new_audio(
        ClipId(810),
        "VoxTake",
        ext_wav.to_string_lossy(),
        4.0,
    ));

    let mut session = Session::new(proj, None);
    session
        .set_library_preferences(LibraryPreferences {
            collect_files_on_export: CollectFilesOnExport::Ask,
            ..LibraryPreferences::default()
        })
        .expect("set ask policy");

    let mut app = LiveCraftApp::from_session(session, None);
    let ctx = egui::Context::default();

    // 1. Device Chain buttons: Save .adv and Save Rack .adg on Track 1 (has Operator).
    app.selected_track = TrackId(1);
    app.bottom_tab = 0;
    let out1 = step_frames(&ctx, &mut app, 2);
    let _out2 = click_exact(&ctx, &mut app, &out1, "Save .adv");
    let adv_out = dir.path().join("SavedOp.adv");
    app.file_manager
        .prompt
        .as_mut()
        .expect("save adv prompt")
        .path = adv_out.to_string_lossy().into_owned();
    let out3 = step_frames(&ctx, &mut app, 2);
    let out4 = click_exact(&ctx, &mut app, &out3, "Save");
    assert!(adv_out.is_file());

    let _out5 = click_exact(&ctx, &mut app, &out4, "Save Rack .adg");
    let adg_out = dir.path().join("SavedRack.adg");
    app.file_manager
        .prompt
        .as_mut()
        .expect("save adg prompt")
        .path = adg_out.to_string_lossy().into_owned();
    let out6 = step_frames(&ctx, &mut app, 2);
    let _out7 = click_exact(&ctx, &mut app, &out6, "Save");
    assert!(adg_out.is_file());

    // 2. Clip View Save .alc on Audio clip triggers Ask modal (Copy / Don't Copy / Cancel).
    app.selected_track = TrackId(3);
    app.selected_clip = Some((TrackId(3), 0));
    app.bottom_tab = 1;
    let out8 = step_frames(&ctx, &mut app, 2);
    let _out9 = click_exact(&ctx, &mut app, &out8, "Save .alc");

    let dont_copy_dir = dir.path().join("ExportDontCopy");
    let dont_copy_alc = dont_copy_dir.join("VoxNoCopy.alc");
    app.file_manager
        .prompt
        .as_mut()
        .expect("export alc prompt")
        .path = dont_copy_alc.to_string_lossy().into_owned();
    let out10 = step_frames(&ctx, &mut app, 2);
    let out11 = click_exact(&ctx, &mut app, &out10, "Export");

    assert!(app.file_manager.ask_copy_on_export.is_some());
    assert!(has_text(&out11, "Collect Files on Export"));
    assert!(has_text(&out11, "Copy"));
    assert!(has_text(&out11, "Don't Copy"));
    assert!(has_text(&out11, "Cancel"));

    let _out12 = click_exact(&ctx, &mut app, &out11, "Don't Copy");
    assert!(dont_copy_alc.is_file());
    assert!(!dont_copy_dir.join("Samples/Imported/vox.wav").exists());

    // Repeat and choose "Copy".
    let copy_dir = dir.path().join("ExportCopy");
    let copy_alc = copy_dir.join("VoxCopy.alc");
    app.file_manager
        .open_export_alc(TrackId(3), ClipId(810), "VoxTake");
    app.file_manager
        .prompt
        .as_mut()
        .expect("export alc prompt")
        .path = copy_alc.to_string_lossy().into_owned();
    let out13 = step_frames(&ctx, &mut app, 2);
    let out14 = click_exact(&ctx, &mut app, &out13, "Export");
    let _out15 = click_exact(&ctx, &mut app, &out14, "Copy");
    assert!(copy_alc.is_file());
    assert!(copy_dir.join("Samples/Imported/vox.wav").is_file());

    // Repeat and choose "Cancel".
    let cancel_alc = dir.path().join("ExportCancel/VoxCancel.alc");
    app.file_manager
        .open_export_alc(TrackId(3), ClipId(810), "VoxTake");
    app.file_manager
        .prompt
        .as_mut()
        .expect("export alc prompt")
        .path = cancel_alc.to_string_lossy().into_owned();
    let out_cancel_1 = step_frames(&ctx, &mut app, 2);
    let out_cancel_2 = click_exact(&ctx, &mut app, &out_cancel_1, "Export");
    let _out_cancel_3 = click_exact(&ctx, &mut app, &out_cancel_2, "Cancel");
    assert!(!cancel_alc.exists());

    // 2b. Clip View Export MIDI button on MIDI clip writes a .mid file.
    app.selected_track = TrackId(1);
    app.selected_clip = Some((TrackId(1), 0));
    app.bottom_tab = 1;
    let out_midi_1 = step_frames(&ctx, &mut app, 2);
    let _out_midi_2 = click_exact(&ctx, &mut app, &out_midi_1, "Export MIDI");
    let mid_out = dir.path().join("ExportedLead.mid");
    app.file_manager
        .prompt
        .as_mut()
        .expect("export midi prompt")
        .path = mid_out.to_string_lossy().into_owned();
    let out_midi_3 = step_frames(&ctx, &mut app, 2);
    let _out_midi_4 = click_exact(&ctx, &mut app, &out_midi_3, "Export");
    assert!(mid_out.is_file());

    // 3. Browser real-file scan + click import across UserLibrary (.adv/.adg), Clips (.alc), and Packs (.mid).
    let user_lib = dir.path().join("UserLib");
    let packs_dir = dir.path().join("FactoryPacks");
    fs::create_dir_all(&user_lib).expect("create user lib");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    fs::copy(&adv_out, user_lib.join("TestPreset.adv")).expect("copy adv into user lib");
    fs::copy(&adg_out, user_lib.join("TestRack.adg")).expect("copy adg into user lib");
    fs::copy(&dont_copy_alc, user_lib.join("ClipItem.alc")).expect("copy alc into user lib");
    fs::copy(&mid_out, packs_dir.join("PackMelody.mid")).expect("copy mid into packs");

    app.session
        .set_library_preferences(LibraryPreferences {
            user_library: Some(user_lib),
            factory_packs_roots: vec![packs_dir],
            collect_files_on_export: CollectFilesOnExport::Ask,
            ..LibraryPreferences::default()
        })
        .expect("update user lib");
    app.selected_track = TrackId(2);
    assert!(
        app.session
            .project()
            .track(TrackId(2))
            .expect("t2")
            .device_chain
            .is_empty()
    );

    app.browser.selected_category = BrowserCategory::UserLibrary;
    app.browser.invalidate();
    let out16 = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out16, "TestPreset.adv"));
    let out17 = click_exact(&ctx, &mut app, &out16, "TestPreset.adv");
    assert_eq!(
        app.session
            .project()
            .track(TrackId(2))
            .expect("t2")
            .device_chain
            .len(),
        1
    );

    let _out18 = click_exact(&ctx, &mut app, &out17, "TestRack.adg");
    assert_eq!(
        app.session
            .project()
            .track(TrackId(2))
            .expect("t2")
            .device_chain
            .len(),
        2
    );

    app.browser.selected_category = BrowserCategory::Packs;
    app.browser.invalidate();
    let out19 = step_frames(&ctx, &mut app, 2);
    let tracks_before_mid = app.session.project().tracks.len();
    app.selected_track = TrackId(4); // Audio track -> creates new MIDI track on .mid click
    let _out20 = click_exact(&ctx, &mut app, &out19, "PackMelody.mid");
    assert_eq!(app.session.project().tracks.len(), tracks_before_mid + 1);

    app.browser.selected_category = BrowserCategory::Clips;
    app.browser.invalidate();
    let out21 = step_frames(&ctx, &mut app, 2);
    assert!(has_text(&out21, "ClipItem.alc"));
    app.selected_track = TrackId(3);
    let _out22 = click_exact(&ctx, &mut app, &out21, "ClipItem.alc");
    assert!(
        app.session
            .project()
            .track(TrackId(3))
            .expect("t3")
            .clip_slots[1]
            .clip
            .is_some()
    );
}
