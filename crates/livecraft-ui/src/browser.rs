use crate::style::LivePalette;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Sense, Ui, Vec2};
use livecraft_model::model::DeviceType;
use livecraft_session::ProjectContext;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SCAN_DEPTH: usize = 8;
const MAX_SCAN_ENTRIES: usize = 2000;

/// A device the browser asks the app to insert: its type and its name.
fn device_request(name: impl Into<String>, device_type: DeviceType) -> (DeviceType, String) {
    (device_type, name.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCategory {
    Collections(usize), // 0..7 colored dots
    Sounds,
    Drums,
    Instruments,
    AudioEffects,
    MidiEffects,
    Plugins,
    Clips,
    Samples,
    Packs,
    UserLibrary,
    CurrentProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFileKind {
    LiveClip,
    DevicePreset,
    RackPreset,
    MidiFile,
    AudioFile,
}

impl BrowserFileKind {
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "alc" => Some(Self::LiveClip),
            "adv" => Some(Self::DevicePreset),
            "adg" => Some(Self::RackPreset),
            "mid" | "midi" => Some(Self::MidiFile),
            "wav" | "aif" | "aiff" | "aifc" | "flac" | "mp3" | "ogg" | "oga" | "opus" | "m4a"
            | "aac" => Some(Self::AudioFile),
            _ => None,
        }
    }

    #[must_use]
    pub fn badge(self) -> &'static str {
        match self {
            Self::LiveClip => ".alc",
            Self::DevicePreset => ".adv",
            Self::RackPreset => ".adg",
            Self::MidiFile => ".mid",
            Self::AudioFile => "aud",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserFileEntry {
    pub path: PathBuf,
    pub display_name: String,
    pub kind: BrowserFileKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrowserAction {
    pub add_device: Option<(DeviceType, String)>,
    pub import_alc: Option<PathBuf>,
    pub load_adv: Option<PathBuf>,
    pub load_adg: Option<PathBuf>,
    pub import_mid: Option<PathBuf>,
}

pub struct LiveBrowser {
    pub selected_category: BrowserCategory,
    pub search_query: String,
    cached_category: Option<BrowserCategory>,
    cached_context: Option<ProjectContext>,
    cached_entries: Vec<BrowserFileEntry>,
    cache_dirty: bool,
}

impl Default for LiveBrowser {
    fn default() -> Self {
        Self {
            selected_category: BrowserCategory::Instruments,
            search_query: String::new(),
            cached_category: None,
            cached_context: None,
            cached_entries: Vec::new(),
            cache_dirty: true,
        }
    }
}

impl LiveBrowser {
    /// Marks the file-browser scan cache stale so the next frame refreshes from disk.
    pub fn invalidate(&mut self) {
        self.cache_dirty = true;
    }

    /// Returns the currently cached scanned entries (for diagnostics or tests).
    #[must_use]
    pub fn cached_entries(&self) -> &[BrowserFileEntry] {
        &self.cached_entries
    }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        palette: &LivePalette,
        project_context: &ProjectContext,
    ) -> BrowserAction {
        let mut action = BrowserAction::default();

        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

            // 1. Search Bar (Manual p. 65)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                let w = ui.available_width()
                    - if self.search_query.is_empty() {
                        0.0
                    } else {
                        22.0
                    };
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search (Ctrl+F)")
                        .desired_width(w),
                );
                if !self.search_query.is_empty() && ui.small_button("x").clicked() {
                    self.search_query.clear();
                }
            });

            ui.add_space(3.0);

            // 2. Collections (Manual p. 81)
            ui.label(
                egui::RichText::new("COLLECTIONS")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let collections = [
                (Color32::from_rgb(0xff, 0x4f, 0x4f), "Favorites"),
                (Color32::from_rgb(0xff, 0x9f, 0x36), "Synths"),
                (Color32::from_rgb(0xff, 0xd8, 0x3d), "Drums"),
                (Color32::from_rgb(0x38, 0xd8, 0x55), "Bass"),
                (Color32::from_rgb(0x2f, 0x77, 0xee), "FX"),
                (Color32::from_rgb(0xaa, 0x55, 0xff), "Templates"),
                (Color32::from_rgb(0x88, 0x88, 0x88), "Other"),
            ];

            for (idx, (dot_color, label)) in collections.iter().enumerate() {
                let is_sel = self.selected_category == BrowserCategory::Collections(idx);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.button_hover);
                    }
                    // Color dot
                    painter.circle_filled(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        3.5,
                        *dot_color,
                    );
                    painter.text(
                        Pos2::new(rect.min.x + 20.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        *label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = BrowserCategory::Collections(idx);
                    self.cache_dirty = true;
                }
            }

            ui.add_space(4.0);

            // 3. Categories (Manual p. 64)
            ui.label(
                egui::RichText::new("CATEGORIES")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let categories = [
                (BrowserCategory::Sounds, "Sounds"),
                (BrowserCategory::Drums, "Drums"),
                (BrowserCategory::Instruments, "Instruments"),
                (BrowserCategory::AudioEffects, "Audio Effects"),
                (BrowserCategory::MidiEffects, "MIDI Effects"),
                (BrowserCategory::Plugins, "Plug-Ins"),
                (BrowserCategory::Clips, "Clips"),
                (BrowserCategory::Samples, "Samples"),
            ];

            for (cat, label) in categories {
                let is_sel = self.selected_category == cat;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.button_hover);
                    }
                    painter.text(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = cat;
                    self.cache_dirty = true;
                }
            }

            ui.add_space(4.0);

            // 4. Places (Manual p. 85)
            ui.label(
                egui::RichText::new("PLACES")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let places = [
                (BrowserCategory::Packs, "Packs"),
                (BrowserCategory::UserLibrary, "User Library"),
                (BrowserCategory::CurrentProject, "Current Project"),
            ];

            for (cat, label) in places {
                let is_sel = self.selected_category == cat;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::same(2), palette.button_hover);
                    }
                    painter.text(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = cat;
                    self.cache_dirty = true;
                }
            }

            ui.add_space(4.0);
            ui.separator();

            if self.cache_dirty
                || self.cached_category != Some(self.selected_category)
                || self.cached_context.as_ref() != Some(project_context)
            {
                self.cached_entries = scan_browser_files(self.selected_category, project_context);
                self.cached_category = Some(self.selected_category);
                self.cached_context = Some(project_context.clone());
                self.cache_dirty = false;
            }

            // 5. Category Content List (scrollable)
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.selected_category {
                    BrowserCategory::Instruments => {
                        let instruments = [
                            (
                                "Operator",
                                DeviceType::Operator,
                                "4-Osc FM & Subtractive Synthesizer",
                            ),
                            (
                                "Simpler",
                                DeviceType::Simpler,
                                "Polyphonic Sample Instrument",
                            ),
                            (
                                "Drum Rack",
                                DeviceType::DrumRack,
                                "16-Pad Drum Sampler & Synth",
                            ),
                            (
                                "Analog",
                                DeviceType::Operator,
                                "Vintage Analog Modeling Synth",
                            ),
                            (
                                "Tension",
                                DeviceType::Operator,
                                "Physical Modeling String Synth",
                            ),
                            (
                                "Electric",
                                DeviceType::Operator,
                                "Classic Electric Piano Modeling",
                            ),
                        ];
                        for (name, dtype, desc) in instruments {
                            if !matches_query(name, &self.search_query) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(false, name)
                                    .on_hover_text(desc)
                                    .clicked()
                                {
                                    action.add_device = Some(device_request(name, dtype.clone()));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(device_request(name, dtype.clone()));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::AudioEffects => {
                        let fx = [
                            ("Chorus-Ensemble", DeviceType::Chorus),
                            ("Compressor", DeviceType::Compressor),
                            ("Delay", DeviceType::Delay),
                            ("EQ Eight", DeviceType::EqEight),
                            ("Reverb", DeviceType::Reverb),
                            ("Saturator", DeviceType::Saturator),
                            ("Utility", DeviceType::Utility),
                            ("Auto Filter", DeviceType::AutoFilter),
                            ("Auto Pan", DeviceType::AutoPan),
                            ("Gate", DeviceType::Gate),
                            ("Redux", DeviceType::Redux),
                        ];
                        for (name, dtype) in fx {
                            if !matches_query(name, &self.search_query) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, name).clicked() {
                                    action.add_device = Some(device_request(name, dtype.clone()));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(device_request(name, dtype.clone()));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::Drums => {
                        let kits = [
                            "808 Core Kit",
                            "909 Vintage Kit",
                            "Acoustic Studio Kit",
                            "Electronic Breakbeat Kit",
                            "Trap 808 Essentials",
                        ];
                        for kit in kits {
                            if !matches_query(kit, &self.search_query) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, kit).clicked() {
                                    action.add_device =
                                        Some(device_request(kit, DeviceType::DrumRack));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(device_request(kit, DeviceType::DrumRack));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::Samples => {
                        if !self.cached_entries.is_empty() {
                            self.render_scanned_entries(ui, palette, &mut action);
                        } else {
                            let samples = [
                                "808 Kick Drum.wav",
                                "Crisp Snare 01.wav",
                                "Hi-Hat Closed 16th.wav",
                                "Synth Lead C4.wav",
                                "Sub Bass 40Hz.wav",
                                "Amen Break 174bpm.wav",
                            ];
                            for s in samples {
                                if !matches_query(s, &self.search_query) {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(s)
                                            .size(10.0)
                                            .color(palette.control_foreground),
                                    );
                                });
                            }
                        }
                    }
                    BrowserCategory::CurrentProject
                    | BrowserCategory::UserLibrary
                    | BrowserCategory::Clips
                    | BrowserCategory::Packs => {
                        ui.horizontal(|ui| {
                            if ui.small_button("Refresh").clicked() {
                                self.cache_dirty = true;
                            }
                        });
                        if self.cached_entries.is_empty() {
                            ui.label(
                                egui::RichText::new("No matching files on disk.")
                                    .size(10.0)
                                    .color(palette.control_disabled),
                            );
                        } else {
                            self.render_scanned_entries(ui, palette, &mut action);
                        }
                    }
                    _ => {
                        let defaults = [
                            ("Operator Preset 1", DeviceType::Operator),
                            ("Ambient Reverb", DeviceType::Reverb),
                            ("Stereo Delay Echo", DeviceType::Delay),
                        ];
                        for (name, dtype) in defaults {
                            if !matches_query(name, &self.search_query) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, name).clicked() {
                                    action.add_device = Some(device_request(name, dtype));
                                }
                            });
                        }
                    }
                });
        });

        action
    }

    fn render_scanned_entries(
        &self,
        ui: &mut Ui,
        palette: &LivePalette,
        action: &mut BrowserAction,
    ) {
        for entry in &self.cached_entries {
            if !matches_query(&entry.display_name, &self.search_query) {
                continue;
            }
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(entry.kind.badge())
                        .size(9.0)
                        .monospace()
                        .color(palette.amber),
                );
                let resp = ui
                    .selectable_label(
                        false,
                        egui::RichText::new(&entry.display_name)
                            .size(10.0)
                            .color(palette.control_foreground),
                    )
                    .on_hover_text(entry.path.display().to_string());
                if resp.clicked() {
                    match entry.kind {
                        BrowserFileKind::LiveClip => {
                            action.import_alc = Some(entry.path.clone());
                        }
                        BrowserFileKind::DevicePreset => {
                            action.load_adv = Some(entry.path.clone());
                        }
                        BrowserFileKind::RackPreset => {
                            action.load_adg = Some(entry.path.clone());
                        }
                        BrowserFileKind::MidiFile => {
                            action.import_mid = Some(entry.path.clone());
                        }
                        BrowserFileKind::AudioFile => {}
                    }
                }
            });
        }
    }
}

fn matches_query(name: &str, query: &str) -> bool {
    query.trim().is_empty()
        || name
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
}

/// Scans real files on disk for `category` under `ctx`'s existing directories.
#[must_use]
pub fn scan_browser_files(
    category: BrowserCategory,
    ctx: &ProjectContext,
) -> Vec<BrowserFileEntry> {
    let mut roots: Vec<PathBuf> = Vec::new();
    match category {
        BrowserCategory::CurrentProject => {
            if let Some(root) = ctx.project_root.as_deref().or(ctx.set_dir.as_deref()) {
                roots.push(root.to_path_buf());
            }
        }
        BrowserCategory::UserLibrary => {
            if let Some(lib) = ctx.user_library.as_deref() {
                roots.push(lib.to_path_buf());
            }
        }
        BrowserCategory::Packs => {
            roots.extend(ctx.factory_packs_roots.iter().cloned());
            if let Some(lib) = ctx.user_library.as_deref() {
                let packs_sub = lib.join("Packs");
                if packs_sub.is_dir() && !roots.contains(&packs_sub) {
                    roots.push(packs_sub);
                }
            }
        }
        BrowserCategory::Clips | BrowserCategory::Samples => {
            if let Some(root) = ctx.project_root.as_deref().or(ctx.set_dir.as_deref()) {
                roots.push(root.to_path_buf());
            }
            if let Some(lib) = ctx.user_library.as_deref() {
                roots.push(lib.to_path_buf());
            }
            roots.extend(ctx.factory_packs_roots.iter().cloned());
        }
        _ => return Vec::new(),
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        walk_browser_dir(&root, &root, 0, category, &mut seen, &mut out);
        if out.len() >= MAX_SCAN_ENTRIES {
            break;
        }
    }

    out.sort_by(|a, b| {
        a.display_name
            .to_ascii_lowercase()
            .cmp(&b.display_name.to_ascii_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    out
}

fn walk_browser_dir(
    root: &Path,
    dir: &Path,
    depth: usize,
    category: BrowserCategory,
    seen: &mut HashSet<PathBuf>,
    out: &mut Vec<BrowserFileEntry>,
) {
    if depth > MAX_SCAN_DEPTH || out.len() >= MAX_SCAN_ENTRIES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<fs::DirEntry> = entries.flatten().collect();
    children.sort_by_key(|e| e.file_name());

    for entry in children {
        if out.len() >= MAX_SCAN_ENTRIES {
            break;
        }
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if ft.is_dir() {
            let name = entry.file_name();
            let name_lossy = name.to_string_lossy();
            if name_lossy == "Ableton Project Info" || name_lossy == "Backup" {
                continue;
            }
            walk_browser_dir(root, &path, depth + 1, category, seen, out);
        } else if ft.is_file()
            && let Some(kind) = BrowserFileKind::from_path(&path)
        {
            let include = match category {
                BrowserCategory::Clips => matches!(
                    kind,
                    BrowserFileKind::LiveClip
                        | BrowserFileKind::MidiFile
                        | BrowserFileKind::AudioFile
                ),
                BrowserCategory::Samples => kind == BrowserFileKind::AudioFile,
                _ => true,
            };
            if !include {
                continue;
            }
            let dedup_key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !seen.insert(dedup_key) {
                continue;
            }
            let display_name = path
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    )
                });
            out.push(BrowserFileEntry {
                path,
                display_name,
                kind,
            });
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
                "livecraft_browser_test_{label}_{}_{}",
                std::process::id(),
                id
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scan_browser_files_finds_supported_formats_and_skips_internal_dirs() {
        let dir = TempDirGuard::new("scan");
        let proj = dir.0.join("My Project");
        let lib = dir.0.join("User Library");
        let packs = dir.0.join("Packs");

        fs::create_dir_all(proj.join("Ableton Project Info")).expect("info dir");
        fs::create_dir_all(proj.join("Backup")).expect("backup dir");
        fs::create_dir_all(proj.join("Samples/Imported")).expect("samples dir");
        fs::create_dir_all(lib.join("Presets")).expect("lib dir");
        fs::create_dir_all(packs.join("Core")).expect("packs dir");

        fs::write(proj.join("Ableton Project Info/ignored.wav"), b"x").expect("write");
        fs::write(proj.join("Backup/ignored.alc"), b"x").expect("write");
        fs::write(proj.join("Samples/Imported/kick.wav"), b"x").expect("write");
        fs::write(proj.join("lead.alc"), b"x").expect("write");
        fs::write(proj.join("melody.mid"), b"x").expect("write");
        fs::write(lib.join("Presets/eq.adv"), b"x").expect("write");
        fs::write(lib.join("Presets/rack.adg"), b"x").expect("write");
        fs::write(packs.join("Core/loop.alc"), b"x").expect("write");

        let ctx = ProjectContext {
            project_root: Some(proj.clone()),
            set_dir: Some(proj),
            user_library: Some(lib),
            factory_packs_roots: vec![packs],
        };

        let cur = scan_browser_files(BrowserCategory::CurrentProject, &ctx);
        let cur_names: Vec<&str> = cur.iter().map(|e| e.display_name.as_str()).collect();
        assert_eq!(
            cur_names,
            vec!["lead.alc", "melody.mid", "Samples/Imported/kick.wav"]
        );

        let user = scan_browser_files(BrowserCategory::UserLibrary, &ctx);
        let user_names: Vec<&str> = user.iter().map(|e| e.display_name.as_str()).collect();
        assert_eq!(user_names, vec!["Presets/eq.adv", "Presets/rack.adg"]);

        let pack_entries = scan_browser_files(BrowserCategory::Packs, &ctx);
        assert_eq!(pack_entries.len(), 1);
        assert_eq!(pack_entries[0].kind, BrowserFileKind::LiveClip);

        let clip_entries = scan_browser_files(BrowserCategory::Clips, &ctx);
        assert!(
            clip_entries
                .iter()
                .any(|e| e.kind == BrowserFileKind::LiveClip)
        );
        assert!(
            clip_entries
                .iter()
                .any(|e| e.kind == BrowserFileKind::MidiFile)
        );
        assert!(
            !clip_entries
                .iter()
                .any(|e| e.kind == BrowserFileKind::DevicePreset)
        );
    }
}
