//! M10 Project & File Management dialogs (manual 5.2–5.8, B11 Project Health Report).

use eframe::egui::{self, Color32};
use livecraft_model::ids::{ClipId, DeviceId, TrackId};
use livecraft_session::{
    AutoRepairReport, AutomaticSearchOptions, ClipLocationRef, CollectAllOptions,
    CollectFilesOnExport, CollectReport, ExportClipOutcome, LibraryPreferences, MissingFile,
    PackInstallReport, ProjectHealthReport, SampleLocation, SearchCandidateOutcome, Session,
    UnusedFileCategory, UnusedFileEntry, UnusedFilesReport,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Tabs in the Manage Project / File Manager window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileManagerTab {
    #[default]
    MissingFiles,
    ExternalFiles,
    UnusedFiles,
}

/// Kind of path-prompt dialog currently open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathPromptKind {
    ImportMidi {
        track_id: Option<TrackId>,
    },
    ExportMidi {
        clip_id: ClipId,
        clip_name: String,
    },
    ImportAlc {
        track_id: Option<TrackId>,
    },
    ExportAlc {
        track_id: TrackId,
        clip_id: ClipId,
        clip_name: String,
    },
    SaveAdv {
        track_id: TrackId,
        device_id: DeviceId,
        device_name: String,
    },
    LoadAdv {
        track_id: TrackId,
    },
    SaveAdg {
        track_id: TrackId,
    },
    LoadAdg {
        track_id: TrackId,
    },
    ReplaceSample {
        clip_id: ClipId,
    },
}

/// State for a single path-prompt modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPromptState {
    pub kind: PathPromptKind,
    pub path: String,
    pub rack_name: String,
    pub create_new_track: bool,
    pub error: Option<String>,
}

/// Pending `.alc` export awaiting user confirmation under `CollectFilesOnExport::Ask` (manual 5.7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAlcExport {
    pub track_id: TrackId,
    pub clip_id: ClipId,
    pub clip_name: String,
    pub out_path: PathBuf,
}

/// Outcomes returned from [`FileManagerWindow::ui`] for the main app to react to.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FileManagerAction {
    pub status_message: Option<String>,
    pub updated_project_path: Option<PathBuf>,
    pub open_set_path: Option<PathBuf>,
    pub files_modified_on_disk: bool,
    pub replaced_clip_sample: Option<ClipId>,
    pub analysis_invalidated: bool,
}

/// Manages all M10 project/file management dialogs and flows.
#[derive(Debug, Clone)]
pub struct FileManagerWindow {
    pub open: bool,
    pub tab: FileManagerTab,

    // Missing Files (manual 5.6)
    pub manual_paths: BTreeMap<String, String>,
    pub search_folder: String,
    pub search_project: bool,
    pub search_library: bool,
    pub last_auto_report: Option<AutoRepairReport>,
    pub missing_error: Option<String>,

    // External Files / Collect All and Save (manual 5.7)
    pub collect_options: CollectAllOptions,
    pub last_collect_report: Option<CollectReport>,
    pub collect_error: Option<String>,

    // Library preferences editor inside External Files
    user_library_input: String,
    factory_packs_input: String,
    collect_on_export: CollectFilesOnExport,
    create_analysis_files: bool,
    analysis_cache_folder_input: String,
    library_prefs_synced: bool,
    library_error: Option<String>,

    // Unused Files (manual 5.8)
    pub last_unused_report: Option<UnusedFilesReport>,
    pub pending_delete_unused: Option<PathBuf>,
    pub unused_error: Option<String>,

    // Save Live Set As... (manual 5.5)
    pub save_as_open: bool,
    pub save_as_path: String,
    pub save_as_error: Option<String>,

    // Project Health Report (B11)
    pub health_open: bool,
    pub health_report: Option<ProjectHealthReport>,
    pub health_json_preview: String,
    pub health_show_json: bool,
    pub health_export_path: String,
    pub health_expected_sr: String,
    pub health_error: Option<String>,

    // Install Pack (.alp)
    pub install_pack_open: bool,
    pub pack_path: String,
    pub pack_dest_dir: String,
    pub pack_report: Option<PackInstallReport>,
    pub pack_error: Option<String>,

    // Path prompt & Ask confirmation
    pub prompt: Option<PathPromptState>,
    pub ask_copy_on_export: Option<PendingAlcExport>,
}

impl Default for FileManagerWindow {
    fn default() -> Self {
        Self {
            open: false,
            tab: FileManagerTab::MissingFiles,
            manual_paths: BTreeMap::new(),
            search_folder: String::new(),
            search_project: true,
            search_library: true,
            last_auto_report: None,
            missing_error: None,
            collect_options: CollectAllOptions::default(),
            last_collect_report: None,
            collect_error: None,
            user_library_input: String::new(),
            factory_packs_input: String::new(),
            collect_on_export: CollectFilesOnExport::default(),
            create_analysis_files: true,
            analysis_cache_folder_input: String::new(),
            library_prefs_synced: false,
            library_error: None,
            last_unused_report: None,
            pending_delete_unused: None,
            unused_error: None,
            save_as_open: false,
            save_as_path: String::new(),
            save_as_error: None,
            health_open: false,
            health_report: None,
            health_json_preview: String::new(),
            health_show_json: true,
            health_export_path: String::from("health_report.json"),
            health_expected_sr: String::new(),
            health_error: None,
            install_pack_open: false,
            pack_path: String::new(),
            pack_dest_dir: String::new(),
            pack_report: None,
            pack_error: None,
            prompt: None,
            ask_copy_on_export: None,
        }
    }
}

impl FileManagerWindow {
    /// Opens the File Manager window on `tab`.
    pub fn open_manage_files(&mut self, tab: FileManagerTab) {
        self.open = true;
        self.tab = tab;
        self.library_prefs_synced = false;
    }

    /// Opens the Save Live Set As... dialog.
    pub fn open_save_as(&mut self, current_path: Option<&Path>, set_name: &str) {
        if self.save_as_path.trim().is_empty() {
            self.save_as_path = match current_path {
                Some(path) => path.display().to_string(),
                None => format!("{}.als", sanitize_filename(set_name, "Untitled")),
            };
        }
        self.save_as_error = None;
        self.save_as_open = true;
    }

    /// Opens the Project Health Report dialog and runs an initial health check.
    pub fn open_health_report(&mut self, session: &Session) {
        self.health_open = true;
        self.refresh_health_report(session);
    }

    /// Recomputes the Project Health Report and JSON preview from `session`.
    pub fn refresh_health_report(&mut self, session: &Session) {
        let expected_sr = self.health_expected_sr.trim();
        let parsed_sr = if expected_sr.is_empty() {
            Ok(None)
        } else {
            expected_sr.parse::<u32>().map(Some).map_err(|_| {
                format!("Invalid expected sample rate '{expected_sr}' (expected integer Hz)")
            })
        };

        let expected = match parsed_sr {
            Ok(val) => val,
            Err(err) => {
                self.health_error = Some(err);
                return;
            }
        };

        match session.health_report(expected) {
            Ok(report) => {
                self.health_json_preview = health_report_to_json(&report);
                self.health_report = Some(report);
                self.health_error = None;
            }
            Err(err) => {
                self.health_error = Some(err.to_string());
            }
        }
    }

    /// Opens the Install Live Pack (`.alp`) dialog, optionally pre-populated with a path.
    pub fn open_install_pack(&mut self, initial_alp: Option<&Path>) {
        if let Some(path) = initial_alp {
            self.pack_path = path.display().to_string();
        }
        self.pack_error = None;
        self.pack_report = None;
        self.install_pack_open = true;
    }

    /// Opens the Import MIDI File (`.mid`) path prompt.
    pub fn open_import_midi(&mut self, selected_track: Option<TrackId>) {
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::ImportMidi {
                track_id: selected_track,
            },
            path: String::new(),
            rack_name: String::new(),
            create_new_track: selected_track.is_none(),
            error: None,
        });
    }

    /// Opens the Export MIDI Clip (`.mid`) path prompt.
    pub fn open_export_midi(&mut self, clip_id: ClipId, clip_name: &str) {
        let default_path = format!("{}.mid", sanitize_filename(clip_name, "clip"));
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::ExportMidi {
                clip_id,
                clip_name: clip_name.to_owned(),
            },
            path: default_path,
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Import Live Clip (`.alc`) path prompt.
    pub fn open_import_alc(&mut self, selected_track: Option<TrackId>) {
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::ImportAlc {
                track_id: selected_track,
            },
            path: String::new(),
            rack_name: String::new(),
            create_new_track: selected_track.is_none(),
            error: None,
        });
    }

    /// Opens the Export Live Clip (`.alc`) path prompt.
    pub fn open_export_alc(&mut self, track_id: TrackId, clip_id: ClipId, clip_name: &str) {
        let default_path = format!("{}.alc", sanitize_filename(clip_name, "clip"));
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::ExportAlc {
                track_id,
                clip_id,
                clip_name: clip_name.to_owned(),
            },
            path: default_path,
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Save Device Preset (`.adv`) path prompt.
    pub fn open_save_adv(&mut self, track_id: TrackId, device_id: DeviceId, device_name: &str) {
        let default_path = format!("{}.adv", sanitize_filename(device_name, "device"));
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::SaveAdv {
                track_id,
                device_id,
                device_name: device_name.to_owned(),
            },
            path: default_path,
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Load Device Preset (`.adv`) path prompt.
    pub fn open_load_adv(&mut self, track_id: TrackId) {
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::LoadAdv { track_id },
            path: String::new(),
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Save Rack Preset (`.adg`) path prompt.
    pub fn open_save_adg(&mut self, track_id: TrackId, default_name: &str) {
        let rack_name = if default_name.trim().is_empty() {
            "Rack".to_owned()
        } else {
            default_name.trim().to_owned()
        };
        let default_path = format!("{}.adg", sanitize_filename(&rack_name, "rack"));
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::SaveAdg { track_id },
            path: default_path,
            rack_name,
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Load Rack Preset (`.adg`) path prompt.
    pub fn open_load_adg(&mut self, track_id: TrackId) {
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::LoadAdg { track_id },
            path: String::new(),
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Opens the Replace Sample... path prompt for `clip_id` (manual 8.12).
    pub fn open_replace_sample(&mut self, clip_id: ClipId) {
        self.prompt = Some(PathPromptState {
            kind: PathPromptKind::ReplaceSample { clip_id },
            path: String::new(),
            rack_name: String::new(),
            create_new_track: false,
            error: None,
        });
    }

    /// Renders any open M10 windows and modals, returning actions for the app shell.
    pub fn ui(&mut self, ctx: &egui::Context, session: &mut Session) -> FileManagerAction {
        let mut action = FileManagerAction::default();
        self.render_manage_project_window(ctx, session, &mut action);
        self.render_save_as_window(ctx, session, &mut action);
        self.render_health_window(ctx, session, &mut action);
        self.render_install_pack_window(ctx, session, &mut action);
        self.render_path_prompt_window(ctx, session, &mut action);
        self.render_ask_copy_modal(ctx, session, &mut action);
        action
    }

    fn render_manage_project_window(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Manage Project")
            .open(&mut open)
            .default_width(660.0)
            .default_height(560.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let missing_label =
                        format!("Missing Files ({})", session.missing_files().len());
                    ui.selectable_value(&mut self.tab, FileManagerTab::MissingFiles, missing_label);
                    ui.selectable_value(
                        &mut self.tab,
                        FileManagerTab::ExternalFiles,
                        "External Files",
                    );
                    ui.selectable_value(&mut self.tab, FileManagerTab::UnusedFiles, "Unused Files");
                });
                ui.separator();

                match self.tab {
                    FileManagerTab::MissingFiles => {
                        self.render_missing_files_tab(ui, session, action)
                    }
                    FileManagerTab::ExternalFiles => {
                        self.render_external_files_tab(ui, session, action);
                    }
                    FileManagerTab::UnusedFiles => {
                        self.render_unused_files_tab(ui, session, action)
                    }
                }
            });
        self.open = open;
    }

    fn render_missing_files_tab(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        let missing = session.missing_files().to_vec();
        if missing.is_empty() {
            ui.colored_label(
                Color32::from_rgb(0x45, 0xd4, 0x70),
                "No missing media files detected.",
            );
        } else {
            ui.colored_label(
                Color32::from_rgb(0xff, 0xad, 0x56),
                format!("{} missing media file(s):", missing.len()),
            );
        }

        if let Some(err) = &self.missing_error {
            ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
        }

        let mut manual_replace_req: Option<(String, String, String)> = None;
        if !missing.is_empty() {
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("missing_files_list")
                .max_height(155.0)
                .show(ui, |ui| {
                    for (idx, item) in missing.iter().enumerate() {
                        let key = item.reference.display_ref().to_owned();
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&item.file_name);
                                ui.label(format!("({} usage(s))", item.usages.len()));
                                ui.label(
                                    egui::RichText::new(format!("Ref: {key}"))
                                        .size(10.5)
                                        .monospace(),
                                );
                            });
                            if let Some(rel) = &item.reference.relative_path
                                && rel != &key
                            {
                                ui.label(
                                    egui::RichText::new(format!("Relative: {rel}"))
                                        .size(10.0)
                                        .monospace(),
                                );
                            }
                            ui.horizontal(|ui| {
                                let buf = self.manual_paths.entry(key.clone()).or_default();
                                ui.add(
                                    egui::TextEdit::singleline(buf)
                                        .id_salt(("manual_replace_input", idx, &key))
                                        .hint_text("Replacement file path...")
                                        .desired_width(380.0),
                                );
                                if ui.button("Replace").clicked() {
                                    manual_replace_req =
                                        Some((key.clone(), item.file_name.clone(), buf.clone()));
                                }
                            });
                        });
                    }
                });
        }

        if let Some((missing_ref, file_name, replacement)) = manual_replace_req {
            self.apply_manual_repair(
                session,
                &missing_ref,
                &file_name,
                Path::new(replacement.trim()),
                action,
            );
        }

        ui.add_space(6.0);
        egui::CollapsingHeader::new("Automatic Search")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search Folder:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_folder)
                            .hint_text("Optional folder path to search...")
                            .desired_width(340.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.search_project, "Search Project");
                    ui.checkbox(&mut self.search_library, "Search Library");
                    if ui.button("Go").clicked() {
                        let search_folder = (!self.search_folder.trim().is_empty())
                            .then(|| PathBuf::from(self.search_folder.trim()));
                        let opts = AutomaticSearchOptions {
                            search_folder,
                            search_project: self.search_project,
                            search_library: self.search_library,
                            library_roots: Vec::new(),
                        };
                        match session.locate_missing_auto(&opts) {
                            Ok(report) => {
                                let msg = format!(
                                    "Automatic search: {} repaired, {} unresolved",
                                    report.repaired.len(),
                                    report.unresolved.len()
                                );
                                if !report.repaired.is_empty() {
                                    action.analysis_invalidated = true;
                                }
                                self.last_auto_report = Some(report);
                                self.missing_error = None;
                                action.status_message = Some(msg);
                            }
                            Err(err) => {
                                let msg = format!("Automatic search failed: {err}");
                                self.missing_error = Some(msg.clone());
                                action.status_message = Some(msg);
                            }
                        }
                    }
                });

                let mut candidate_pick: Option<(String, String, PathBuf)> = None;
                if let Some(report) = &self.last_auto_report {
                    ui.separator();
                    ui.label(format!(
                        "Last Search: {} auto-repaired, {} unresolved",
                        report.repaired.len(),
                        report.unresolved.len()
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("auto_search_results")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for (ref_str, found) in &report.repaired {
                                ui.colored_label(
                                    Color32::from_rgb(0x45, 0xd4, 0x70),
                                    format!("Repaired: {ref_str} -> {}", found.display()),
                                );
                            }
                            for (item, outcome) in &report.unresolved {
                                let ref_str = item.reference.display_ref().to_owned();
                                match outcome {
                                    SearchCandidateOutcome::NoCandidate => {
                                        ui.colored_label(
                                            Color32::from_rgb(0xff, 0x77, 0x77),
                                            format!("NoCandidate: {} ({ref_str})", item.file_name),
                                        );
                                    }
                                    SearchCandidateOutcome::MultipleCandidates(candidates) => {
                                        ui.group(|ui| {
                                            ui.colored_label(
                                                Color32::from_rgb(0xff, 0xad, 0x56),
                                                format!(
                                                    "MultipleCandidates: {} ({} found)",
                                                    item.file_name,
                                                    candidates.len()
                                                ),
                                            );
                                            for cand in candidates {
                                                let btn_label =
                                                    format!("Use candidate: {}", cand.display());
                                                if ui.button(btn_label).clicked() {
                                                    candidate_pick = Some((
                                                        ref_str.clone(),
                                                        item.file_name.clone(),
                                                        cand.clone(),
                                                    ));
                                                }
                                            }
                                        });
                                    }
                                    SearchCandidateOutcome::OneCandidate(cand) => {
                                        let btn_label =
                                            format!("Use candidate: {}", cand.display());
                                        if ui.button(btn_label).clicked() {
                                            candidate_pick = Some((
                                                ref_str.clone(),
                                                item.file_name.clone(),
                                                cand.clone(),
                                            ));
                                        }
                                    }
                                }
                            }
                        });
                }

                if let Some((missing_ref, file_name, cand)) = candidate_pick {
                    self.apply_manual_repair(session, &missing_ref, &file_name, &cand, action);
                }
            });
    }

    fn apply_manual_repair(
        &mut self,
        session: &mut Session,
        missing_ref: &str,
        file_name: &str,
        replacement: &Path,
        action: &mut FileManagerAction,
    ) {
        match session.locate_missing_manual(missing_ref, replacement) {
            Ok(count) => {
                self.manual_paths.remove(missing_ref);
                self.missing_error = None;
                if let Some(report) = &mut self.last_auto_report {
                    let before_len = report.unresolved.len();
                    report
                        .unresolved
                        .retain(|(m, _)| m.reference.display_ref() != missing_ref);
                    if report.unresolved.len() < before_len {
                        report
                            .repaired
                            .push((missing_ref.to_owned(), replacement.to_path_buf()));
                    }
                }
                action.analysis_invalidated = true;
                action.status_message = Some(format!(
                    "Replaced '{file_name}' ({count} reference(s)) with {}",
                    replacement.display()
                ));
            }
            Err(err) => {
                let msg = format!("Replace failed for '{file_name}': {err}");
                self.missing_error = Some(msg.clone());
                action.status_message = Some(msg);
            }
        }
    }

    fn render_external_files_tab(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        let project_root = session.project_context().project_root.clone();
        if let Some(root) = &project_root {
            ui.label(format!("Active Live Project: {}", root.display()));
        } else {
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(0xff, 0xad, 0x56),
                    "Set is not inside a Live Project folder yet.",
                );
                if ui.button("Save Live Set As...").clicked() {
                    let name = session.project().name.clone();
                    let set_path = session.set_path().map(Path::to_path_buf);
                    self.open_save_as(set_path.as_deref(), &name);
                }
            });
        }

        ui.add_space(6.0);
        ui.strong("Collect All and Save (copies samples into Samples/Imported/)");
        ui.checkbox(
            &mut self.collect_options.collect_elsewhere,
            "Collect Elsewhere",
        );
        ui.checkbox(
            &mut self.collect_options.collect_other_projects,
            "Collect Other Projects",
        );
        ui.checkbox(
            &mut self.collect_options.collect_user_library,
            "Collect User Library",
        );
        ui.checkbox(
            &mut self.collect_options.collect_factory_packs,
            "Collect Factory Packs",
        );

        ui.add_space(4.0);
        if ui.button("Collect and Save").clicked() {
            match session.collect_all_and_save(&self.collect_options) {
                Ok(report) => {
                    let msg = format!(
                        "Collected {} file(s), skipped {}, updated {} reference(s)",
                        report.collected.len(),
                        report.skipped_by_policy.len(),
                        report.updated_references
                    );
                    self.last_collect_report = Some(report);
                    self.collect_error = None;
                    action.status_message = Some(msg);
                    action.files_modified_on_disk = true;
                }
                Err(err) => {
                    let msg = format!("Collect All and Save failed: {err}");
                    self.collect_error = Some(msg.clone());
                    action.status_message = Some(msg);
                }
            }
        }

        if let Some(err) = &self.collect_error {
            ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
        }

        if let Some(report) = &self.last_collect_report {
            ui.separator();
            ui.strong("CollectReport Summary");
            ui.label(format!(
                "Collected: {} | Skipped by policy: {} | Missing: {} | Updated refs: {}",
                report.collected.len(),
                report.skipped_by_policy.len(),
                report.missing.len(),
                report.updated_references
            ));
            egui::ScrollArea::vertical()
                .id_salt("collect_report_scroll")
                .max_height(130.0)
                .show(ui, |ui| {
                    for item in &report.collected {
                        let reused_tag = if item.reused_existing {
                            " [reused]"
                        } else {
                            ""
                        };
                        ui.label(format!(
                            "Copied ({:?}): {} -> {}{reused_tag}",
                            item.location,
                            item.original_path.display(),
                            item.project_relative_path
                        ));
                    }
                    for (path, loc) in &report.skipped_by_policy {
                        ui.label(format!("Skipped ({loc:?}): {}", path.display()));
                    }
                    for missing_ref in &report.missing {
                        ui.colored_label(
                            Color32::from_rgb(0xff, 0xad, 0x56),
                            format!("Missing: {missing_ref}"),
                        );
                    }
                });
        }

        ui.add_space(6.0);
        self.render_library_preferences_section(ui, session, action);
    }

    fn render_library_preferences_section(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        if !self.library_prefs_synced {
            let lib = &session.preferences().library;
            self.user_library_input = lib
                .user_library
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            self.factory_packs_input = lib
                .factory_packs_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("; ");
            self.collect_on_export = lib.collect_files_on_export;
            self.create_analysis_files = lib.create_analysis_files;
            self.analysis_cache_folder_input = lib
                .analysis_cache_folder
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            self.library_prefs_synced = true;
        }

        egui::CollapsingHeader::new("Library & Export Preferences")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("User Library:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.user_library_input)
                            .hint_text("Default Documents/LiveCraft/User Library")
                            .desired_width(320.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Factory Packs (;-separated):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.factory_packs_input)
                            .hint_text("Optional pack directories separated by ;")
                            .desired_width(260.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Collect Files on Export (.alc):");
                    egui::ComboBox::from_id_salt("collect_files_on_export_combo")
                        .selected_text(format!("{:?}", self.collect_on_export))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.collect_on_export,
                                CollectFilesOnExport::Always,
                                "Always",
                            );
                            ui.selectable_value(
                                &mut self.collect_on_export,
                                CollectFilesOnExport::Ask,
                                "Ask",
                            );
                            ui.selectable_value(
                                &mut self.collect_on_export,
                                CollectFilesOnExport::Never,
                                "Never",
                            );
                        });
                });
                ui.checkbox(
                    &mut self.create_analysis_files,
                    "Create Analysis Files (.lcanalysis)",
                );
                ui.horizontal(|ui| {
                    ui.label("Analysis Cache Folder:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.analysis_cache_folder_input)
                            .hint_text("Blank = beside each sample")
                            .desired_width(290.0),
                    );
                });
                if ui.button("Apply Library Preferences").clicked() {
                    let user_library = (!self.user_library_input.trim().is_empty())
                        .then(|| PathBuf::from(self.user_library_input.trim()));
                    let factory_packs_roots = self
                        .factory_packs_input
                        .split(';')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect();
                    let analysis_cache_folder =
                        (!self.analysis_cache_folder_input.trim().is_empty())
                            .then(|| PathBuf::from(self.analysis_cache_folder_input.trim()));
                    let next = LibraryPreferences {
                        user_library,
                        factory_packs_roots,
                        collect_files_on_export: self.collect_on_export,
                        create_analysis_files: self.create_analysis_files,
                        analysis_cache_folder,
                    };
                    match session.set_library_preferences(next) {
                        Ok(()) => {
                            self.library_error = None;
                            action.status_message = Some("Library preferences updated".to_owned());
                            action.files_modified_on_disk = true;
                            action.analysis_invalidated = true;
                        }
                        Err(err) => {
                            let msg = format!("Failed to save library preferences: {err}");
                            self.library_error = Some(msg.clone());
                            action.status_message = Some(msg);
                        }
                    }
                }
                if let Some(err) = &self.library_error {
                    ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
                }
            });
    }

    fn render_unused_files_tab(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        ui.horizontal(|ui| {
            if ui.button("Scan Unused Files").clicked() {
                match session.find_unused_files() {
                    Ok(report) => {
                        let msg = format!("Found {} unused file(s)", report.unused_files.len());
                        self.last_unused_report = Some(report);
                        self.unused_error = None;
                        self.pending_delete_unused = None;
                        action.status_message = Some(msg);
                    }
                    Err(err) => {
                        let msg = format!("Unused scan failed: {err}");
                        self.unused_error = Some(msg.clone());
                        action.status_message = Some(msg);
                    }
                }
            }
        });

        if let Some(err) = &self.unused_error {
            ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
        }

        let Some(report) = &self.last_unused_report else {
            ui.label("Click 'Scan Unused Files' to scan the active Live Project folder.");
            return;
        };

        ui.label(format!(
            "Scanned {} document(s); {} unused file(s) found in {}",
            report.scanned_documents.len(),
            report.unused_files.len(),
            report.project_root.display()
        ));

        let mut confirmed_delete: Option<PathBuf> = None;
        let categories = [
            (UnusedFileCategory::Recorded, "Recorded"),
            (UnusedFileCategory::Imported, "Imported"),
            (UnusedFileCategory::Processed, "Processed"),
            (UnusedFileCategory::Freeze, "Freeze"),
            (UnusedFileCategory::Other, "Other"),
        ];

        egui::ScrollArea::vertical()
            .id_salt("unused_files_scroll")
            .max_height(300.0)
            .show(ui, |ui| {
                for (cat, label) in categories {
                    let entries: Vec<&UnusedFileEntry> = report
                        .unused_files
                        .iter()
                        .filter(|e| e.category == cat)
                        .collect();
                    if entries.is_empty() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.strong(format!("{label} ({})", entries.len()));
                    for entry in entries {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{} ({} bytes)",
                                entry.relative_path, entry.size_bytes
                            ));
                            if self.pending_delete_unused.as_ref() == Some(&entry.path) {
                                ui.colored_label(Color32::from_rgb(0xff, 0x77, 0x77), "Confirm?");
                                if ui.button("Confirm Delete").clicked() {
                                    confirmed_delete = Some(entry.path.clone());
                                }
                                if ui.button("Cancel").clicked() {
                                    self.pending_delete_unused = None;
                                }
                            } else if ui.button("Delete").clicked() {
                                self.pending_delete_unused = Some(entry.path.clone());
                            }
                        });
                    }
                }
            });

        if let Some(path) = confirmed_delete {
            match session.delete_unused_file(&path) {
                Ok(()) => {
                    self.pending_delete_unused = None;
                    self.unused_error = None;
                    self.last_unused_report = session.find_unused_files().ok();
                    action.files_modified_on_disk = true;
                    action.status_message =
                        Some(format!("Deleted unused file: {}", path.display()));
                }
                Err(err) => {
                    self.pending_delete_unused = None;
                    let msg = format!("Delete unused failed: {err}");
                    self.unused_error = Some(msg.clone());
                    action.status_message = Some(msg);
                }
            }
        }
    }

    fn render_save_as_window(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        if !self.save_as_open {
            return;
        }
        let mut open = self.save_as_open;
        let mut should_close = false;
        egui::Window::new("Save Live Set As...")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(
                    "Saves into an enclosing Live Project or creates '<Name> Project/' outside.",
                );
                ui.horizontal(|ui| {
                    ui.label("Target .als path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.save_as_path)
                            .hint_text("path/to/MySong.als")
                            .desired_width(340.0),
                    );
                });
                if let Some(err) = &self.save_as_error {
                    ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
                }
                ui.horizontal(|ui| {
                    if ui.button("Save Into Project").clicked() {
                        let trimmed = self.save_as_path.trim();
                        if trimmed.is_empty() {
                            self.save_as_error = Some("Enter a target .als path".to_owned());
                        } else {
                            match session.save_into_project(Path::new(trimmed)) {
                                Ok(saved) => {
                                    let kind = if saved.created_new_project {
                                        "Created project and saved"
                                    } else {
                                        "Saved into project"
                                    };
                                    action.updated_project_path = Some(saved.set_path.clone());
                                    action.files_modified_on_disk = true;
                                    action.status_message =
                                        Some(format!("{kind}: {}", saved.set_path.display()));
                                    self.save_as_error = None;
                                    should_close = true;
                                }
                                Err(err) => {
                                    let msg = format!("Save Into Project failed: {err}");
                                    self.save_as_error = Some(msg.clone());
                                    action.status_message = Some(msg);
                                }
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });
        self.save_as_open = open && !should_close;
    }

    fn render_health_window(
        &mut self,
        ctx: &egui::Context,
        session: &Session,
        action: &mut FileManagerAction,
    ) {
        if !self.health_open {
            return;
        }
        if self.health_report.is_none() && self.health_error.is_none() {
            self.refresh_health_report(session);
        }

        let mut open = self.health_open;
        let mut refresh_requested = false;
        let mut locate_missing_clicked = false;
        let mut collect_external_clicked = false;

        egui::Window::new("Project Health Report")
            .open(&mut open)
            .default_width(660.0)
            .default_height(480.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Refresh Health Check").clicked() {
                        refresh_requested = true;
                    }
                    ui.label("Expected SR (Hz):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.health_expected_sr)
                            .hint_text("Auto")
                            .desired_width(64.0),
                    );
                    if let Some(report) = &self.health_report {
                        if report.is_healthy() {
                            ui.colored_label(Color32::from_rgb(0x45, 0xd4, 0x70), "HEALTHY");
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(0xff, 0xad, 0x56),
                                format!("ISSUES FOUND ({})", report.issue_count()),
                            );
                        }
                    }
                });

                if let Some(err) = &self.health_error {
                    ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
                }

                if let Some(report) = &self.health_report {
                    ui.label(format!(
                        "Set: {} | Root: {}",
                        report.set_name,
                        report
                            .project_root
                            .as_ref()
                            .map_or_else(|| "(none)".to_owned(), |p| p.display().to_string())
                    ));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("health_report_body")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            render_health_missing_section(
                                ui,
                                &report.missing_files,
                                &mut locate_missing_clicked,
                            );
                            render_health_unused_section(ui, &report.unused_files);
                            render_health_plugins_section(ui, report);
                            render_health_sr_section(ui, report);
                            render_health_external_section(
                                ui,
                                report,
                                &mut collect_external_clicked,
                            );
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.health_show_json, "JSON Preview");
                        ui.label("Export path:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.health_export_path)
                                .hint_text("health_report.json")
                                .desired_width(240.0),
                        );
                        if ui.button("Export JSON").clicked() {
                            let target = self.health_export_path.trim();
                            if target.is_empty() {
                                self.health_error =
                                    Some("Enter an export JSON file path".to_owned());
                            } else {
                                let out_path = Path::new(target);
                                let write_res = (|| -> std::io::Result<()> {
                                    if let Some(parent) =
                                        out_path.parent().filter(|p| !p.as_os_str().is_empty())
                                    {
                                        fs::create_dir_all(parent)?;
                                    }
                                    fs::write(out_path, &self.health_json_preview)
                                })();
                                match write_res {
                                    Ok(()) => {
                                        self.health_error = None;
                                        action.status_message = Some(format!(
                                            "Exported health report JSON to {}",
                                            out_path.display()
                                        ));
                                    }
                                    Err(err) => {
                                        let msg = format!("JSON export failed: {err}");
                                        self.health_error = Some(msg.clone());
                                        action.status_message = Some(msg);
                                    }
                                }
                            }
                        }
                    });

                    if self.health_show_json {
                        egui::ScrollArea::vertical()
                            .id_salt("health_json_scroll")
                            .max_height(130.0)
                            .show(ui, |ui| {
                                let mut preview = self.health_json_preview.as_str();
                                ui.add(
                                    egui::TextEdit::multiline(&mut preview)
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(6),
                                );
                            });
                    }
                }
            });

        self.health_open = open;
        if refresh_requested {
            self.refresh_health_report(session);
        }
        if locate_missing_clicked {
            self.open_manage_files(FileManagerTab::MissingFiles);
        }
        if collect_external_clicked {
            self.open_manage_files(FileManagerTab::ExternalFiles);
        }
    }

    fn render_install_pack_window(
        &mut self,
        ctx: &egui::Context,
        session: &Session,
        action: &mut FileManagerAction,
    ) {
        if !self.install_pack_open {
            return;
        }
        let mut open = self.install_pack_open;
        let mut should_close = false;
        let mut open_set: Option<PathBuf> = None;

        egui::Window::new("Install Live Pack (.alp)")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Pack (.alp) path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pack_path)
                            .hint_text("path/to/Pack.alp")
                            .desired_width(340.0),
                    );
                });
                let default_lib = session
                    .preferences()
                    .library
                    .effective_user_library()
                    .map_or_else(|| "User Library".to_owned(), |p| p.display().to_string());
                ui.horizontal(|ui| {
                    ui.label("Destination dir:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pack_dest_dir)
                            .hint_text(format!("Blank = {default_lib}"))
                            .desired_width(340.0),
                    );
                });

                if let Some(err) = &self.pack_error {
                    ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
                }

                ui.horizontal(|ui| {
                    if ui.button("Install Pack").clicked() {
                        let alp_trimmed = self.pack_path.trim();
                        if alp_trimmed.is_empty() {
                            self.pack_error = Some("Enter a .alp Pack path".to_owned());
                        } else {
                            let dest = (!self.pack_dest_dir.trim().is_empty())
                                .then(|| PathBuf::from(self.pack_dest_dir.trim()));
                            match session.install_pack(Path::new(alp_trimmed), dest.as_deref()) {
                                Ok(report) => {
                                    let msg = format!(
                                        "Installed Pack: {} file(s) ({} bytes)",
                                        report.extracted_files.len(),
                                        report.total_bytes
                                    );
                                    self.pack_report = Some(report);
                                    self.pack_error = None;
                                    action.files_modified_on_disk = true;
                                    action.status_message = Some(msg);
                                }
                                Err(err) => {
                                    let msg = format!("Pack installation failed: {err}");
                                    self.pack_error = Some(msg.clone());
                                    action.status_message = Some(msg);
                                }
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });

                if let Some(report) = &self.pack_report {
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(0x45, 0xd4, 0x70),
                        format!(
                            "Extracted {} file(s) ({} bytes), {} Set(s) found",
                            report.extracted_files.len(),
                            report.total_bytes,
                            report.sets_found.len()
                        ),
                    );
                    for als in &report.sets_found {
                        ui.horizontal(|ui| {
                            ui.label(als.display().to_string());
                            if ui.button("Open Set").clicked() {
                                open_set = Some(als.clone());
                                should_close = true;
                            }
                        });
                    }
                }
            });

        self.install_pack_open = open && !should_close;
        if let Some(set_path) = open_set {
            action.open_set_path = Some(set_path);
        }
    }

    fn render_path_prompt_window(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        action: &mut FileManagerAction,
    ) {
        let Some(mut state) = self.prompt.take() else {
            return;
        };
        let title = match &state.kind {
            PathPromptKind::ImportMidi { .. } => "Import MIDI File (.mid)",
            PathPromptKind::ExportMidi { .. } => "Export MIDI Clip (.mid)",
            PathPromptKind::ImportAlc { .. } => "Import Live Clip (.alc)",
            PathPromptKind::ExportAlc { .. } => "Export Live Clip (.alc)",
            PathPromptKind::SaveAdv { .. } => "Save Device Preset (.adv)",
            PathPromptKind::LoadAdv { .. } => "Load Device Preset (.adv)",
            PathPromptKind::SaveAdg { .. } => "Save Rack Preset (.adg)",
            PathPromptKind::LoadAdg { .. } => "Load Rack Preset (.adg)",
            PathPromptKind::ReplaceSample { .. } => "Replace Sample...",
        };
        let submit_label = match &state.kind {
            PathPromptKind::ImportMidi { .. } | PathPromptKind::ImportAlc { .. } => "Import",
            PathPromptKind::ExportMidi { .. } | PathPromptKind::ExportAlc { .. } => "Export",
            PathPromptKind::SaveAdv { .. } | PathPromptKind::SaveAdg { .. } => "Save",
            PathPromptKind::LoadAdv { .. } | PathPromptKind::LoadAdg { .. } => "Load",
            PathPromptKind::ReplaceSample { .. } => "Replace",
        };

        let mut open = true;
        let mut keep_open = true;

        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                match &state.kind {
                    PathPromptKind::ExportMidi { clip_name, .. }
                    | PathPromptKind::ExportAlc { clip_name, .. } => {
                        ui.label(format!("Clip: {clip_name}"));
                    }
                    PathPromptKind::SaveAdv { device_name, .. } => {
                        ui.label(format!("Device: {device_name}"));
                    }
                    PathPromptKind::SaveAdg { .. } => {
                        ui.horizontal(|ui| {
                            ui.label("Rack Name:");
                            ui.add(
                                egui::TextEdit::singleline(&mut state.rack_name)
                                    .desired_width(300.0),
                            );
                        });
                    }
                    PathPromptKind::ImportMidi { .. } | PathPromptKind::ImportAlc { .. } => {
                        ui.checkbox(
                            &mut state.create_new_track,
                            "Import onto a new track (unchecked = selected track)",
                        );
                    }
                    PathPromptKind::ReplaceSample { clip_id } => {
                        let clip_label = session
                            .project()
                            .all_tracks()
                            .flat_map(|t| {
                                t.clip_slots
                                    .iter()
                                    .filter_map(|s| s.clip.as_ref())
                                    .chain(t.arrangement_clips.iter().map(|a| &a.clip))
                            })
                            .find(|c| c.id == *clip_id)
                            .map_or_else(|| format!("Clip #{}", clip_id.0), |c| c.name.clone());
                        ui.label(format!("Clip: {clip_label}"));
                    }
                    PathPromptKind::LoadAdv { .. } | PathPromptKind::LoadAdg { .. } => {}
                }

                ui.horizontal(|ui| {
                    ui.label("File path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.path)
                            .hint_text("Enter file path...")
                            .desired_width(340.0),
                    );
                });

                if let Some(err) = &state.error {
                    ui.colored_label(Color32::from_rgb(0xff, 0x55, 0x55), err);
                }

                ui.horizontal(|ui| {
                    if ui.button(submit_label).clicked() {
                        let path_trimmed = state.path.trim().to_owned();
                        if path_trimmed.is_empty() {
                            state.error = Some("Enter a file path".to_owned());
                        } else {
                            let target = PathBuf::from(&path_trimmed);
                            keep_open =
                                !self.execute_path_prompt(session, &mut state, &target, action);
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        keep_open = false;
                    }
                });
            });

        if open && keep_open {
            self.prompt = Some(state);
        }
    }

    fn execute_path_prompt(
        &mut self,
        session: &mut Session,
        state: &mut PathPromptState,
        target: &Path,
        action: &mut FileManagerAction,
    ) -> bool {
        match &state.kind {
            PathPromptKind::ImportMidi { track_id } => {
                let target_track = if state.create_new_track {
                    None
                } else {
                    *track_id
                };
                match session.import_midi_file(target_track, None, None, target) {
                    Ok(_) => {
                        action.status_message =
                            Some(format!("Imported MIDI file: {}", target.display()));
                        true
                    }
                    Err(err) => {
                        let msg = format!("MIDI import failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::ExportMidi { clip_id, clip_name } => {
                match session.export_midi_clip(*clip_id, target) {
                    Ok(()) => {
                        action.files_modified_on_disk = true;
                        action.status_message = Some(format!(
                            "Exported MIDI clip '{clip_name}' to {}",
                            target.display()
                        ));
                        true
                    }
                    Err(err) => {
                        let msg = format!("MIDI export failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::ImportAlc { track_id } => {
                let target_track = if state.create_new_track {
                    None
                } else {
                    *track_id
                };
                match session.import_live_clip(target_track, None, None, target) {
                    Ok(_) => {
                        action.status_message =
                            Some(format!("Imported Live Clip: {}", target.display()));
                        true
                    }
                    Err(err) => {
                        let msg = format!("Live Clip import failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::ExportAlc {
                track_id,
                clip_id,
                clip_name,
            } => match session.export_live_clip(*track_id, *clip_id, target, None) {
                Ok(ExportClipOutcome::Saved { copied_samples }) => {
                    action.files_modified_on_disk = true;
                    let suffix = if copied_samples {
                        " (samples copied)"
                    } else {
                        ""
                    };
                    action.status_message = Some(format!(
                        "Exported Live Clip '{clip_name}' to {}{suffix}",
                        target.display()
                    ));
                    true
                }
                Ok(ExportClipOutcome::NeedsCopyConfirmation) => {
                    self.ask_copy_on_export = Some(PendingAlcExport {
                        track_id: *track_id,
                        clip_id: *clip_id,
                        clip_name: clip_name.clone(),
                        out_path: target.to_path_buf(),
                    });
                    true
                }
                Err(err) => {
                    let msg = format!("Live Clip export failed: {err}");
                    state.error = Some(msg.clone());
                    action.status_message = Some(msg);
                    false
                }
            },
            PathPromptKind::SaveAdv {
                track_id,
                device_id,
                device_name,
            } => match session.save_device_preset(*track_id, *device_id, target) {
                Ok(()) => {
                    action.files_modified_on_disk = true;
                    action.status_message = Some(format!(
                        "Saved device preset '{device_name}' to {}",
                        target.display()
                    ));
                    true
                }
                Err(err) => {
                    let msg = format!("Save .adv failed: {err}");
                    state.error = Some(msg.clone());
                    action.status_message = Some(msg);
                    false
                }
            },
            PathPromptKind::LoadAdv { track_id } => {
                match session.load_device_preset(*track_id, target) {
                    Ok(_) => {
                        action.status_message =
                            Some(format!("Loaded device preset: {}", target.display()));
                        true
                    }
                    Err(err) => {
                        let msg = format!("Load .adv failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::SaveAdg { track_id } => {
                let rack_name = if state.rack_name.trim().is_empty() {
                    "Rack"
                } else {
                    state.rack_name.trim()
                };
                match session.save_rack_preset(*track_id, rack_name, [0.0; 8], target) {
                    Ok(()) => {
                        action.files_modified_on_disk = true;
                        action.status_message = Some(format!(
                            "Saved rack preset '{rack_name}' to {}",
                            target.display()
                        ));
                        true
                    }
                    Err(err) => {
                        let msg = format!("Save .adg failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::LoadAdg { track_id } => {
                match session.load_rack_preset(*track_id, target) {
                    Ok(devs) => {
                        action.status_message = Some(format!(
                            "Loaded rack preset ({} device(s)): {}",
                            devs.len(),
                            target.display()
                        ));
                        true
                    }
                    Err(err) => {
                        let msg = format!("Load .adg failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
            PathPromptKind::ReplaceSample { clip_id } => {
                match session.replace_clip_sample(*clip_id, target) {
                    Ok(()) => {
                        action.replaced_clip_sample = Some(*clip_id);
                        action.analysis_invalidated = true;
                        action.status_message = Some(format!(
                            "Replaced sample for clip #{} with {}",
                            clip_id.0,
                            target.display()
                        ));
                        true
                    }
                    Err(err) => {
                        let msg = format!("Replace Sample failed: {err}");
                        state.error = Some(msg.clone());
                        action.status_message = Some(msg);
                        false
                    }
                }
            }
        }
    }

    fn render_ask_copy_modal(
        &mut self,
        ctx: &egui::Context,
        session: &Session,
        action: &mut FileManagerAction,
    ) {
        let Some(pending) = self.ask_copy_on_export.clone() else {
            return;
        };
        let mut choice: Option<Option<bool>> = None;

        egui::Window::new("Collect Files on Export")
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Copy referenced audio files for '{}' into '{}'?",
                    pending.clip_name,
                    pending.out_path.display()
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        choice = Some(Some(true));
                    }
                    if ui.button("Don't Copy").clicked() {
                        choice = Some(Some(false));
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(None);
                    }
                });
            });

        if let Some(decision) = choice {
            self.ask_copy_on_export = None;
            match decision {
                Some(copy_audio) => {
                    match session.export_live_clip(
                        pending.track_id,
                        pending.clip_id,
                        &pending.out_path,
                        Some(copy_audio),
                    ) {
                        Ok(ExportClipOutcome::Saved { copied_samples }) => {
                            action.files_modified_on_disk = true;
                            let suffix = if copied_samples {
                                " (samples copied)"
                            } else {
                                " (without copying samples)"
                            };
                            action.status_message = Some(format!(
                                "Exported Live Clip '{}' to {}{suffix}",
                                pending.clip_name,
                                pending.out_path.display()
                            ));
                        }
                        Ok(ExportClipOutcome::NeedsCopyConfirmation) => {
                            action.status_message =
                                Some("Export still requires copy confirmation".to_owned());
                        }
                        Err(err) => {
                            action.status_message = Some(format!("Live Clip export failed: {err}"));
                        }
                    }
                }
                None => {
                    action.status_message = Some("Live Clip export cancelled".to_owned());
                }
            }
        }
    }
}

fn render_health_missing_section(
    ui: &mut egui::Ui,
    missing: &[MissingFile],
    locate_clicked: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.strong(format!("Missing Files ({})", missing.len()));
        if !missing.is_empty() && ui.small_button("Locate...").clicked() {
            *locate_clicked = true;
        }
    });
    for m in missing {
        ui.label(format!(
            "  - {} [{}] ({} usage(s))",
            m.file_name,
            m.reference.display_ref(),
            m.usages.len()
        ));
    }
}

fn render_health_unused_section(ui: &mut egui::Ui, unused: &[UnusedFileEntry]) {
    ui.strong(format!("Unused Files ({})", unused.len()));
    for u in unused {
        ui.label(format!(
            "  - {} ({:?}, {} bytes)",
            u.relative_path, u.category, u.size_bytes
        ));
    }
}

fn render_health_plugins_section(ui: &mut egui::Ui, report: &ProjectHealthReport) {
    ui.strong(format!(
        "Missing Plug-ins ({})",
        report.missing_plugins.len()
    ));
    for p in &report.missing_plugins {
        ui.label(format!(
            "  - [{}] {} ({})",
            p.track_name, p.device_name, p.plugin_type
        ));
    }
}

fn render_health_sr_section(ui: &mut egui::Ui, report: &ProjectHealthReport) {
    ui.strong(format!(
        "Sample-rate Mismatches ({})",
        report.sample_rate_mismatches.len()
    ));
    for s in &report.sample_rate_mismatches {
        ui.label(format!(
            "  - {}: file {} Hz != expected {} Hz",
            s.reference, s.actual_sample_rate, s.expected_sample_rate
        ));
    }
}

fn render_health_external_section(
    ui: &mut egui::Ui,
    report: &ProjectHealthReport,
    collect_clicked: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.strong(format!("External Files ({})", report.external_files.len()));
        if !report.external_files.is_empty() && ui.small_button("Collect...").clicked() {
            *collect_clicked = true;
        }
    });
    for e in &report.external_files {
        ui.label(format!(
            "  - {} ({:?}, {} bytes)",
            e.file_path.display(),
            e.location,
            e.size_bytes
        ));
    }
}

fn sanitize_filename(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned
    }
}

/// Serializes a [`ProjectHealthReport`] to formatted JSON without extra crate dependencies.
#[must_use]
pub fn health_report_to_json(report: &ProjectHealthReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    let _ = writeln!(out, "  \"set_name\": {},", json_str(&report.set_name));
    let _ = writeln!(
        out,
        "  \"set_path\": {},",
        json_opt_path(report.set_path.as_deref())
    );
    let _ = writeln!(
        out,
        "  \"project_root\": {},",
        json_opt_path(report.project_root.as_deref())
    );
    let _ = writeln!(out, "  \"is_healthy\": {},", report.is_healthy());
    let _ = writeln!(out, "  \"issue_count\": {},", report.issue_count());

    write_json_array(
        &mut out,
        "missing_files",
        &report.missing_files,
        |buf, item| {
            let rel_json = item
                .reference
                .relative_path
                .as_deref()
                .map_or_else(|| "null".to_owned(), json_str);
            let usages = json_usages(&item.usages);
            let _ = write!(
                buf,
                "{{\"file_name\": {}, \"path\": {}, \"relative_path\": {rel_json}, \"usages\": {usages}}}",
                json_str(&item.file_name),
                json_str(&item.reference.path),
            );
        },
    );
    out.push_str(",\n");

    write_json_array(
        &mut out,
        "unused_files",
        &report.unused_files,
        |buf, item| {
            let cat = format!("{:?}", item.category);
            let _ = write!(
                buf,
                "{{\"path\": {}, \"relative_path\": {}, \"category\": {}, \"size_bytes\": {}}}",
                json_path(&item.path),
                json_str(&item.relative_path),
                json_str(&cat),
                item.size_bytes,
            );
        },
    );
    out.push_str(",\n");

    write_json_array(
        &mut out,
        "missing_plugins",
        &report.missing_plugins,
        |buf, item| {
            let _ = write!(
                buf,
                "{{\"track_id\": {}, \"track_name\": {}, \"device_id\": {}, \"device_name\": {}, \"plugin_type\": {}}}",
                item.track_id,
                json_str(&item.track_name),
                item.device_id,
                json_str(&item.device_name),
                json_str(&item.plugin_type),
            );
        },
    );
    out.push_str(",\n");

    write_json_array(
        &mut out,
        "sample_rate_mismatches",
        &report.sample_rate_mismatches,
        |buf, item| {
            let usages = json_usages(&item.usages);
            let _ = write!(
                buf,
                "{{\"file_path\": {}, \"reference\": {}, \"expected_sample_rate\": {}, \"actual_sample_rate\": {}, \"usages\": {usages}}}",
                json_path(&item.file_path),
                json_str(&item.reference),
                item.expected_sample_rate,
                item.actual_sample_rate,
            );
        },
    );
    out.push_str(",\n");

    write_json_array(
        &mut out,
        "external_files",
        &report.external_files,
        |buf, item| {
            let usages = json_usages(&item.usages);
            let _ = write!(
                buf,
                "{{\"file_path\": {}, \"reference\": {}, \"location\": {}, \"size_bytes\": {}, \"usages\": {usages}}}",
                json_path(&item.file_path),
                json_str(&item.reference),
                json_sample_location(&item.location),
                item.size_bytes,
            );
        },
    );
    out.push_str("\n}");
    out
}

fn write_json_array<T>(
    out: &mut String,
    key: &str,
    items: &[T],
    mut write_item: impl FnMut(&mut String, &T),
) {
    if items.is_empty() {
        let _ = write!(out, "  \"{key}\": []");
        return;
    }
    let _ = writeln!(out, "  \"{key}\": [");
    for (idx, item) in items.iter().enumerate() {
        out.push_str("    ");
        write_item(out, item);
        if idx + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
}

fn json_usages(usages: &[ClipLocationRef]) -> String {
    let mut out = String::from("[");
    for (idx, u) in usages.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        match u {
            ClipLocationRef::SessionSlot(track_id, slot, name) => {
                let _ = write!(
                    out,
                    "{{\"kind\": \"SessionSlot\", \"track_id\": {track_id}, \"slot\": {slot}, \"clip_name\": {}}}",
                    json_str(name)
                );
            }
            ClipLocationRef::Arrangement(track_id, clip_id, name) => {
                let _ = write!(
                    out,
                    "{{\"kind\": \"Arrangement\", \"track_id\": {track_id}, \"clip_id\": {clip_id}, \"clip_name\": {}}}",
                    json_str(name)
                );
            }
            ClipLocationRef::XmlFileRef(tag) => {
                let _ = write!(
                    out,
                    "{{\"kind\": \"XmlFileRef\", \"tag\": {}}}",
                    json_str(tag)
                );
            }
        }
    }
    out.push(']');
    out
}

fn json_sample_location(loc: &SampleLocation) -> String {
    match loc {
        SampleLocation::InProject => json_str("InProject"),
        SampleLocation::UserLibrary => json_str("UserLibrary"),
        SampleLocation::FactoryPacks => json_str("FactoryPacks"),
        SampleLocation::OtherProject(path) => {
            format!("{{\"OtherProject\": {}}}", json_path(path))
        }
        SampleLocation::Elsewhere => json_str("Elsewhere"),
        SampleLocation::Missing => json_str("Missing"),
    }
}

fn json_opt_path(path: Option<&Path>) -> String {
    path.map_or_else(|| "null".to_owned(), json_path)
}

fn json_path(path: &Path) -> String {
    json_str(&path.to_string_lossy())
}

fn json_str(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_session::{ExternalFile, MissingPlugin, SampleRateMismatch, SampleReference};

    #[test]
    fn health_report_to_json_escapes_paths_and_lists_all_categories() {
        let report = ProjectHealthReport {
            set_name: "Demo \"Set\"".to_owned(),
            set_path: Some(PathBuf::from("C:\\Projects\\Demo\\Demo.als")),
            project_root: Some(PathBuf::from("C:\\Projects\\Demo")),
            missing_files: vec![MissingFile {
                reference: SampleReference {
                    path: "C:\\missing\\kick.wav".to_owned(),
                    relative_path: Some("Samples/Imported/kick.wav".to_owned()),
                },
                file_name: "kick.wav".to_owned(),
                usages: vec![ClipLocationRef::SessionSlot(1, 0, "Kick".to_owned())],
            }],
            unused_files: vec![UnusedFileEntry {
                path: PathBuf::from("C:\\Projects\\Demo\\Samples\\Imported\\old.wav"),
                relative_path: "Samples/Imported/old.wav".to_owned(),
                category: UnusedFileCategory::Imported,
                size_bytes: 2048,
            }],
            missing_plugins: vec![MissingPlugin {
                track_id: 2,
                track_name: "Synth".to_owned(),
                device_id: 99,
                device_name: "MissingVST".to_owned(),
                plugin_type: "Vst3PluginInfo".to_owned(),
            }],
            sample_rate_mismatches: vec![SampleRateMismatch {
                file_path: PathBuf::from("C:\\Projects\\Demo\\Samples\\Recorded\\vox.wav"),
                reference: "Samples/Recorded/vox.wav".to_owned(),
                expected_sample_rate: 44_100,
                actual_sample_rate: 48_000,
                usages: vec![ClipLocationRef::Arrangement(3, 55, "Vox".to_owned())],
            }],
            external_files: vec![ExternalFile {
                file_path: PathBuf::from("D:\\Lib\\snare.wav"),
                reference: "D:/Lib/snare.wav".to_owned(),
                location: SampleLocation::Elsewhere,
                size_bytes: 4096,
                usages: vec![ClipLocationRef::XmlFileRef("MultiSamplePart".to_owned())],
            }],
        };

        let json = health_report_to_json(&report);
        assert!(json.contains("\"set_name\": \"Demo \\\"Set\\\"\""));
        assert!(json.contains("\"set_path\": \"C:\\\\Projects\\\\Demo\\\\Demo.als\""));
        assert!(json.contains("\"is_healthy\": false"));
        assert!(json.contains("\"issue_count\": 5"));
        assert!(json.contains("\"missing_files\": ["));
        assert!(json.contains("\"unused_files\": ["));
        assert!(json.contains("\"missing_plugins\": ["));
        assert!(json.contains("\"sample_rate_mismatches\": ["));
        assert!(json.contains("\"external_files\": ["));
        assert!(json.contains("\"expected_sample_rate\": 44100"));
        assert!(json.contains("\"actual_sample_rate\": 48000"));
    }

    #[test]
    fn sanitize_filename_replaces_illegal_chars() {
        assert_eq!(sanitize_filename("Lead: \"1\"/2?", "clip"), "Lead_ _1__2_");
        assert_eq!(sanitize_filename("   ", "fallback"), "fallback");
    }
}
