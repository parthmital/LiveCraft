use crate::Edits;
use crate::arrange::render_arrangement_view;
use crate::browser::LiveBrowser;
use crate::devices::render_device_chain;
use crate::pianoroll::render_piano_roll;
use crate::session::render_session_view;
use crate::style::{LivePalette, apply_live_style};
use crate::transport::{ViewMode, render_transport_bar};
use eframe::egui::{self, Color32, Key, Vec2};
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use livecraft_session::commands::device::InsertDevice;
use livecraft_session::commands::track::{
    CreateAudioTrack, CreateMidiTrack, SetTrackArmed, SetTrackMuted, SetTrackSoloed,
};
use livecraft_session::{Command, Session, demo_set, load_set};
use std::path::{Path, PathBuf};

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
}

impl LiveCraftApp {
    pub fn new(project_path: Option<&Path>) -> Self {
        Self::new_with_options(crate::GuiOptions {
            project_path: project_path.map(|p| p.to_path_buf()),
            ..Default::default()
        })
    }

    pub fn new_with_options(options: crate::GuiOptions) -> Self {
        let palette = match options.theme.as_deref() {
            Some("light") => LivePalette::light(),
            _ => LivePalette::dark(),
        };

        let loaded = options
            .project_path
            .as_ref()
            .and_then(|path| load_set(path).ok().map(|project| (project, path.clone())));
        let (project, project_path) = match loaded {
            Some((project, path)) => (project, Some(path)),
            None => (
                demo_set().unwrap_or_else(|_| Project::new_default("LiveCraft Session")),
                None,
            ),
        };

        // Try initializing audio engine

        let view_mode = match options.initial_view.as_deref() {
            Some("arrange") | Some("arrangement") => ViewMode::Arrangement,
            _ => ViewMode::Session,
        };

        let bottom_tab = options.initial_tab.unwrap_or(0);
        let selected_track = project.tracks.first().map_or(TrackId(0), |track| track.id);
        Self {
            session: Session::open(project),
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
        }
    }

    /// Applies the changes a view asked for, through the session.
    fn apply(&mut self, edits: Edits) {
        if let Err(error) = self.session.apply(edits) {
            self.status_message = format!("Edit refused: {error}");
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Spacebar -> Toggle Playback
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            let playing = !self.session.project().transport.is_playing;
            self.session.set_playing(playing);
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

        // Ctrl+T -> Insert Audio Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            let number = self.session.project().tracks.len() + 1;
            let name = format!("{}-Audio", number);
            self.apply(vec![Box::new(CreateAudioTrack::new(name))]);
            self.status_message = "Inserted Audio Track".to_string();
        }

        // Ctrl+Shift+T -> Insert MIDI Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            let number = self.session.project().tracks.len() + 1;
            let name = format!("{}-MIDI", number);
            self.apply(vec![Box::new(CreateMidiTrack::new(name))]);
            self.status_message = "Inserted MIDI Track".to_string();
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

        // Ctrl+Z -> Undo
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z)) && self.session.undo() {
            self.status_message = "Undo".to_string();
        }

        // Ctrl+Y -> Redo
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Y)) {
            match self.session.redo() {
                Ok(true) => self.status_message = "Redo".to_string(),
                Ok(false) => {}
                Err(error) => self.status_message = format!("Redo failed: {error}"),
            }
        }

        // Ctrl+S -> Save
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            let path = self
                .project_path
                .clone()
                .unwrap_or_else(|| PathBuf::from("Untitled.als"));
            match self.session.save(&path) {
                Ok(()) => {
                    self.status_message = format!("Saved to {}", path.display());
                }
                Err(error) => self.status_message = format!("Save failed: {error}"),
            }
            self.project_path = Some(path);
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
}

impl eframe::App for LiveCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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

        // Continuous repaint during audio playback
        if self.session.project().transport.is_playing {
            ctx.request_repaint();
        }

        // The engine's playhead, meters, and active clips, read without blocking the audio thread
        let status = self.session.status();
        let playhead_beats = status.playhead_beats;

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
                    self.session.stop_all();
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

                    ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(10.5)
                            .color(Color32::from_rgb(0xa0, 0xa0, 0xa0)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let engine_text = match status.sample_rate {
                            Some(rate) => format!("{:.1} kHz", rate / 1000.0),
                            None => "No audio device".to_string(),
                        };
                        ui.label(
                            egui::RichText::new(engine_text)
                                .size(10.0)
                                .color(Color32::from_rgb(0x70, 0x70, 0x70)),
                        );
                    });
                });
            });

        // Bottom Detail Panel: Device Chain or Piano Roll (Manual p. 50, 445)
        let mut detail_edits: Edits = Vec::new();
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

                let project = self.session.project();
                if self.bottom_tab == 0 {
                    // Device Chain View
                    if let Some(trk) = project.track(self.selected_track) {
                        render_device_chain(ui, trk, &self.palette, &mut detail_edits);
                    }
                } else if let Some((t_id, s_idx)) = self.selected_clip {
                    // Clip View / Piano Roll
                    let slot_clip = project
                        .track(t_id)
                        .and_then(|trk| trk.clip_slots.get(s_idx))
                        .and_then(|slot| slot.clip.as_ref());
                    if let Some(clip) = slot_clip {
                        render_piano_roll(
                            ui,
                            clip,
                            &project.scale,
                            playhead_beats,
                            &self.palette,
                            &mut detail_edits,
                        );
                    } else {
                        ui.label("Empty slot. Double-click in Session View to create a clip.");
                    }
                } else {
                    ui.label("Select a clip to view notes");
                }
            });
        self.apply(detail_edits);

        // Left Browser Sidebar (toggleable)
        if self.browser_visible {
            let mut device_request = None;
            egui::Panel::left("browser_panel")
                .exact_size(180.0)
                .resizable(true)
                .show(ui, |ui| {
                    let action = self.browser.render(ui, &self.palette);
                    device_request = action.add_device;
                });

            if let Some((device_type, name)) = device_request {
                let command: Box<dyn Command> =
                    Box::new(InsertDevice::new(self.selected_track, device_type, name));
                self.apply(vec![command]);
                self.status_message = "Device added to track".to_string();
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
