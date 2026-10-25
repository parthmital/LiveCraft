use crate::style::LivePalette;
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::{Clip, Project, TrackType};
use livecraft_core::theme::parse_hex_color;

#[derive(Default)]
pub struct SessionAction {
    pub trigger_clip: Option<(usize, usize)>, // (track_idx, slot_idx)
    pub stop_clip: Option<usize>,             // track_idx
    pub trigger_scene: Option<usize>,         // scene_idx
    pub stop_all_clips: bool,
    pub select_track: Option<usize>,
    pub select_clip: Option<(usize, usize)>, // (track_idx, slot_idx)
}

pub fn render_session_view(
    ui: &mut Ui,
    project: &mut Project,
    selected_track_idx: usize,
    active_clip_slots: &[Option<usize>],
    playhead_beats: f64,
    palette: &LivePalette,
) -> SessionAction {
    let mut action = SessionAction::default();

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                let track_width = 96.0;
                let num_scenes = project.scenes.len().max(8);

                // -------------------------------------------------------------
                // Player Tracks (Audio and MIDI)
                // -------------------------------------------------------------
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let is_selected = trk_idx == selected_track_idx;
                    let active_slot = active_clip_slots.get(trk_idx).copied().flatten();

                    ui.vertical(|ui| {
                        ui.set_width(track_width);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                        // 1. Track Header
                        let (hdr_rect, hdr_resp) =
                            ui.allocate_exact_size(Vec2::new(track_width, 24.0), Sense::click());
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            let bg_color = if is_selected {
                                palette.surface_highlight
                            } else {
                                Color32::from_rgb(0x2d, 0x2d, 0x2d)
                            };

                            painter.rect_filled(hdr_rect, Rounding::same(2.0), bg_color);
                            let border_stroke = if is_selected {
                                Stroke::new(1.5_f32, palette.amber)
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38))
                            };
                            painter.rect_stroke(hdr_rect, Rounding::same(2.0), border_stroke);

                            // Track color accent bar on bottom edge of header
                            let [r, g, b, _] = parse_hex_color(&trk.color);
                            let trk_color = Color32::from_rgb(r, g, b);
                            let color_rect = Rect::from_min_size(
                                Pos2::new(hdr_rect.min.x + 1.0, hdr_rect.max.y - 3.0),
                                Vec2::new(hdr_rect.width() - 2.0, 2.0),
                            );
                            painter.rect_filled(color_rect, Rounding::ZERO, trk_color);

                            // Track Number and Name
                            painter.text(
                                Pos2::new(hdr_rect.center().x, hdr_rect.center().y - 1.0),
                                egui::Align2::CENTER_CENTER,
                                &trk.name,
                                egui::FontId::proportional(11.0),
                                if is_selected {
                                    Color32::WHITE
                                } else {
                                    palette.control_foreground
                                },
                            );
                        }

                        if hdr_resp.clicked() {
                            action.select_track = Some(trk_idx);
                        }

                        // 2. Clip Slots Matrix
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
                                    Color32::from_rgb(0x18, 0x33, 0x1e)
                                } else if slot_resp.hovered() {
                                    Color32::from_rgb(0x35, 0x35, 0x35)
                                } else {
                                    Color32::from_rgb(0x27, 0x27, 0x27)
                                };

                                painter.rect_filled(slot_rect, Rounding::same(2.0), slot_bg);
                                painter.rect_stroke(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Stroke::new(
                                        1.0_f32,
                                        if is_slot_playing {
                                            palette.play_green
                                        } else {
                                            Color32::from_rgb(0x3c, 0x3c, 0x3c)
                                        },
                                    ),
                                );

                                // Left solid clip color pill
                                let pill_rect = Rect::from_min_size(
                                    Pos2::new(slot_rect.min.x + 1.0, slot_rect.min.y + 1.0),
                                    Vec2::new(4.0, slot_rect.height() - 2.0),
                                );
                                painter.rect_filled(pill_rect, Rounding::same(1.0), clip_color);

                                // Playhead progress highlight during playback
                                if is_slot_playing {
                                    let prog = ((playhead_beats % clip.length_beats.max(1.0))
                                        / clip.length_beats.max(1.0))
                                        as f32;
                                    let fill_w = (slot_rect.width() - 8.0) * prog;
                                    let prog_rect = Rect::from_min_size(
                                        Pos2::new(slot_rect.min.x + 6.0, slot_rect.min.y + 1.0),
                                        Vec2::new(fill_w, slot_rect.height() - 2.0),
                                    );
                                    painter.rect_filled(
                                        prog_rect,
                                        Rounding::ZERO,
                                        Color32::from_rgba_unmultiplied(r, g, b, 45),
                                    );
                                }

                                // Launch Triangle
                                let arrow_c =
                                    Pos2::new(slot_rect.min.x + 13.0, slot_rect.center().y);
                                let arrow_color = if is_slot_playing {
                                    palette.play_green
                                } else {
                                    Color32::from_rgb(0x90, 0x90, 0x90)
                                };
                                let p1 = Pos2::new(arrow_c.x - 3.0, arrow_c.y - 4.5);
                                let p2 = Pos2::new(arrow_c.x - 3.0, arrow_c.y + 4.5);
                                let p3 = Pos2::new(arrow_c.x + 4.0, arrow_c.y);
                                painter.add(egui::Shape::convex_polygon(
                                    vec![p1, p2, p3],
                                    arrow_color,
                                    Stroke::NONE,
                                ));

                                // Clip Name
                                painter.text(
                                    Pos2::new(slot_rect.min.x + 21.0, slot_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &clip.name,
                                    egui::FontId::proportional(10.5),
                                    if is_slot_playing {
                                        Color32::WHITE
                                    } else {
                                        palette.control_foreground
                                    },
                                );

                                if slot_resp.clicked() {
                                    action.trigger_clip = Some((trk_idx, slot_idx));
                                    action.select_track = Some(trk_idx);
                                    action.select_clip = Some((trk_idx, slot_idx));
                                }
                            } else {
                                // Empty clip slot
                                let bg_color = if slot_resp.hovered() {
                                    Color32::from_rgb(0x2a, 0x2a, 0x2a)
                                } else {
                                    Color32::from_rgb(0x20, 0x20, 0x20)
                                };
                                painter.rect_filled(slot_rect, Rounding::same(2.0), bg_color);
                                painter.rect_stroke(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Stroke::new(1.0_f32, Color32::from_rgb(0x2c, 0x2c, 0x2c)),
                                );

                                // Ableton circular stop/placeholder dot
                                painter.circle_filled(
                                    Pos2::new(slot_rect.min.x + 12.0, slot_rect.center().y),
                                    2.5,
                                    Color32::from_rgb(0x40, 0x40, 0x40),
                                );

                                if slot_resp.double_clicked() {
                                    let new_clip = if trk.track_type == TrackType::Midi {
                                        Clip::new_midi(
                                            trk.id * 100 + slot_idx as u64,
                                            format!("Clip {}", slot_idx + 1),
                                            4.0,
                                        )
                                    } else {
                                        Clip::new_audio(
                                            trk.id * 100 + slot_idx as u64,
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

                        // 3. Track Stop Clip Button
                        let (stop_rect, stop_resp) =
                            ui.allocate_exact_size(Vec2::new(track_width, 18.0), Sense::click());
                        if ui.is_rect_visible(stop_rect) {
                            let painter = ui.painter();
                            let bg = if stop_resp.hovered() {
                                palette.button_hover
                            } else {
                                palette.control_background
                            };
                            painter.rect_filled(stop_rect, Rounding::same(2.0), bg);
                            painter.rect_stroke(
                                stop_rect,
                                Rounding::same(2.0),
                                Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
                            );

                            let c = stop_rect.center();
                            let sq = Rect::from_center_size(c, Vec2::new(7.0, 7.0));
                            painter.rect_filled(
                                sq,
                                Rounding::ZERO,
                                Color32::from_rgb(0x88, 0x88, 0x88),
                            );
                        }
                        if stop_resp.clicked() {
                            action.stop_clip = Some(trk_idx);
                        }
                    });
                }

                // -------------------------------------------------------------
                // Return Tracks (A Reverb, B Delay)
                // -------------------------------------------------------------
                for ret in project.return_tracks.iter_mut() {
                    ui.vertical(|ui| {
                        ui.set_width(track_width * 0.75);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                        let (hdr_rect, _) = ui.allocate_exact_size(
                            Vec2::new(track_width * 0.75, 24.0),
                            Sense::hover(),
                        );
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            painter.rect_filled(
                                hdr_rect,
                                Rounding::same(2.0),
                                Color32::from_rgb(0x27, 0x27, 0x27),
                            );
                            painter.rect_stroke(
                                hdr_rect,
                                Rounding::same(2.0),
                                Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38)),
                            );

                            let [r, g, b, _] = parse_hex_color(&ret.color);
                            let color_rect = Rect::from_min_size(
                                Pos2::new(hdr_rect.min.x + 1.0, hdr_rect.max.y - 3.0),
                                Vec2::new(hdr_rect.width() - 2.0, 2.0),
                            );
                            painter.rect_filled(
                                color_rect,
                                Rounding::ZERO,
                                Color32::from_rgb(r, g, b),
                            );

                            painter.text(
                                hdr_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &ret.name,
                                egui::FontId::proportional(10.5),
                                palette.control_foreground,
                            );
                        }

                        // Empty slots for Return tracks
                        for _ in 0..num_scenes {
                            let (slot_rect, _) = ui.allocate_exact_size(
                                Vec2::new(track_width * 0.75, 22.0),
                                Sense::hover(),
                            );
                            if ui.is_rect_visible(slot_rect) {
                                let painter = ui.painter();
                                painter.rect_filled(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Color32::from_rgb(0x1a, 0x1a, 0x1a),
                                );
                                painter.rect_stroke(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Stroke::new(1.0_f32, Color32::from_rgb(0x25, 0x25, 0x25)),
                                );
                            }
                        }

                        // Empty spacer for stop button row
                        ui.allocate_exact_size(Vec2::new(track_width * 0.75, 18.0), Sense::hover());
                    });
                }

                // -------------------------------------------------------------
                // Master / Scene Launch Column (Manual p. 190)
                // -------------------------------------------------------------
                ui.vertical(|ui| {
                    ui.set_width(72.0);
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                    // Master Header
                    let (m_rect, _) = ui.allocate_exact_size(Vec2::new(72.0, 24.0), Sense::hover());
                    if ui.is_rect_visible(m_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(
                            m_rect,
                            Rounding::same(2.0),
                            Color32::from_rgb(0x30, 0x30, 0x30),
                        );
                        painter.rect_stroke(
                            m_rect,
                            Rounding::same(2.0),
                            Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
                        );
                        painter.text(
                            m_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Master",
                            egui::FontId::proportional(11.0),
                            Color32::WHITE,
                        );
                    }

                    // Scene Launch Buttons
                    for scene_idx in 0..num_scenes {
                        let (sc_rect, sc_resp) =
                            ui.allocate_exact_size(Vec2::new(72.0, 22.0), Sense::click());
                        let painter = ui.painter();
                        let bg = if sc_resp.hovered() {
                            Color32::from_rgb(0x40, 0x40, 0x40)
                        } else {
                            Color32::from_rgb(0x2c, 0x2c, 0x2c)
                        };
                        painter.rect_filled(sc_rect, Rounding::same(2.0), bg);
                        painter.rect_stroke(
                            sc_rect,
                            Rounding::same(2.0),
                            Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38)),
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
                        let arrow_c = Pos2::new(sc_rect.max.x - 12.0, sc_rect.center().y);
                        let p1 = Pos2::new(arrow_c.x - 3.0, arrow_c.y - 4.5);
                        let p2 = Pos2::new(arrow_c.x - 3.0, arrow_c.y + 4.5);
                        let p3 = Pos2::new(arrow_c.x + 4.0, arrow_c.y);
                        painter.add(egui::Shape::convex_polygon(
                            vec![p1, p2, p3],
                            palette.play_green,
                            Stroke::NONE,
                        ));

                        if sc_resp.clicked() {
                            action.trigger_scene = Some(scene_idx);
                        }
                    }

                    // Stop All Clips Button
                    let (sa_rect, sa_resp) =
                        ui.allocate_exact_size(Vec2::new(72.0, 18.0), Sense::click());
                    if ui.is_rect_visible(sa_rect) {
                        let painter = ui.painter();
                        let bg = if sa_resp.hovered() {
                            palette.button_hover
                        } else {
                            palette.control_background
                        };
                        painter.rect_filled(sa_rect, Rounding::same(2.0), bg);
                        painter.rect_stroke(
                            sa_rect,
                            Rounding::same(2.0),
                            Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
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
                });
            });
        });

    action
}
