use crate::mixer::{
    render_master_mixer_strip, render_return_mixer_strip, render_track_mixer_strip,
};
use crate::style::LivePalette;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use livecraft_engine::graph::TrackMeter;
use livecraft_formats::theme::parse_hex_color;
use livecraft_model::ids::{IdAllocator, SceneId, TrackId};
use livecraft_model::model::{Clip, Project, TrackType};

#[derive(Default)]
pub struct SessionAction {
    pub trigger_clip: Option<(TrackId, usize)>, // (track, slot_idx)
    pub stop_clip: Option<TrackId>,             // track
    pub trigger_scene: Option<SceneId>,         // scene
    pub stop_all_clips: bool,
    pub select_track: Option<TrackId>,
    pub select_clip: Option<(TrackId, usize)>, // (track, slot_idx)
    pub add_track: Option<TrackType>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_session_view(
    ui: &mut Ui,
    project: &mut Project,
    selected_track: TrackId,
    active_clip_slots: &[Option<usize>],
    track_meters: &[TrackMeter],
    master_meter: &TrackMeter,
    playhead_beats: f64,
    ids: &mut IdAllocator,
    palette: &LivePalette,
) -> SessionAction {
    let mut action = SessionAction::default();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                let track_width = 104.0;
                let return_width = 86.0;
                let master_width = 84.0;
                let num_scenes = project.scenes.len().max(8);
                let is_dark = palette.is_dark();

                // -------------------------------------------------------------
                // 1. Player Tracks (Audio and MIDI)
                // -------------------------------------------------------------
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let track_id = trk.id;
                    let is_selected = track_id == selected_track;
                    let active_slot = active_clip_slots.get(trk_idx).copied().flatten();
                    let meter = track_meters.get(trk_idx).cloned().unwrap_or_default();

                    ui.vertical(|ui| {
                        ui.set_width(track_width);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                        // A. Track Header
                        let (hdr_rect, hdr_resp) =
                            ui.allocate_exact_size(Vec2::new(track_width, 24.0), Sense::click());
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            let bg_color = if is_selected {
                                palette.surface_highlight
                            } else {
                                palette.button_idle
                            };

                            painter.rect_filled(hdr_rect, CornerRadius::same(2), bg_color);
                            let border_stroke = if is_selected {
                                Stroke::new(1.5_f32, palette.amber)
                            } else {
                                Stroke::new(1.0_f32, palette.panel_border)
                            };
                            painter.rect_stroke(
                                hdr_rect,
                                CornerRadius::same(2),
                                border_stroke,
                                eframe::egui::StrokeKind::Middle,
                            );

                            // Track color accent bar on top edge of header
                            let [r, g, b, _] = parse_hex_color(&trk.color);
                            let trk_color = Color32::from_rgb(r, g, b);
                            let color_bar = Rect::from_min_size(
                                Pos2::new(hdr_rect.min.x + 1.0, hdr_rect.min.y + 1.0),
                                Vec2::new(hdr_rect.width() - 2.0, 3.0),
                            );
                            painter.rect_filled(color_bar, CornerRadius::ZERO, trk_color);

                            // Track Number and Name
                            let text_color = if is_selected {
                                if is_dark {
                                    Color32::WHITE
                                } else {
                                    Color32::BLACK
                                }
                            } else {
                                palette.control_foreground
                            };
                            painter.text(
                                Pos2::new(hdr_rect.center().x, hdr_rect.center().y + 1.0),
                                egui::Align2::CENTER_CENTER,
                                &trk.name,
                                egui::FontId::proportional(11.0),
                                text_color,
                            );
                        }

                        if hdr_resp.clicked() {
                            action.select_track = Some(track_id);
                        }

                        // B. Clip Slots Matrix (Scenes 1..num_scenes)
                        for slot_idx in 0..num_scenes {
                            let slot_clip =
                                trk.clip_slots.get(slot_idx).and_then(|s| s.clip.as_ref());
                            let is_slot_playing =
                                active_slot == Some(slot_idx) && project.transport.is_playing;

                            let (slot_rect, slot_resp) = ui
                                .allocate_exact_size(Vec2::new(track_width, 22.0), Sense::click());
                            let painter = ui.painter();

                            if let Some(clip) = slot_clip {
                                // Filled clip slot
                                let [r, g, b, _] = parse_hex_color(&clip.color);
                                let clip_color = Color32::from_rgb(r, g, b);

                                let slot_bg = if is_slot_playing {
                                    if is_dark {
                                        Color32::from_rgb(0x1a, 0x44, 0x24)
                                    } else {
                                        Color32::from_rgb(0xb0, 0xe8, 0xba)
                                    }
                                } else if slot_resp.hovered() {
                                    if is_dark {
                                        Color32::from_rgb(
                                            (r as f32 * 0.35 + 35.0) as u8,
                                            (g as f32 * 0.35 + 35.0) as u8,
                                            (b as f32 * 0.35 + 35.0) as u8,
                                        )
                                    } else {
                                        Color32::from_rgb(
                                            (r as f32 * 0.4 + 140.0) as u8,
                                            (g as f32 * 0.4 + 140.0) as u8,
                                            (b as f32 * 0.4 + 140.0) as u8,
                                        )
                                    }
                                } else {
                                    if is_dark {
                                        Color32::from_rgb(
                                            (r as f32 * 0.25 + 24.0) as u8,
                                            (g as f32 * 0.25 + 24.0) as u8,
                                            (b as f32 * 0.25 + 24.0) as u8,
                                        )
                                    } else {
                                        Color32::from_rgb(
                                            (r as f32 * 0.3 + 160.0) as u8,
                                            (g as f32 * 0.3 + 160.0) as u8,
                                            (b as f32 * 0.3 + 160.0) as u8,
                                        )
                                    }
                                };

                                painter.rect_filled(slot_rect, CornerRadius::same(2), slot_bg);
                                painter.rect_stroke(
                                    slot_rect,
                                    CornerRadius::same(2),
                                    Stroke::new(
                                        1.0_f32,
                                        if is_slot_playing {
                                            palette.play_green
                                        } else {
                                            palette.clip_slot_border
                                        },
                                    ),
                                    eframe::egui::StrokeKind::Middle,
                                );

                                // Left solid clip color indicator bar
                                let pill_rect = Rect::from_min_size(
                                    Pos2::new(slot_rect.min.x + 1.0, slot_rect.min.y + 1.0),
                                    Vec2::new(4.0, slot_rect.height() - 2.0),
                                );
                                painter.rect_filled(pill_rect, CornerRadius::same(1), clip_color);

                                // Playhead progress highlight during playback
                                if is_slot_playing {
                                    let prog = ((playhead_beats % clip.length_beats.max(1.0))
                                        / clip.length_beats.max(1.0))
                                        as f32;
                                    let fill_w = (slot_rect.width() - 8.0) * prog.clamp(0.0, 1.0);
                                    let prog_rect = Rect::from_min_size(
                                        Pos2::new(slot_rect.min.x + 6.0, slot_rect.min.y + 1.0),
                                        Vec2::new(fill_w, slot_rect.height() - 2.0),
                                    );
                                    painter.rect_filled(
                                        prog_rect,
                                        CornerRadius::ZERO,
                                        Color32::from_rgba_unmultiplied(r, g, b, 70),
                                    );
                                }

                                // Launch Triangle
                                let arrow_c =
                                    Pos2::new(slot_rect.min.x + 14.0, slot_rect.center().y);
                                let arrow_color = if is_slot_playing {
                                    palette.play_green
                                } else if is_dark {
                                    Color32::from_rgb(0xd0, 0xd0, 0xd0)
                                } else {
                                    Color32::from_rgb(0x20, 0x20, 0x20)
                                };
                                let p1 = Pos2::new(arrow_c.x - 3.5, arrow_c.y - 4.5);
                                let p2 = Pos2::new(arrow_c.x - 3.5, arrow_c.y + 4.5);
                                let p3 = Pos2::new(arrow_c.x + 4.5, arrow_c.y);
                                painter.add(egui::Shape::convex_polygon(
                                    vec![p1, p2, p3],
                                    arrow_color,
                                    Stroke::NONE,
                                ));

                                // Clip Name
                                let name_color = if is_slot_playing {
                                    if is_dark {
                                        Color32::WHITE
                                    } else {
                                        Color32::BLACK
                                    }
                                } else {
                                    palette.control_foreground
                                };
                                painter.text(
                                    Pos2::new(slot_rect.min.x + 23.0, slot_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &clip.name,
                                    egui::FontId::proportional(10.5),
                                    name_color,
                                );

                                if slot_resp.clicked() {
                                    action.trigger_clip = Some((track_id, slot_idx));
                                    action.select_track = Some(track_id);
                                    action.select_clip = Some((track_id, slot_idx));
                                }
                            } else {
                                // Empty clip slot
                                let bg_color = if slot_resp.hovered() {
                                    palette.button_hover
                                } else {
                                    palette.clip_slot_bg
                                };
                                painter.rect_filled(slot_rect, CornerRadius::same(2), bg_color);
                                painter.rect_stroke(
                                    slot_rect,
                                    CornerRadius::same(2),
                                    Stroke::new(1.0_f32, palette.clip_slot_border),
                                    eframe::egui::StrokeKind::Middle,
                                );

                                // Ableton stop/placeholder square in center-left
                                let sq_center =
                                    Pos2::new(slot_rect.min.x + 13.0, slot_rect.center().y);
                                let sq_rect =
                                    Rect::from_center_size(sq_center, Vec2::new(6.0, 6.0));
                                let sq_color = if is_dark {
                                    Color32::from_rgb(0x44, 0x44, 0x44)
                                } else {
                                    Color32::from_rgb(0x9a, 0x9a, 0x98)
                                };
                                painter.rect_filled(sq_rect, CornerRadius::same(1), sq_color);

                                if slot_resp.double_clicked() {
                                    let new_clip = if trk.track_type == TrackType::Midi {
                                        Clip::new_midi(
                                            ids.clip(),
                                            format!("Clip {}", slot_idx + 1),
                                            4.0,
                                        )
                                    } else {
                                        Clip::new_audio(
                                            ids.clip(),
                                            format!("Audio {}", slot_idx + 1),
                                            "",
                                            4.0,
                                        )
                                    };
                                    if slot_idx < trk.clip_slots.len() {
                                        trk.clip_slots[slot_idx].clip = Some(new_clip);
                                    }
                                }
                            }
                        }

                        // C. Track Stop Clip Button
                        let (stop_rect, stop_resp) =
                            ui.allocate_exact_size(Vec2::new(track_width, 18.0), Sense::click());
                        if ui.is_rect_visible(stop_rect) {
                            let painter = ui.painter();
                            let bg = if stop_resp.hovered() {
                                palette.button_hover
                            } else {
                                palette.button_idle
                            };
                            painter.rect_filled(stop_rect, CornerRadius::same(2), bg);
                            painter.rect_stroke(
                                stop_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, palette.panel_border),
                                eframe::egui::StrokeKind::Middle,
                            );

                            let c = stop_rect.center();
                            let sq = Rect::from_center_size(c, Vec2::new(7.0, 7.0));
                            let stop_sq_col = if is_dark {
                                Color32::from_rgb(0x88, 0x88, 0x88)
                            } else {
                                Color32::from_rgb(0x40, 0x40, 0x40)
                            };
                            painter.rect_filled(sq, CornerRadius::ZERO, stop_sq_col);
                        }
                        if stop_resp.clicked() {
                            action.stop_clip = Some(track_id);
                        }

                        ui.add_space(2.0);

                        // D. Track Mixer Strip (unified directly underneath!)
                        render_track_mixer_strip(ui, trk, trk_idx, &meter, track_width, palette);
                    });
                }

                // -------------------------------------------------------------
                // 2. Return Tracks (A-Reverb, B-Delay)
                // -------------------------------------------------------------
                for (ret_idx, ret) in project.return_tracks.iter_mut().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_width(return_width);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                        // Return Header
                        let (hdr_rect, _) =
                            ui.allocate_exact_size(Vec2::new(return_width, 24.0), Sense::hover());
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            painter.rect_filled(
                                hdr_rect,
                                CornerRadius::same(2),
                                palette.button_idle,
                            );
                            painter.rect_stroke(
                                hdr_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, palette.panel_border),
                                eframe::egui::StrokeKind::Middle,
                            );

                            let [r, g, b, _] = parse_hex_color(&ret.color);
                            let color_bar = Rect::from_min_size(
                                Pos2::new(hdr_rect.min.x + 1.0, hdr_rect.min.y + 1.0),
                                Vec2::new(hdr_rect.width() - 2.0, 3.0),
                            );
                            painter.rect_filled(
                                color_bar,
                                CornerRadius::ZERO,
                                Color32::from_rgb(r, g, b),
                            );

                            painter.text(
                                Pos2::new(hdr_rect.center().x, hdr_rect.center().y + 1.0),
                                egui::Align2::CENTER_CENTER,
                                &ret.name,
                                egui::FontId::proportional(10.5),
                                palette.control_foreground,
                            );
                        }

                        // Empty slots for Return tracks (visual row alignment)
                        for _ in 0..num_scenes {
                            let (slot_rect, _) = ui
                                .allocate_exact_size(Vec2::new(return_width, 22.0), Sense::hover());
                            if ui.is_rect_visible(slot_rect) {
                                let painter = ui.painter();
                                painter.rect_filled(
                                    slot_rect,
                                    CornerRadius::same(2),
                                    palette.clip_slot_bg,
                                );
                                painter.rect_stroke(
                                    slot_rect,
                                    CornerRadius::same(2),
                                    Stroke::new(1.0_f32, palette.clip_slot_border),
                                    eframe::egui::StrokeKind::Middle,
                                );
                            }
                        }

                        // Spacer for stop button row
                        ui.allocate_exact_size(Vec2::new(return_width, 18.0), Sense::hover());

                        ui.add_space(2.0);

                        // Return Mixer Strip
                        render_return_mixer_strip(ui, ret, ret_idx, return_width, palette);
                    });
                }

                // -------------------------------------------------------------
                // 3. Master / Scene Launch Column (Manual p. 190)
                // -------------------------------------------------------------
                ui.vertical(|ui| {
                    ui.set_width(master_width);
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                    // Master Header
                    let (m_rect, _) =
                        ui.allocate_exact_size(Vec2::new(master_width, 24.0), Sense::hover());
                    if ui.is_rect_visible(m_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(m_rect, CornerRadius::same(2), palette.button_idle);
                        painter.rect_stroke(
                            m_rect,
                            CornerRadius::same(2),
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );

                        // Master color bar
                        let m_bar = Rect::from_min_size(
                            Pos2::new(m_rect.min.x + 1.0, m_rect.min.y + 1.0),
                            Vec2::new(m_rect.width() - 2.0, 3.0),
                        );
                        painter.rect_filled(
                            m_bar,
                            CornerRadius::ZERO,
                            Color32::from_rgb(0x80, 0xe0, 0x90),
                        );

                        painter.text(
                            Pos2::new(m_rect.center().x, m_rect.center().y + 1.0),
                            egui::Align2::CENTER_CENTER,
                            "Master",
                            egui::FontId::proportional(11.0),
                            palette.control_foreground,
                        );
                    }

                    // Scene Launch Buttons (aligned with each clip slot row)
                    for scene_idx in 0..num_scenes {
                        let (sc_rect, sc_resp) =
                            ui.allocate_exact_size(Vec2::new(master_width, 22.0), Sense::click());
                        let painter = ui.painter();
                        let bg = if sc_resp.hovered() {
                            palette.button_hover
                        } else {
                            palette.button_idle
                        };
                        painter.rect_filled(sc_rect, CornerRadius::same(2), bg);
                        painter.rect_stroke(
                            sc_rect,
                            CornerRadius::same(2),
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );

                        // Scene number and launch arrow
                        painter.text(
                            Pos2::new(sc_rect.min.x + 8.0, sc_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", scene_idx + 1),
                            egui::FontId::proportional(11.0),
                            palette.control_foreground,
                        );

                        // Launch green triangle
                        let arrow_c = Pos2::new(sc_rect.max.x - 14.0, sc_rect.center().y);
                        let p1 = Pos2::new(arrow_c.x - 3.5, arrow_c.y - 4.5);
                        let p2 = Pos2::new(arrow_c.x - 3.5, arrow_c.y + 4.5);
                        let p3 = Pos2::new(arrow_c.x + 4.5, arrow_c.y);
                        painter.add(egui::Shape::convex_polygon(
                            vec![p1, p2, p3],
                            palette.play_green,
                            Stroke::NONE,
                        ));

                        if sc_resp.clicked() {
                            action.trigger_scene =
                                project.scenes.get(scene_idx).map(|scene| scene.id);
                        }
                    }

                    // Stop All Clips Button (aligned with track stop row)
                    let (sa_rect, sa_resp) =
                        ui.allocate_exact_size(Vec2::new(master_width, 18.0), Sense::click());
                    if ui.is_rect_visible(sa_rect) {
                        let painter = ui.painter();
                        let bg = if sa_resp.hovered() {
                            palette.button_hover
                        } else {
                            palette.button_idle
                        };
                        painter.rect_filled(sa_rect, CornerRadius::same(2), bg);
                        painter.rect_stroke(
                            sa_rect,
                            CornerRadius::same(2),
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );
                        painter.text(
                            sa_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Stop All",
                            egui::FontId::proportional(10.0),
                            palette.amber,
                        );
                    }
                    if sa_resp.clicked() {
                        action.stop_all_clips = true;
                    }

                    ui.add_space(2.0);

                    // Master Track Mixer Strip
                    render_master_mixer_strip(
                        ui,
                        &mut project.master_track,
                        master_meter,
                        master_width,
                        palette,
                    );
                });

                // -------------------------------------------------------------
                // 4. Quick "+ Track" column at the end
                // -------------------------------------------------------------
                ui.vertical(|ui| {
                    ui.set_width(76.0);
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                    let (add_hdr_rect, _) =
                        ui.allocate_exact_size(Vec2::new(76.0, 24.0), Sense::hover());
                    if ui.is_rect_visible(add_hdr_rect) {
                        ui.painter().text(
                            add_hdr_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+ Track",
                            egui::FontId::proportional(10.0),
                            palette.control_disabled,
                        );
                    }

                    let midi_btn = crate::widgets::live_toggle_button(
                        ui,
                        false,
                        palette.amber,
                        "+ MIDI",
                        Vec2::new(76.0, 22.0),
                    );
                    if midi_btn.clicked() {
                        action.add_track = Some(TrackType::Midi);
                    }

                    let audio_btn = crate::widgets::live_toggle_button(
                        ui,
                        false,
                        palette.cyan,
                        "+ Audio",
                        Vec2::new(76.0, 22.0),
                    );
                    if audio_btn.clicked() {
                        action.add_track = Some(TrackType::Audio);
                    }
                });
            });
        });

    action
}
