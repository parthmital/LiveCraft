use crate::Edits;
use crate::arrange::render_arrangement_view;
use crate::browser::LiveBrowser;
use crate::devices::render_device_chain;
use crate::file_manager::{FileManagerTab, FileManagerWindow};
use crate::pianoroll::render_piano_roll;
use crate::session::render_session_view;
use crate::settings::SettingsWindow;
use crate::style::{LivePalette, apply_live_style};
use crate::transport::{ViewMode, render_transport_bar};
use eframe::egui::{self, Color32, Key, Vec2};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::{Clip, ClipPayload, Project, TrackType};
use livecraft_session::commands::device::InsertDevice;
use livecraft_session::commands::project::InsertPreparedClip;
use livecraft_session::commands::track::{
    CreateAudioTrack, CreateMidiTrack, SetTrackArmed, SetTrackMuted, SetTrackSoloed,
};
use livecraft_session::{
    CachedSampleAnalysis, Command, Session, SessionFormatError, demo_set, probe_audio_info,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SampleAnalysisCacheKey {
    clip_id: ClipId,
    file_path: String,
    relative_path: Option<String>,
    create_analysis_files: bool,
    analysis_cache_folder: Option<PathBuf>,
    project_root: Option<PathBuf>,
}

struct SampleAnalysisCacheEntry {
    key: SampleAnalysisCacheKey,
    analysis: Option<CachedSampleAnalysis>,
}

pub struct LiveCraftApp {
    /// The open Set, its undo history, and its audio engine. Every change goes through it.
    pub session: Session,
    pub project_path: Option<PathBuf>,
    pub view_mode: ViewMode,
    pub selected_track: TrackId,
    pub selected_clip: Option<(TrackId, usize)>, // (track, slot_idx)
    pub bottom_tab: usize,                       // 0 = Device Chain, 1 = Piano Roll / Clip View
    pub browser: LiveBrowser,
    pub palette: LivePalette,
    pub status_message: String,
    pub screenshot_path: Option<PathBuf>,
    pub frame_counter: u32,
    pub browser_visible: bool,
    /// The Settings window (Ctrl + , in Live, manual 2.3).
    pub settings: SettingsWindow,
    /// M10 File Manager, Save As, Health Report, Pack Install, and export dialogs.
    pub file_manager: FileManagerWindow,
    /// Cached `.lcanalysis` waveform pyramid for the active Audio Clip View selection.
    sample_analysis_cache: Option<SampleAnalysisCacheEntry>,
}

impl LiveCraftApp {
    pub fn new(project_path: Option<&Path>) -> Self {
        Self::new_with_options(crate::GuiOptions {
            project_path: project_path.map(|p| p.to_path_buf()),
            ..Default::default()
        })
    }

    /// Creates a UI shell around an existing [`Session`] (used by headless tests).
    pub fn from_session(session: Session, project_path: Option<PathBuf>) -> Self {
        let selected_track = session
            .project()
            .tracks
            .first()
            .map_or(TrackId(0), |track| track.id);
        Self {
            session,
            project_path,
            view_mode: ViewMode::Session,
            selected_track,
            selected_clip: Some((selected_track, 0)),
            bottom_tab: 0,
            browser: LiveBrowser::default(),
            palette: LivePalette::dark(),
            status_message: "LiveCraft Ready".to_string(),
            screenshot_path: None,
            frame_counter: 0,
            browser_visible: true,
            settings: SettingsWindow::default(),
            file_manager: FileManagerWindow::default(),
            sample_analysis_cache: None,
        }
    }

    pub fn new_with_options(options: crate::GuiOptions) -> Self {
        let palette = match options.theme.as_deref() {
            Some("light") => LivePalette::light(),
            _ => LivePalette::dark(),
        };

        let loaded = options.project_path.as_ref().and_then(|path| {
            Session::open_file(path)
                .ok()
                .map(|session| (session, path.clone()))
        });
        let (session, project_path) = match loaded {
            Some((session, path)) => (session, Some(path)),
            None => (
                Session::open(
                    demo_set().unwrap_or_else(|_| Project::new_default("LiveCraft Session")),
                ),
                None,
            ),
        };

        // Try initializing audio engine

        let view_mode = match options.initial_view.as_deref() {
            Some("arrange") | Some("arrangement") => ViewMode::Arrangement,
            _ => ViewMode::Session,
        };

        let bottom_tab = options.initial_tab.unwrap_or(0);
        let selected_track = session
            .project()
            .tracks
            .first()
            .map_or(TrackId(0), |track| track.id);
        let mut settings = SettingsWindow::default();
        if options.open_settings {
            settings.show_window();
        }
        Self {
            session,
            project_path,
            view_mode,
            selected_track,
            selected_clip: Some((selected_track, 0)),
            bottom_tab,
            browser: LiveBrowser::default(),
            palette,
            status_message: "LiveCraft Ready".to_string(),
            screenshot_path: options.screenshot_path,
            frame_counter: 0,
            browser_visible: true,
            settings,
            file_manager: FileManagerWindow::default(),
            sample_analysis_cache: None,
        }
    }

    /// Applies the changes a view asked for, through the session.
    fn apply(&mut self, edits: Edits) {
        if let Err(error) = self.session.apply(edits) {
            self.status_message = format!("Edit refused: {error}");
        }
    }

    fn save_current_set(&mut self) {
        let path = self
            .project_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("Untitled.als"));
        match self.session.save(&path) {
            Ok(()) => {
                self.status_message = format!("Saved to {}", path.display());
                self.browser.invalidate();
            }
            Err(error) => self.status_message = format!("Save failed: {error}"),
        }
        self.project_path = Some(path);
    }

    fn new_empty_set(&mut self) {
        self.session = Session::open(Project::new_default("Untitled"));
        self.project_path = None;
        self.selected_track = self
            .session
            .project()
            .tracks
            .first()
            .map_or(TrackId(0), |t| t.id);
        self.selected_clip = Some((self.selected_track, 0));
        self.browser.invalidate();
        self.sample_analysis_cache = None;
        self.status_message = "Created new Live Set".to_string();
    }

    fn open_set_from_path(&mut self, path: &Path) {
        match Session::open_file(path) {
            Ok(session) => {
                self.session = session;
                self.project_path = Some(path.to_path_buf());
                self.selected_track = self
                    .session
                    .project()
                    .tracks
                    .first()
                    .map_or(TrackId(0), |t| t.id);
                self.selected_clip = Some((self.selected_track, 0));
                self.browser.invalidate();
                self.sample_analysis_cache = None;
                let missing = self.session.missing_files().len();
                self.status_message = if missing > 0 {
                    format!("Opened {} ({missing} missing file(s))", path.display())
                } else {
                    format!("Opened {}", path.display())
                };
            }
            Err(err) => {
                self.status_message = format!("Failed to open {}: {err}", path.display());
            }
        }
    }

    fn insert_audio_track(&mut self) {
        let number = self.session.project().tracks.len() + 1;
        let name = format!("{number}-Audio");
        self.apply(vec![Box::new(CreateAudioTrack::new(name))]);
        self.status_message = "Inserted Audio Track".to_string();
    }

    fn insert_midi_track(&mut self) {
        let number = self.session.project().tracks.len() + 1;
        let name = format!("{number}-MIDI");
        self.apply(vec![Box::new(CreateMidiTrack::new(name))]);
        self.status_message = "Inserted MIDI Track".to_string();
    }

    fn find_clip_for_export(&self, midi_only: bool) -> Option<(TrackId, ClipId, String)> {
        let project = self.session.project();
        if let Some((t_id, s_idx)) = self.selected_clip
            && let Some(clip) = project
                .track(t_id)
                .and_then(|t| t.clip_slots.get(s_idx))
                .and_then(|s| s.clip.as_ref())
            && (!midi_only || matches!(clip.payload, ClipPayload::Midi(_)))
        {
            return Some((t_id, clip.id, clip.name.clone()));
        }
        if let Some(trk) = project.track(self.selected_track) {
            for slot in &trk.clip_slots {
                if let Some(clip) = &slot.clip
                    && (!midi_only || matches!(clip.payload, ClipPayload::Midi(_)))
                {
                    return Some((trk.id, clip.id, clip.name.clone()));
                }
            }
            for arr in &trk.arrangement_clips {
                if !midi_only || matches!(arr.clip.payload, ClipPayload::Midi(_)) {
                    return Some((trk.id, arr.clip.id, arr.clip.name.clone()));
                }
            }
        }
        for trk in &project.tracks {
            for slot in &trk.clip_slots {
                if let Some(clip) = &slot.clip
                    && (!midi_only || matches!(clip.payload, ClipPayload::Midi(_)))
                {
                    return Some((trk.id, clip.id, clip.name.clone()));
                }
            }
            for arr in &trk.arrangement_clips {
                if !midi_only || matches!(arr.clip.payload, ClipPayload::Midi(_)) {
                    return Some((trk.id, arr.clip.id, arr.clip.name.clone()));
                }
            }
        }
        None
    }

    fn open_export_midi_prompt(&mut self, explicit_clip: Option<ClipId>) {
        if let Some(cid) = explicit_clip
            && let Some((_, _, name)) = self.find_clip_by_id(cid)
        {
            self.file_manager.open_export_midi(cid, &name);
            return;
        }
        if let Some((_, cid, name)) = self.find_clip_for_export(true) {
            self.file_manager.open_export_midi(cid, &name);
        } else {
            self.status_message = "No MIDI clip available to export".to_string();
        }
    }

    fn open_export_alc_prompt(&mut self, explicit_clip: Option<ClipId>) {
        if let Some(cid) = explicit_clip
            && let Some((tid, _, name)) = self.find_clip_by_id(cid)
        {
            self.file_manager.open_export_alc(tid, cid, &name);
            return;
        }
        if let Some((tid, cid, name)) = self.find_clip_for_export(false) {
            self.file_manager.open_export_alc(tid, cid, &name);
        } else {
            self.status_message = "No clip available to export as .alc".to_string();
        }
    }

    fn find_clip_by_id(&self, clip_id: ClipId) -> Option<(TrackId, Option<usize>, String)> {
        for trk in self.session.project().all_tracks() {
            for (slot_idx, slot) in trk.clip_slots.iter().enumerate() {
                if let Some(clip) = &slot.clip
                    && clip.id == clip_id
                {
                    return Some((trk.id, Some(slot_idx), clip.name.clone()));
                }
            }
            for arr in &trk.arrangement_clips {
                if arr.clip.id == clip_id {
                    return Some((trk.id, None, arr.clip.name.clone()));
                }
            }
        }
        None
    }

    fn select_imported_clip(&mut self, clip_id: ClipId) {
        if let Some((tid, slot_opt, _)) = self.find_clip_by_id(clip_id) {
            self.selected_track = tid;
            if let Some(slot_idx) = slot_opt {
                self.selected_clip = Some((tid, slot_idx));
            }
        }
    }

    fn import_mid_path(&mut self, path: &Path) {
        let arr_beat = (self.view_mode == ViewMode::Arrangement)
            .then(|| self.session.status().playhead_beats.max(0.0));
        let is_midi_track = self
            .session
            .project()
            .track(self.selected_track)
            .is_some_and(|t| t.track_type == TrackType::Midi);
        let target = is_midi_track.then_some(self.selected_track);
        let attempt = self.session.import_midi_file(target, None, arr_beat, path);
        let res = match attempt {
            Err(SessionFormatError::InvalidPlacement(_)) if target.is_some() => {
                self.session.import_midi_file(None, None, arr_beat, path)
            }
            other => other,
        };
        match res {
            Ok(clip_id) => {
                self.select_imported_clip(clip_id);
                self.status_message = format!("Imported MIDI file: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("MIDI import failed: {err}");
            }
        }
    }

    fn import_alc_path(&mut self, path: &Path) {
        let arr_beat = (self.view_mode == ViewMode::Arrangement)
            .then(|| self.session.status().playhead_beats.max(0.0));
        let attempt =
            self.session
                .import_live_clip(Some(self.selected_track), None, arr_beat, path);
        let res = match attempt {
            Err(
                SessionFormatError::TrackTypeMismatch { .. }
                | SessionFormatError::InvalidPlacement(_)
                | SessionFormatError::Edit(
                    EditError::NoSuchTrack(_)
                    | EditError::GroupHoldsClips
                    | EditError::ReturnHoldsClips,
                ),
            ) => self.session.import_live_clip(None, None, arr_beat, path),
            other => other,
        };
        match res {
            Ok(clip_id) => {
                self.select_imported_clip(clip_id);
                self.status_message = format!("Imported Live Clip: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Live Clip import failed: {err}");
            }
        }
    }

    fn load_adv_path(&mut self, path: &Path) {
        match self.session.load_device_preset(self.selected_track, path) {
            Ok(_) => {
                self.bottom_tab = 0;
                self.status_message = format!("Loaded device preset: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Load .adv failed: {err}");
            }
        }
    }

    fn load_adg_path(&mut self, path: &Path) {
        match self.session.load_rack_preset(self.selected_track, path) {
            Ok(ids) => {
                self.bottom_tab = 0;
                self.status_message = format!(
                    "Loaded rack preset ({} device(s)): {}",
                    ids.len(),
                    path.display()
                );
            }
            Err(err) => {
                self.status_message = format!("Load .adg failed: {err}");
            }
        }
    }

    fn selected_audio_clip_id(&self) -> Option<ClipId> {
        let (t_id, s_idx) = self.selected_clip?;
        self.session
            .project()
            .track(t_id)
            .and_then(|trk| trk.clip_slots.get(s_idx))
            .and_then(|slot| slot.clip.as_ref())
            .filter(|clip| matches!(clip.payload, ClipPayload::Audio(_)))
            .map(|clip| clip.id)
    }

    fn find_audio_drop_slot(&self) -> Option<(TrackId, usize)> {
        let project = self.session.project();
        if let Some(trk) = project.track(self.selected_track)
            && trk.track_type == TrackType::Audio
        {
            if let Some((sel_t, sel_s)) = self.selected_clip
                && sel_t == self.selected_track
                && trk
                    .clip_slots
                    .get(sel_s)
                    .is_some_and(|slot| slot.clip.is_none())
            {
                return Some((self.selected_track, sel_s));
            }
            if let Some(slot_idx) = trk.clip_slots.iter().position(|slot| slot.clip.is_none()) {
                return Some((self.selected_track, slot_idx));
            }
        }
        for trk in &project.tracks {
            if trk.track_type == TrackType::Audio
                && let Some(slot_idx) = trk.clip_slots.iter().position(|slot| slot.clip.is_none())
            {
                return Some((trk.id, slot_idx));
            }
        }
        None
    }

    fn refresh_selected_clip_analysis(&mut self, track_id: TrackId, slot_idx: usize) {
        let Some(clip) = self
            .session
            .project()
            .track(track_id)
            .and_then(|trk| trk.clip_slots.get(slot_idx))
            .and_then(|slot| slot.clip.as_ref())
        else {
            return;
        };
        let ClipPayload::Audio(ref audio) = clip.payload else {
            return;
        };
        let lib = &self.session.preferences().library;
        let key = SampleAnalysisCacheKey {
            clip_id: clip.id,
            file_path: audio.file_path.clone(),
            relative_path: audio.relative_path.clone(),
            create_analysis_files: lib.create_analysis_files,
            analysis_cache_folder: lib.analysis_cache_folder.clone(),
            project_root: self.session.project_context().project_root.clone(),
        };
        let needs_load = self
            .sample_analysis_cache
            .as_ref()
            .is_none_or(|entry| entry.key != key);
        if needs_load {
            let analysis = self.session.sample_analysis(clip);
            self.sample_analysis_cache = Some(SampleAnalysisCacheEntry { key, analysis });
        }
    }

    fn drop_audio_path(&mut self, path: &Path) {
        if let Some(clip_id) = self.selected_audio_clip_id() {
            match self.session.replace_clip_sample(clip_id, path) {
                Ok(()) => {
                    self.sample_analysis_cache = None;
                    self.bottom_tab = 1;
                    self.status_message = format!(
                        "Replaced sample for clip #{} with {}",
                        clip_id.0,
                        path.display()
                    );
                }
                Err(err) => {
                    self.status_message = format!("Replace sample failed: {err}");
                }
            }
            return;
        }

        let info = match probe_audio_info(path) {
            Ok(info) => info,
            Err(err) => {
                self.status_message =
                    format!("Failed to read audio file {}: {err}", path.display());
                return;
            }
        };

        let bpm = f64::from(self.session.project().transport.bpm()).max(1.0);
        let length_beats = match info.total_frames {
            Some(frames) if frames > 0 && info.sample_rate > 0 => {
                let secs = frames as f64 / f64::from(info.sample_rate);
                (secs * (bpm / 60.0)).max(0.25)
            }
            _ => 4.0,
        };
        let clip_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Audio Clip")
            .to_owned();

        let abs_path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let stored_path = abs_path.to_string_lossy().replace('\\', "/");
        let relative_path = self
            .session
            .project_context()
            .project_root
            .as_deref()
            .and_then(|root| abs_path.strip_prefix(root).ok())
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));

        let mut clip = Clip::new_audio(ClipId(0), &clip_name, stored_path, length_beats);
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = relative_path;
            audio.sample_rate = info.sample_rate;
            audio.channels = info.channels;
        }

        let target = self.find_audio_drop_slot();
        let new_track_name = format!("{}-Audio", self.session.project().tracks.len() + 1);
        let mut placed: Option<(TrackId, usize)> = None;
        let tx_res = self.session.transaction("Import Audio File", |tx| {
            let (track_id, slot_idx) = match target {
                Some((tid, slot)) => (tid, slot),
                None => {
                    tx.run(CreateAudioTrack::new(new_track_name))?;
                    let tid = tx
                        .project()
                        .tracks
                        .last()
                        .map(|t| t.id)
                        .ok_or(EditError::NoSuchTrack(TrackId(0)))?;
                    (tid, 0)
                }
            };
            tx.run(InsertPreparedClip::new(track_id, slot_idx, clip))?;
            placed = Some((track_id, slot_idx));
            Ok(())
        });

        match tx_res {
            Ok(()) => {
                if let Some((tid, slot_idx)) = placed {
                    self.selected_track = tid;
                    self.selected_clip = Some((tid, slot_idx));
                    self.sample_analysis_cache = None;
                    self.bottom_tab = 1;
                }
                self.status_message = format!("Inserted audio clip: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Audio import failed: {err}");
            }
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let path = file.path();
            if path.as_os_str().is_empty() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            match ext.as_deref() {
                Some("mid" | "midi") => self.import_mid_path(path),
                Some("alc") => self.import_alc_path(path),
                Some(
                    "wav" | "aif" | "aiff" | "aifc" | "flac" | "mp3" | "ogg" | "oga" | "opus"
                    | "m4a" | "aac",
                ) => self.drop_audio_path(path),
                Some("adv") => self.load_adv_path(path),
                Some("adg") => self.load_adg_path(path),
                Some("alp") => {
                    self.file_manager.open_install_pack(Some(path));
                    self.status_message = format!("Ready to install Pack: {}", path.display());
                }
                Some("als") => self.open_set_from_path(path),
                _ => {
                    self.status_message = format!("Unsupported dropped file: {}", path.display());
                }
            }
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Set").clicked() {
                    self.new_empty_set();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    self.save_current_set();
                }
                if ui.button("Save Live Set As... (Ctrl+Shift+S)").clicked() {
                    let name = self.session.project().name.clone();
                    self.file_manager
                        .open_save_as(self.project_path.as_deref(), &name);
                }
                if ui.button("Collect All and Save...").clicked() {
                    self.file_manager
                        .open_manage_files(FileManagerTab::ExternalFiles);
                }
                ui.separator();
                if ui.button("Import MIDI File...").clicked() {
                    self.file_manager
                        .open_import_midi(Some(self.selected_track));
                }
                if ui.button("Export MIDI Clip...").clicked() {
                    self.open_export_midi_prompt(None);
                }
                if ui.button("Import Live Clip (.alc)...").clicked() {
                    self.file_manager.open_import_alc(Some(self.selected_track));
                }
                if ui.button("Export Live Clip (.alc)...").clicked() {
                    self.open_export_alc_prompt(None);
                }
                ui.separator();
                if ui.button("Install Pack...").clicked() {
                    self.file_manager.open_install_pack(None);
                }
            });

            ui.menu_button("Create", |ui| {
                if ui.button("Insert Audio Track (Ctrl+T)").clicked() {
                    self.insert_audio_track();
                }
                if ui.button("Insert MIDI Track (Ctrl+Shift+T)").clicked() {
                    self.insert_midi_track();
                }
                ui.separator();
                if ui.button("Load Device Preset (.adv)...").clicked() {
                    self.file_manager.open_load_adv(self.selected_track);
                }
                if ui.button("Load Rack Preset (.adg)...").clicked() {
                    self.file_manager.open_load_adg(self.selected_track);
                }
            });

            ui.menu_button("Manage", |ui| {
                if ui.button("Manage Files...").clicked() {
                    self.file_manager
                        .open_manage_files(FileManagerTab::MissingFiles);
                }
                if ui.button("Locate Missing Files...").clicked() {
                    self.file_manager
                        .open_manage_files(FileManagerTab::MissingFiles);
                }
                if ui.button("Collect All and Save...").clicked() {
                    self.file_manager
                        .open_manage_files(FileManagerTab::ExternalFiles);
                }
                if ui.button("Find Unused Files...").clicked() {
                    self.file_manager
                        .open_manage_files(FileManagerTab::UnusedFiles);
                }
                ui.separator();
                if ui.button("Project Health...").clicked() {
                    self.file_manager.open_health_report(&self.session);
                }
                if ui.button("Install Pack...").clicked() {
                    self.file_manager.open_install_pack(None);
                }
                ui.separator();
                if ui.button("Settings... (Ctrl+,)").clicked() {
                    self.settings.show_window();
                }
            });
        });
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();

        // Ctrl + , -> Settings (docs/keyboard-shortcuts.md)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Comma)) {
            self.settings.show_window();
        }

        // Ctrl+Shift+S -> Save Live Set As... (must precede Ctrl+S!)
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::S)) {
            let name = self.session.project().name.clone();
            self.file_manager
                .open_save_as(self.project_path.as_deref(), &name);
        } else if ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::S))
        {
            // Ctrl+S -> Save
            self.save_current_set();
        }

        // Ctrl+T -> Insert Audio Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            self.insert_audio_track();
        }

        // Ctrl+Shift+T -> Insert MIDI Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            self.insert_midi_track();
        }

        // Ctrl+Z -> Undo
        if !typing
            && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z))
            && self.session.undo()
        {
            self.status_message = "Undo".to_string();
        }

        // Ctrl+Y -> Redo
        if !typing && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Y)) {
            match self.session.redo() {
                Ok(true) => self.status_message = "Redo".to_string(),
                Ok(false) => {}
                Err(error) => self.status_message = format!("Redo failed: {error}"),
            }
        }

        if typing {
            return;
        }

        // Space toggles playback from the insert marker; Shift + Space continues from where playback
        // stopped (manual 6.3, page 169).
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            let shift = ctx.input(|i| i.modifiers.shift);
            if self.session.project().transport.is_playing {
                self.session.stop();
            } else if shift {
                self.session.continue_play();
            } else {
                self.session.play();
            }
        }

        // Tab -> Toggle Session / Arrangement View
        if ctx.input(|i| !i.modifiers.shift && i.key_pressed(Key::Tab)) {
            self.view_mode = match self.view_mode {
                ViewMode::Session => ViewMode::Arrangement,
                ViewMode::Arrangement => ViewMode::Session,
            };
        }

        // Shift+Tab or F12 -> Toggle Device View vs Clip View (Chapter 42.1)
        if ctx.input(|i| (i.modifiers.shift && i.key_pressed(Key::Tab)) || i.key_pressed(Key::F12))
        {
            self.bottom_tab = if self.bottom_tab == 0 { 1 } else { 0 };
        }

        // 0 -> Toggle Mute / Track Activator on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::Num0))
            && let Some(trk) = self.session.project().track(self.selected_track)
        {
            let (id, muted, name) = (trk.id, !trk.is_muted, trk.name.clone());
            self.apply(vec![Box::new(SetTrackMuted::new(id, muted))]);
            self.status_message = format!("Track {} muted: {}", name, muted);
        }

        // S -> Toggle Solo on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::S))
            && let Some(trk) = self.session.project().track(self.selected_track)
        {
            let (id, soloed, name) = (trk.id, !trk.is_soloed, trk.name.clone());
            self.apply(vec![Box::new(SetTrackSoloed::new(id, soloed))]);
            self.status_message = format!("Track {} soloed: {}", name, soloed);
        }

        // C -> Toggle Arm on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::C))
            && let Some(trk) = self.session.project().track(self.selected_track)
        {
            let (id, armed, name) = (trk.id, !trk.is_armed, trk.name.clone());
            self.apply(vec![Box::new(SetTrackArmed::new(id, armed))]);
            self.status_message = format!("Track {} armed: {}", name, armed);
        }

        // F8 -> Toggle Dark / Light Theme
        if ctx.input(|i| i.key_pressed(Key::F8)) {
            self.palette = if self.palette.is_dark() {
                LivePalette::light()
            } else {
                LivePalette::dark()
            };
            self.status_message = format!(
                "Theme changed to {}",
                if self.palette.is_dark() {
                    "Dark"
                } else {
                    "Light"
                }
            );
        }
    }

    /// Renders one complete UI frame into `ui` (used by `eframe::App::ui` and headless tests).
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        self.frame_counter += 1;
        if let Some(path) = self.screenshot_path.clone() {
            ctx.request_repaint();
            if self.frame_counter == 5 {
                ctx.request_screenshot(move |image| save_screenshot_and_exit(&path, &image));
            }
            if self.frame_counter > 50 {
                log::error!("Screenshot timeout");
                std::process::exit(1);
            }
        }

        apply_live_style(ctx, &self.palette);
        self.handle_keyboard_shortcuts(ctx);
        self.handle_dropped_files(ctx);

        // Continuous repaint during audio playback
        if self.session.project().transport.is_playing {
            ctx.request_repaint();
        }

        // A lost audio device moves the Set to a silent driver, and the session retries the device
        // (M06 item 8). Repaint while it is lost, so the retries run without input.
        self.session.poll_audio();
        self.session.poll_background();
        if self.session.audio_error().is_some() {
            ctx.request_repaint_after(livecraft_session::AUDIO_RETRY);
        }

        if let Some(message) = self.settings.ui(ctx, &mut self.session) {
            self.status_message = message;
        }

        let fm_action = self.file_manager.ui(ctx, &mut self.session);
        if let Some(msg) = fm_action.status_message {
            self.status_message = msg;
        }
        if let Some(new_path) = fm_action.updated_project_path {
            self.project_path = Some(new_path);
        }
        if fm_action.files_modified_on_disk
            || fm_action.analysis_invalidated
            || fm_action.replaced_clip_sample.is_some()
        {
            self.browser.invalidate();
            self.sample_analysis_cache = None;
        }
        if let Some(set_path) = fm_action.open_set_path {
            self.open_set_from_path(&set_path);
        }

        // The engine's playhead, meters, and active clips, read without blocking the audio thread
        let status = self.session.status();
        let playhead_beats = status.playhead_beats;

        // Top Menu Bar
        egui::Panel::top("menu_bar")
            .exact_size(22.0)
            .show(ui, |ui| {
                self.render_menu_bar(ui);
            });

        // Top Transport Panel
        egui::Panel::top("transport_bar")
            .exact_size(34.0)
            .show(ui, |ui| {
                let t_resp = render_transport_bar(
                    ui,
                    self.session.project(),
                    self.view_mode,
                    &self.palette,
                    2.5, // CPU percent
                    status.disk_load,
                    status.disk_underruns,
                    self.browser_visible,
                );

                self.apply(t_resp.edits);
                if let Some(beats) = t_resp.playhead_set {
                    self.session.set_playhead(beats);
                }
                if t_resp.play_toggled {
                    let playing = !self.session.project().transport.is_playing;
                    self.session.set_playing(playing);
                }
                if t_resp.stop_clicked {
                    // Stop, and a second press while stopped returns the insert marker to the start.
                    self.session.stop();
                }
                if t_resp.record_toggled {
                    let recording = !self.session.project().transport.is_recording;
                    self.session.set_recording(recording);
                }
                if t_resp.browser_toggled {
                    self.browser_visible = !self.browser_visible;
                }
                if let Some(mode) = t_resp.view_mode_changed {
                    self.view_mode = mode;
                }
            });

        // Bottom Status Bar (Info View & Status, Manual p. 36)
        egui::Panel::bottom("status_bar")
            .exact_size(22.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    // Ableton Info View toggle button '?'
                    let _info_btn = crate::widgets::live_toggle_button(
                        ui,
                        false,
                        self.palette.amber,
                        "?",
                        Vec2::new(18.0, 18.0),
                    );

                    let missing_count = self.session.missing_files().len();
                    if missing_count > 0 {
                        let label = format!("Media files are missing ({missing_count}) — Locate");
                        let btn = egui::Button::new(
                            egui::RichText::new(label)
                                .size(10.5)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(self.palette.amber)
                        .corner_radius(egui::CornerRadius::same(2));
                        if ui.add(btn).clicked() {
                            self.file_manager
                                .open_manage_files(FileManagerTab::MissingFiles);
                        }
                    }

                    ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(10.5)
                            .color(Color32::from_rgb(0xa0, 0xa0, 0xa0)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (engine_text, color) =
                            match (self.session.audio_error(), status.sample_rate) {
                                (Some(error), _) => (
                                    format!("Audio device unavailable: {error}"),
                                    self.palette.amber,
                                ),
                                (None, Some(rate)) => (
                                    format!("{:.1} kHz", rate / 1000.0),
                                    Color32::from_rgb(0x70, 0x70, 0x70),
                                ),
                                (None, None) => (
                                    "No audio device".to_string(),
                                    Color32::from_rgb(0x70, 0x70, 0x70),
                                ),
                            };
                        ui.label(egui::RichText::new(engine_text).size(10.0).color(color));
                    });
                });
            });

        // Bottom Detail Panel: Device Chain or Piano Roll (Manual p. 50, 445)
        let mut detail_edits: Edits = Vec::new();
        let mut save_adv_req = None;
        let mut save_adg_req = None;
        let mut export_midi_req = None;
        let mut save_alc_req = None;
        let mut replace_sample_req = None;
        let mut refresh_sample_req = None;

        egui::Panel::bottom("detail_panel")
            .exact_size(185.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                    // Authentic Ableton Detail View Tabs
                    let dev_btn = crate::widgets::live_toggle_button(
                        ui,
                        self.bottom_tab == 0,
                        self.palette.amber,
                        "Device View",
                        Vec2::new(88.0, 22.0),
                    );
                    if dev_btn.clicked() {
                        self.bottom_tab = 0;
                    }

                    let clip_btn = crate::widgets::live_toggle_button(
                        ui,
                        self.bottom_tab == 1,
                        self.palette.amber,
                        "Clip View",
                        Vec2::new(76.0, 22.0),
                    );
                    if clip_btn.clicked() {
                        self.bottom_tab = 1;
                    }

                    if let Some(trk) = self.session.project().track(self.selected_track) {
                        crate::widgets::live_divider(ui, 20.0);
                        let [r, g, b, _] = livecraft_model::color::parse_hex_color(&trk.color);
                        ui.label(
                            egui::RichText::new(format!("● {}", trk.name))
                                .strong()
                                .size(11.0)
                                .color(Color32::from_rgb(r, g, b)),
                        );
                    }
                });

                ui.add_space(2.0);

                if self.bottom_tab == 0 {
                    // Device Chain View
                    if let Some(trk) = self.session.project().track(self.selected_track) {
                        let dev_action =
                            render_device_chain(ui, trk, &self.palette, &mut detail_edits);
                        save_adv_req = dev_action.save_device_preset;
                        save_adg_req = dev_action.save_rack_preset;
                    }
                } else if let Some((t_id, s_idx)) = self.selected_clip {
                    // Clip View / Piano Roll / Audio Clip View
                    self.refresh_selected_clip_analysis(t_id, s_idx);
                    let project = self.session.project();
                    let slot_clip = project
                        .track(t_id)
                        .and_then(|trk| trk.clip_slots.get(s_idx))
                        .and_then(|slot| slot.clip.as_ref());
                    if let Some(clip) = slot_clip {
                        let cached_analysis = self
                            .sample_analysis_cache
                            .as_ref()
                            .filter(|entry| entry.key.clip_id == clip.id)
                            .and_then(|entry| entry.analysis.as_ref());
                        let pr_action = render_piano_roll(
                            ui,
                            clip,
                            &project.scale,
                            playhead_beats,
                            &self.palette,
                            cached_analysis,
                            &mut detail_edits,
                        );
                        export_midi_req = pr_action.export_midi;
                        save_alc_req = pr_action.save_live_clip;
                        replace_sample_req = pr_action.replace_sample_requested;
                        refresh_sample_req = pr_action.refresh_sample_requested;
                    } else {
                        ui.label("Empty slot. Double-click in Session View to create a clip.");
                    }
                } else {
                    ui.label("Select a clip to view notes");
                }
            });
        self.apply(detail_edits);
        if let Some((tid, did, dname)) = save_adv_req {
            self.file_manager.open_save_adv(tid, did, &dname);
        }
        if let Some(tid) = save_adg_req {
            let rack_name = self
                .session
                .project()
                .track(tid)
                .map_or("Rack", |t| t.name.as_str())
                .to_owned();
            self.file_manager.open_save_adg(tid, &rack_name);
        }
        if let Some(cid) = export_midi_req {
            self.open_export_midi_prompt(Some(cid));
        }
        if let Some(cid) = save_alc_req {
            self.open_export_alc_prompt(Some(cid));
        }
        if let Some(cid) = replace_sample_req {
            self.file_manager.open_replace_sample(cid);
        }
        if let Some(cid) = refresh_sample_req {
            match self.session.refresh_clip_sample(cid) {
                Ok(()) => {
                    self.sample_analysis_cache = None;
                    self.status_message = format!("Reloaded sample for clip #{}", cid.0);
                }
                Err(err) => {
                    self.status_message = format!("Reload sample failed: {err}");
                }
            }
        }

        // Left Browser Sidebar (toggleable)
        if self.browser_visible {
            let mut browser_action = crate::browser::BrowserAction::default();
            let project_context = self.session.project_context().clone();
            egui::Panel::left("browser_panel")
                .exact_size(180.0)
                .resizable(true)
                .show(ui, |ui| {
                    browser_action = self.browser.render(ui, &self.palette, &project_context);
                });

            if let Some((device_type, name)) = browser_action.add_device {
                let command: Box<dyn Command> =
                    Box::new(InsertDevice::new(self.selected_track, device_type, name));
                self.apply(vec![command]);
                self.status_message = "Device added to track".to_string();
            }
            if let Some(alc_path) = browser_action.import_alc {
                self.import_alc_path(&alc_path);
            }
            if let Some(adv_path) = browser_action.load_adv {
                self.load_adv_path(&adv_path);
            }
            if let Some(adg_path) = browser_action.load_adg {
                self.load_adg_path(&adg_path);
            }
            if let Some(mid_path) = browser_action.import_mid {
                self.import_mid_path(&mid_path);
            }
        }

        // Center Work Area: Session View or Arrangement View
        egui::CentralPanel::default().show(ui, |ui| {
            match self.view_mode {
                ViewMode::Session => {
                    let sess_action = render_session_view(
                        ui,
                        self.session.project(),
                        self.selected_track,
                        &status.active_slots,
                        &status.track_meters,
                        &status.master_meter,
                        playhead_beats,
                        &self.palette,
                    );

                    self.apply(sess_action.edits);
                    if let Some((t, s)) = sess_action.trigger_clip {
                        self.session.trigger_clip(t, s);
                    }
                    if let Some(t) = sess_action.stop_clip {
                        self.session.stop_clip(t);
                    }
                    if let Some(sc) = sess_action.trigger_scene {
                        self.session.trigger_scene(sc);
                    }
                    if sess_action.stop_all_clips {
                        self.session.stop_all();
                    }
                    if let Some(t) = sess_action.select_track {
                        self.selected_track = t;
                    }
                    if let Some(c) = sess_action.select_clip {
                        self.selected_clip = Some(c);
                        self.bottom_tab = 1; // Switch to clip view
                    }
                }
                ViewMode::Arrangement => {
                    let arr_action = render_arrangement_view(
                        ui,
                        self.session.project(),
                        self.selected_track,
                        playhead_beats,
                        &self.palette,
                    );

                    if let Some(beats) = arr_action.set_playhead {
                        self.session.set_playhead(beats);
                    }
                    if let Some(t) = arr_action.select_track {
                        self.selected_track = t;
                    }
                    if let Some(c) = arr_action.select_clip {
                        self.status_message = format!("Selected arrangement clip #{}", c);
                    }
                }
            }
        });
    }
}

impl eframe::App for LiveCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui);
    }
}

/// Writes an egui screenshot to `path` as PNG and exits the process (0 on success, 1 on failure).
fn save_screenshot_and_exit(path: &std::path::Path, image: &egui::ColorImage) -> ! {
    let [width, height] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match image::save_buffer(
        path,
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    ) {
        Ok(()) => {
            println!("Screenshot successfully saved to: {}", path.display());
            std::process::exit(0);
        }
        Err(e) => {
            log::error!("Failed to save screenshot: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "livecraft_ui_app_test_{}_{}_{}",
                label,
                std::process::id(),
                id
            ));
            std::fs::create_dir_all(&dir).expect("create temp test dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const SAMPLE_ALS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10" Creator="LiveCraft Test" Revision="rev">
	<LiveSet>
		<NextPointeeId Value="5000" />
		<LomId Value="0" />
		<CustomUnknownRootTag KeepMe="true" />
		<Tracks>
			<MidiTrack Id="7">
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="Lead" />
					<UserName Value="" />
				</Name>
				<Color Value="2" />
				<CustomTrackUnknown Keep="1" />
				<DeviceChain>
					<Mixer>
						<Speaker><Manual Value="true" /></Speaker>
						<SoloSink Value="false" />
						<Pan><Manual Value="0" /></Pan>
						<Volume><Manual Value="1" /></Volume>
					</Mixer>
					<MainSequencer><ClipSlotList /></MainSequencer>
					<DeviceChain><Devices /></DeviceChain>
				</DeviceChain>
			</MidiTrack>
		</Tracks>
		<MainTrack Id="8">
			<Name><EffectiveName Value="Main" /></Name>
		</MainTrack>
	</LiveSet>
</Ableton>"#;

    #[test]
    fn new_with_options_opens_als_file_and_ctrl_s_preserves_unknown_xml() {
        let dir = TempDirGuard::new("open_als_ctrl_s");
        let als_path = dir.path().join("fixture.als");
        std::fs::write(&als_path, SAMPLE_ALS_XML).expect("write fixture .als");

        let mut app = LiveCraftApp::new_with_options(crate::GuiOptions {
            project_path: Some(als_path.clone()),
            ..Default::default()
        });

        assert_eq!(app.project_path.as_deref(), Some(als_path.as_path()));
        assert!(app.session.als_document().is_some());
        assert_eq!(app.selected_track, TrackId(7));

        assert!(!app.session.project().tracks[0].is_muted);
        app.apply(vec![Box::new(SetTrackMuted::new(TrackId(7), true))]);
        assert!(
            app.session.project().tracks[0].is_muted,
            "expected mute edit to succeed, status: {}",
            app.status_message
        );

        let ctx = egui::Context::default();
        let raw_input = egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
                egui::Event::Key {
                    key: Key::S,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            ..Default::default()
        };
        ctx.begin_pass(raw_input);
        app.handle_keyboard_shortcuts(&ctx);
        let mut output = ctx.end_pass();
        output.textures_delta.clear();

        assert_eq!(
            app.status_message,
            format!("Saved to {}", als_path.display())
        );

        let reopened = Session::open_file(&als_path).expect("reopen saved .als");
        assert!(reopened.project().tracks[0].is_muted);
        let saved_xml = reopened
            .als_document()
            .expect("reopened session has als_document")
            .to_xml()
            .expect("serialize reopened als_document");
        assert!(saved_xml.contains(r#"<CustomUnknownRootTag KeepMe="true" />"#));
        assert!(saved_xml.contains(r#"<CustomTrackUnknown Keep="1" />"#));
    }

    #[test]
    fn new_with_options_falls_back_to_demo_set_when_path_none_or_invalid() {
        let app_none = LiveCraftApp::new_with_options(crate::GuiOptions::default());
        assert!(app_none.project_path.is_none());
        assert!(app_none.session.als_document().is_none());
        assert_eq!(
            app_none.selected_track,
            app_none
                .session
                .project()
                .tracks
                .first()
                .map_or(TrackId(0), |track| track.id)
        );

        let dir = TempDirGuard::new("fallback_invalid");
        let missing = dir.path().join("missing.als");
        let app_missing = LiveCraftApp::new_with_options(crate::GuiOptions {
            project_path: Some(missing),
            ..Default::default()
        });
        assert!(app_missing.project_path.is_none());
        assert!(app_missing.session.als_document().is_none());
        assert_eq!(
            app_missing.selected_track,
            app_missing
                .session
                .project()
                .tracks
                .first()
                .map_or(TrackId(0), |track| track.id)
        );
    }
}
