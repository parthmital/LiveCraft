use crate::arrange::render_arrangement_view;
use crate::browser::LiveBrowser;
use crate::devices::render_device_chain;
use crate::pianoroll::render_piano_roll;
use crate::session::render_session_view;
use crate::style::{LivePalette, apply_live_style};
use crate::transport::{ViewMode, render_transport_bar};
use eframe::egui::{self, Color32, Key, Vec2};
use livecraft_engine::engine::AudioEngine;
use livecraft_engine::graph::TrackMeter;
use livecraft_formats::als::AlsParser;
use livecraft_model::command::CommandHistory;
use livecraft_model::ids::{IdAllocator, TrackId};
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project, Track, TrackType};
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq)]
struct TrackParamSnapshot {
    volume_db: f32,
    pan: f32,
    is_muted: bool,
    is_soloed: bool,
    sends: Vec<f32>,
}

pub struct LiveCraftApp {
    pub project: Project,
    pub project_path: Option<PathBuf>,
    pub engine: Option<AudioEngine>,
    pub command_history: CommandHistory,
    pub view_mode: ViewMode,
    pub selected_track: TrackId,
    pub selected_clip: Option<(TrackId, usize)>, // (track, slot_idx)
    pub ids: IdAllocator,
    pub bottom_tab: usize, // 0 = Device Chain, 1 = Piano Roll / Clip View
    pub browser: LiveBrowser,
    pub palette: LivePalette,
    pub status_message: String,
    pub screenshot_path: Option<PathBuf>,
    pub frame_counter: u32,
    pub browser_visible: bool,
    last_bpm: f32,
    last_master_vol: f32,
    last_master_pan: f32,
    last_tracks_snapshot: Vec<TrackParamSnapshot>,
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

        let (project, path_buf) = if let Some(ref p) = options.project_path {
            if let Ok(proj) = AlsParser::load_project(p) {
                (proj, Some(p.clone()))
            } else {
                (Self::create_demo_project(), None)
            }
        } else {
            (Self::create_demo_project(), None)
        };

        // Try initializing audio engine
        let engine = AudioEngine::new(&project).ok();

        let view_mode = match options.initial_view.as_deref() {
            Some("arrange") | Some("arrangement") => ViewMode::Arrangement,
            _ => ViewMode::Session,
        };

        let bottom_tab = options.initial_tab.unwrap_or(0);

        let last_bpm = project.transport.bpm();
        let last_master_vol = project.master_track.volume_db;
        let last_master_pan = project.master_track.pan;
        let last_tracks_snapshot = project
            .tracks
            .iter()
            .map(|t| TrackParamSnapshot {
                volume_db: t.volume_db,
                pan: t.pan,
                is_muted: t.is_muted,
                is_soloed: t.is_soloed,
                sends: t.sends.clone(),
            })
            .collect();

        let ids = project.id_allocator();
        let selected_track = project.tracks.first().map_or(TrackId(0), |track| track.id);
        Self {
            project,
            project_path: path_buf,
            engine,
            command_history: CommandHistory::default(),
            view_mode,
            selected_track,
            selected_clip: Some((selected_track, 0)),
            ids,
            bottom_tab,
            browser: LiveBrowser::default(),
            palette,
            status_message: "LiveCraft Ready".to_string(),
            screenshot_path: options.screenshot_path,
            frame_counter: 0,
            browser_visible: true,
            last_bpm,
            last_master_vol,
            last_master_pan,
            last_tracks_snapshot,
        }
    }

    fn create_demo_project() -> Project {
        let mut proj = Project::new_default("LiveCraft Session");
        // Add a demo clip with notes on track 0 (Operator)
        let mut clip = Clip::new_midi(101, "Intro Chords", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // C4
            mdata.notes.push(MidiNote::new(64, 0.0, 1.0, 100)); // E4
            mdata.notes.push(MidiNote::new(67, 0.0, 1.0, 100)); // G4

            mdata.notes.push(MidiNote::new(62, 1.0, 1.0, 95)); // D4
            mdata.notes.push(MidiNote::new(65, 1.0, 1.0, 95)); // F4
            mdata.notes.push(MidiNote::new(69, 1.0, 1.0, 95)); // A4

            mdata.notes.push(MidiNote::new(64, 2.0, 2.0, 105)); // E4
            mdata.notes.push(MidiNote::new(67, 2.0, 2.0, 105)); // G4
            mdata.notes.push(MidiNote::new(71, 2.0, 2.0, 105)); // B4
        }
        proj.tracks[0].clip_slots[0].clip = Some(clip);
        proj
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Spacebar -> Toggle Playback
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            self.project.transport.is_playing = !self.project.transport.is_playing;
            if let Some(ref engine) = self.engine {
                if self.project.transport.is_playing {
                    engine.play();
                } else {
                    engine.stop();
                }
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

        // Ctrl+T -> Insert Audio Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            let number = self.project.tracks.len() + 1;
            let name = format!("{}-Audio", number);
            self.project
                .tracks
                .push(Track::new(self.ids.track(), name, TrackType::Audio, 8));
            self.status_message = "Inserted Audio Track".to_string();
        }

        // Ctrl+Shift+T -> Insert MIDI Track (Chapter 42.19)
        if ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::T)
        }) {
            let number = self.project.tracks.len() + 1;
            let name = format!("{}-MIDI", number);
            self.project
                .tracks
                .push(Track::new(self.ids.track(), name, TrackType::Midi, 8));
            self.status_message = "Inserted MIDI Track".to_string();
        }

        // 0 -> Toggle Mute / Track Activator on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::Num0))
            && let Some(trk) = self.project.track_mut(self.selected_track)
        {
            trk.is_muted = !trk.is_muted;
            self.status_message = format!("Track {} muted: {}", trk.name, trk.is_muted);
        }

        // S -> Toggle Solo on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::S))
            && let Some(trk) = self.project.track_mut(self.selected_track)
        {
            trk.is_soloed = !trk.is_soloed;
            self.status_message = format!("Track {} soloed: {}", trk.name, trk.is_soloed);
        }

        // C -> Toggle Arm on selected track (Chapter 42.19)
        if ctx.input(|i| !i.modifiers.command && i.key_pressed(Key::C))
            && let Some(trk) = self.project.track_mut(self.selected_track)
        {
            trk.is_armed = !trk.is_armed;
            self.status_message = format!("Track {} armed: {}", trk.name, trk.is_armed);
        }

        // Ctrl+Z -> Undo
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z))
            && self.command_history.undo(&mut self.project)
        {
            self.status_message = "Undo".to_string();
        }

        // Ctrl+Y -> Redo
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Y))
            && self.command_history.redo(&mut self.project)
        {
            self.status_message = "Redo".to_string();
        }

        // Ctrl+S -> Save
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            if let Some(ref path) = self.project_path {
                let _ = AlsParser::save_project_to_als(&self.project, path);
                self.status_message = format!("Saved to {}", path.display());
            } else {
                let default_save = PathBuf::from("Untitled.als");
                let _ = AlsParser::save_project_to_als(&self.project, &default_save);
                self.project_path = Some(default_save);
                self.status_message = "Saved to Untitled.als".to_string();
            }
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

    fn sync_audio_parameters(&mut self) {
        if let Some(ref eng) = self.engine {
            if self.project.tracks.len() != self.last_tracks_snapshot.len() {
                eng.sync_project(&self.project);
                self.last_tracks_snapshot = self
                    .project
                    .tracks
                    .iter()
                    .map(|t| TrackParamSnapshot {
                        volume_db: t.volume_db,
                        pan: t.pan,
                        is_muted: t.is_muted,
                        is_soloed: t.is_soloed,
                        sends: t.sends.clone(),
                    })
                    .collect();
                self.last_bpm = self.project.transport.bpm();
                self.last_master_vol = self.project.master_track.volume_db;
                self.last_master_pan = self.project.master_track.pan;
                return;
            }

            if (self.project.transport.bpm() - self.last_bpm).abs() > 0.001 {
                eng.set_tempo(self.project.transport.bpm());
                self.last_bpm = self.project.transport.bpm();
            }

            if (self.project.master_track.volume_db - self.last_master_vol).abs() > 0.01 {
                eng.set_master_volume(self.project.master_track.volume_db);
                self.last_master_vol = self.project.master_track.volume_db;
            }

            if (self.project.master_track.pan - self.last_master_pan).abs() > 0.01 {
                eng.set_master_pan(self.project.master_track.pan);
                self.last_master_pan = self.project.master_track.pan;
            }

            for (idx, trk) in self.project.tracks.iter().enumerate() {
                if let Some(snap) = self.last_tracks_snapshot.get_mut(idx) {
                    if (trk.volume_db - snap.volume_db).abs() > 0.01 {
                        eng.set_track_volume(trk.id, trk.volume_db);
                        snap.volume_db = trk.volume_db;
                    }
                    if (trk.pan - snap.pan).abs() > 0.01 {
                        eng.set_track_pan(trk.id, trk.pan);
                        snap.pan = trk.pan;
                    }
                    if trk.is_muted != snap.is_muted {
                        eng.set_track_mute(trk.id, trk.is_muted);
                        snap.is_muted = trk.is_muted;
                    }
                    if trk.is_soloed != snap.is_soloed {
                        eng.set_track_solo(trk.id, trk.is_soloed);
                        snap.is_soloed = trk.is_soloed;
                    }
                    for (s_idx, &send_val) in trk.sends.iter().enumerate() {
                        let prev_val = snap.sends.get(s_idx).copied().unwrap_or(0.0);
                        if (send_val - prev_val).abs() > 0.01 {
                            eng.set_track_send(trk.id, s_idx, send_val);
                            if s_idx < snap.sends.len() {
                                snap.sends[s_idx] = send_val;
                            } else {
                                snap.sends.push(send_val);
                            }
                        }
                    }
                }
            }
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

        // Dispatch discrete audio parameter changes lock-free to the audio engine
        self.sync_audio_parameters();

        // Continuous repaint during audio playback
        if self.project.transport.is_playing {
            ctx.request_repaint();
        }

        // Query real-time playhead and meters from engine
        let (playhead_beats, track_meters, master_meter, active_slots) =
            if let Some(ref eng) = self.engine {
                match eng.graph.try_lock() {
                    Ok(g) => {
                        let t_meters = g.track_states.iter().map(|s| s.meter.clone()).collect();
                        let m_meter = g.master_meter.clone();
                        let a_slots: Vec<_> =
                            g.track_states.iter().map(|s| s.active_clip_slot).collect();
                        (g.playhead_beats, t_meters, m_meter, a_slots)
                    }
                    _ => (
                        self.project.transport.playhead_beats,
                        Vec::new(),
                        TrackMeter::default(),
                        Vec::new(),
                    ),
                }
            } else {
                (
                    self.project.transport.playhead_beats,
                    Vec::new(),
                    TrackMeter::default(),
                    Vec::new(),
                )
            };

        self.project.transport.playhead_beats = playhead_beats;

        // Top Transport Panel
        egui::Panel::top("transport_bar")
            .exact_size(34.0)
            .show(ui, |ui| {
                let t_resp = render_transport_bar(
                    ui,
                    &mut self.project,
                    self.view_mode,
                    &self.palette,
                    2.5, // CPU percent
                    self.browser_visible,
                );

                if t_resp.browser_toggled {
                    self.browser_visible = !self.browser_visible;
                }

                if t_resp.play_toggled
                    && let Some(ref eng) = self.engine
                {
                    if self.project.transport.is_playing {
                        eng.play();
                    } else {
                        eng.stop();
                    }
                }
                if t_resp.stop_clicked
                    && let Some(ref eng) = self.engine
                {
                    eng.stop();
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
                        let engine_text = match &self.engine {
                            Some(eng) => format!("{:.1} kHz", eng.sample_rate / 1000.0),
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

                    if let Some(trk) = self.project.track(self.selected_track) {
                        crate::widgets::live_divider(ui, 20.0);
                        let [r, g, b, _] = livecraft_formats::theme::parse_hex_color(&trk.color);
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
                    if let Some(trk) = self.project.track_mut(self.selected_track) {
                        render_device_chain(ui, trk, &self.palette);
                    }
                } else {
                    // Clip View / Piano Roll
                    if let Some((t_id, s_idx)) = self.selected_clip {
                        let scale = &mut self.project.scale;
                        if let Some(trk) = self.project.tracks.iter_mut().find(|t| t.id == t_id)
                            && let Some(slot) = trk.clip_slots.get_mut(s_idx)
                        {
                            if let Some(ref mut clip) = slot.clip {
                                render_piano_roll(ui, clip, scale, playhead_beats, &self.palette);
                            } else {
                                ui.label(
                                    "Empty slot. Double-click in Session View to create a clip.",
                                );
                            }
                        }
                    } else {
                        ui.label("Select a clip to view notes");
                    }
                }
            });

        // Left Browser Sidebar (toggleable)
        if self.browser_visible {
            egui::Panel::left("browser_panel")
                .exact_size(180.0)
                .resizable(true)
                .show(ui, |ui| {
                    let action = self.browser.render(
                        ui,
                        self.project.track_mut(self.selected_track),
                        &mut self.ids,
                        &self.palette,
                    );

                    if let Some(dev) = action.add_device
                        && let Some(trk) = self.project.track_mut(self.selected_track)
                    {
                        trk.device_chain.push(dev);
                        self.status_message = "Device added to track".to_string();
                    }

                    if let Some(ttype) = action.add_track {
                        let number = self.project.tracks.len() + 1;
                        let name = match ttype {
                            TrackType::Audio => format!("{}-Audio", number),
                            TrackType::Midi => format!("{}-MIDI", number),
                            _ => format!("Track-{}", number),
                        };
                        self.project
                            .tracks
                            .push(Track::new(self.ids.track(), name, ttype, 8));
                        self.status_message = "New track added".to_string();
                    }
                });
        }

        // Center Work Area: Session View or Arrangement View
        egui::CentralPanel::default().show(ui, |ui| {
            match self.view_mode {
                ViewMode::Session => {
                    let sess_action = render_session_view(
                        ui,
                        &mut self.project,
                        self.selected_track,
                        &active_slots,
                        &track_meters,
                        &master_meter,
                        playhead_beats,
                        &mut self.ids,
                        &self.palette,
                    );

                    if let Some((t, s)) = sess_action.trigger_clip
                        && let Some(ref eng) = self.engine
                    {
                        eng.trigger_clip(t, s);
                        eng.play();
                    }
                    if let Some(t) = sess_action.stop_clip
                        && let Some(ref eng) = self.engine
                    {
                        eng.stop_clip(t);
                    }
                    if let Some(sc) = sess_action.trigger_scene
                        && let Some(ref eng) = self.engine
                    {
                        eng.trigger_scene(sc);
                        eng.play();
                    }
                    if sess_action.stop_all_clips
                        && let Some(ref eng) = self.engine
                    {
                        eng.stop();
                    }
                    if let Some(t) = sess_action.select_track {
                        self.selected_track = t;
                    }
                    if let Some(c) = sess_action.select_clip {
                        self.selected_clip = Some(c);
                        self.bottom_tab = 1; // Switch to clip view
                    }
                    if let Some(ttype) = sess_action.add_track {
                        let number = self.project.tracks.len() + 1;
                        let name = match ttype {
                            TrackType::Audio => format!("{}-Audio", number),
                            TrackType::Midi => format!("{}-MIDI", number),
                            _ => format!("Track-{}", number),
                        };
                        self.project
                            .tracks
                            .push(Track::new(self.ids.track(), name, ttype, 8));
                        self.status_message = format!("Added new {:?} track", ttype);
                    }
                }
                ViewMode::Arrangement => {
                    let arr_action = render_arrangement_view(
                        ui,
                        &mut self.project,
                        self.selected_track,
                        playhead_beats,
                        &self.palette,
                    );

                    if let Some(beats) = arr_action.set_playhead
                        && let Some(ref eng) = self.engine
                    {
                        eng.set_playhead(beats);
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

        // Dispatch any updates from CentralPanel / Mixer interaction
        self.sync_audio_parameters();
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
