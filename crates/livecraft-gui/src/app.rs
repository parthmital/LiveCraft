use crate::arrange::render_arrangement_view;
use crate::browser::LiveBrowser;
use crate::devices::render_device_chain;
use crate::mixer::render_mixer_section;
use crate::pianoroll::render_piano_roll;
use crate::session::render_session_view;
use crate::style::{apply_live_style, LivePalette};
use crate::transport::{render_transport_bar, ViewMode};
use eframe::egui::{self, Color32, Key};
use livecraft_core::als::AlsParser;
use livecraft_core::command::CommandHistory;
use livecraft_core::model::{Clip, ClipPayload, MidiNote, Project, Track, TrackType};
use livecraft_dsp::engine::AudioEngine;
use livecraft_dsp::graph::TrackMeter;
use std::path::{Path, PathBuf};

pub struct LiveCraftApp {
    pub project: Project,
    pub project_path: Option<PathBuf>,
    pub engine: Option<AudioEngine>,
    pub command_history: CommandHistory,
    pub view_mode: ViewMode,
    pub selected_track_idx: usize,
    pub selected_clip: Option<(usize, usize)>, // (track_idx, slot_idx)
    pub bottom_tab: usize,                     // 0 = Device Chain, 1 = Piano Roll / Clip View
    pub browser: LiveBrowser,
    pub palette: LivePalette,
    pub status_message: String,
}

impl LiveCraftApp {
    pub fn new(project_path: Option<&Path>) -> Self {
        let palette = LivePalette::default();

        let (project, path_buf) = if let Some(p) = project_path {
            if let Ok(proj) = AlsParser::load_project(p) {
                (proj, Some(p.to_path_buf()))
            } else {
                (Self::create_demo_project(), None)
            }
        } else {
            (Self::create_demo_project(), None)
        };

        // Try initializing audio engine
        let engine = AudioEngine::new(&project).ok();

        Self {
            project,
            project_path: path_buf,
            engine,
            command_history: CommandHistory::default(),
            view_mode: ViewMode::Session,
            selected_track_idx: 0,
            selected_clip: Some((0, 0)),
            bottom_tab: 0,
            browser: LiveBrowser::default(),
            palette,
            status_message: "LiveCraft Ready".to_string(),
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
        if ctx.input(|i| i.key_pressed(Key::Tab)) {
            self.view_mode = match self.view_mode {
                ViewMode::Session => ViewMode::Arrangement,
                ViewMode::Arrangement => ViewMode::Session,
            };
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
    }
}

impl eframe::App for LiveCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_live_style(ctx, &self.palette);
        self.handle_keyboard_shortcuts(ctx);

        // Keep audio engine synchronized with project model in real-time
        if let Some(ref eng) = self.engine {
            eng.sync_project(&self.project);
        }

        // Continuous repaint during audio playback
        if self.project.transport.is_playing {
            ctx.request_repaint();
        }

        // Query real-time playhead and meters from engine
        let (playhead_beats, track_meters, master_meter, active_slots) = if let Some(ref eng) =
            self.engine
        {
            if let Ok(g) = eng.graph.lock() {
                let t_meters = g.track_states.iter().map(|s| s.meter.clone()).collect();
                let m_meter = g.master_meter.clone();
                let a_slots: Vec<_> = g.track_states.iter().map(|s| s.active_clip_slot).collect();
                (g.playhead_beats, t_meters, m_meter, a_slots)
            } else {
                (
                    self.project.transport.playhead_beats,
                    Vec::new(),
                    TrackMeter::default(),
                    Vec::new(),
                )
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
        egui::TopBottomPanel::top("transport_bar")
            .exact_height(34.0)
            .show(ctx, |ui| {
                let t_resp = render_transport_bar(
                    ui,
                    &mut self.project,
                    self.view_mode,
                    &self.palette,
                    2.5, // CPU percent
                );

                if t_resp.play_toggled {
                    if let Some(ref eng) = self.engine {
                        if self.project.transport.is_playing {
                            eng.play();
                        } else {
                            eng.stop();
                        }
                    }
                }
                if t_resp.stop_clicked {
                    if let Some(ref eng) = self.engine {
                        eng.stop();
                    }
                }
                if let Some(mode) = t_resp.view_mode_changed {
                    self.view_mode = mode;
                }
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(20.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(10.0)
                            .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("LiveCraft v0.1.0 | 44.1 kHz 24-bit")
                                .size(10.0)
                                .color(Color32::from_rgb(0x66, 0x66, 0x66)),
                        );
                    });
                });
            });

        // Bottom Detail Panel: Device Chain or Piano Roll
        egui::TopBottomPanel::bottom("detail_panel")
            .exact_height(170.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Tab selector: Devices vs Clip Detail
                    let dev_btn = ui.selectable_label(self.bottom_tab == 0, "Device View");
                    let clip_btn = ui.selectable_label(self.bottom_tab == 1, "Clip View / Piano Roll");
                    if dev_btn.clicked() { self.bottom_tab = 0; }
                    if clip_btn.clicked() { self.bottom_tab = 1; }

                    if let Some(trk) = self.project.tracks.get(self.selected_track_idx) {
                        ui.separator();
                        ui.label(egui::RichText::new(&trk.name).strong().size(11.0).color(self.palette.amber));
                    }
                });

                ui.separator();

                if self.bottom_tab == 0 {
                    // Device Chain View
                    if let Some(trk) = self.project.tracks.get_mut(self.selected_track_idx) {
                        render_device_chain(ui, trk, &self.palette);
                    }
                } else {
                    // Clip View / Piano Roll
                    if let Some((t_idx, s_idx)) = self.selected_clip {
                        let scale = &mut self.project.scale;
                        if let Some(trk) = self.project.tracks.get_mut(t_idx) {
                            if let Some(slot) = trk.clip_slots.get_mut(s_idx) {
                                if let Some(ref mut clip) = slot.clip {
                                    render_piano_roll(
                                        ui,
                                        clip,
                                        scale,
                                        playhead_beats,
                                        &self.palette,
                                    );
                                } else {
                                    ui.label("Empty slot. Double-click in Session View to create a clip.");
                                }
                            }
                        }
                    } else {
                        ui.label("Select a clip to view notes");
                    }
                }
            });

        // Left Browser Sidebar
        egui::SidePanel::left("browser_panel")
            .exact_width(170.0)
            .resizable(true)
            .show(ctx, |ui| {
                let action = self.browser.render(
                    ui,
                    self.project.tracks.get_mut(self.selected_track_idx),
                    &self.palette,
                );

                if let Some(dev) = action.add_device {
                    if let Some(trk) = self.project.tracks.get_mut(self.selected_track_idx) {
                        trk.device_chain.push(dev);
                        self.status_message = "Device added to track".to_string();
                    }
                }

                if let Some(ttype) = action.add_track {
                    let new_id = self.project.tracks.len() as u64 + 1;
                    let name = match ttype {
                        TrackType::Audio => format!("{}-Audio", new_id),
                        TrackType::Midi => format!("{}-MIDI", new_id),
                        _ => format!("Track-{}", new_id),
                    };
                    self.project.tracks.push(Track::new(new_id, name, ttype, 8));
                    self.status_message = "New track added".to_string();
                }
            });

        // Center Work Area: Session View or Arrangement View
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.view_mode {
                ViewMode::Session => {
                    ui.vertical(|ui| {
                        // Top part of CentralPanel: Clip Matrix
                        let sess_action = render_session_view(
                            ui,
                            &mut self.project,
                            self.selected_track_idx,
                            &active_slots,
                            playhead_beats,
                            &self.palette,
                        );

                        if let Some((t, s)) = sess_action.trigger_clip {
                            if let Some(ref eng) = self.engine {
                                eng.trigger_clip(t, s);
                                eng.play();
                            }
                        }
                        if let Some(t) = sess_action.stop_clip {
                            if let Some(ref eng) = self.engine {
                                if let Ok(mut g) = eng.graph.lock() {
                                    g.stop_clip(t);
                                }
                            }
                        }
                        if let Some(sc) = sess_action.trigger_scene {
                            if let Some(ref eng) = self.engine {
                                if let Ok(mut g) = eng.graph.lock() {
                                    g.trigger_scene(sc, &self.project);
                                    g.is_playing = true;
                                }
                            }
                        }
                        if sess_action.stop_all_clips {
                            if let Some(ref eng) = self.engine {
                                eng.stop();
                            }
                        }
                        if let Some(t) = sess_action.select_track {
                            self.selected_track_idx = t;
                        }
                        if let Some(c) = sess_action.select_clip {
                            self.selected_clip = Some(c);
                            self.bottom_tab = 1; // Switch to clip view
                        }

                        ui.separator();

                        // Bottom part of Session View: Mixer Faders & VU meters
                        render_mixer_section(
                            ui,
                            &mut self.project,
                            &track_meters,
                            &master_meter,
                            &self.palette,
                        );
                    });
                }
                ViewMode::Arrangement => {
                    let arr_action = render_arrangement_view(
                        ui,
                        &mut self.project,
                        self.selected_track_idx,
                        playhead_beats,
                        &self.palette,
                    );

                    if let Some(beats) = arr_action.set_playhead {
                        if let Some(ref eng) = self.engine {
                            if let Ok(mut g) = eng.graph.lock() {
                                g.playhead_beats = beats;
                            }
                        }
                    }
                    if let Some(t) = arr_action.select_track {
                        self.selected_track_idx = t;
                    }
                    if let Some(c) = arr_action.select_clip {
                        self.status_message = format!("Selected arrangement clip #{}", c + 1);
                    }
                }
            }
        });
    }
}
