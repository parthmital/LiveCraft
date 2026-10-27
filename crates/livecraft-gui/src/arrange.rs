use crate::style::LivePalette;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use livecraft_formats::theme::parse_hex_color;
use livecraft_model::model::{ClipPayload, Project};

pub struct ArrangementAction {
    pub set_playhead: Option<f64>,
    pub select_track: Option<usize>,
    pub select_clip: Option<usize>,
}

pub fn render_arrangement_view(
    ui: &mut Ui,
    project: &mut Project,
    selected_track_idx: usize,
    playhead_beats: f64,
    palette: &LivePalette,
) -> ArrangementAction {
    let mut action = ArrangementAction {
        set_playhead: None,
        select_track: None,
        select_clip: None,
    };

    let avail_w = ui.available_width();
    let track_header_width = 180.0_f32;
    let track_lane_height = 56.0_f32;
    let timeline_width = (avail_w - track_header_width - 4.0).max(300.0);
    let pixels_per_beat = 24.0_f32;
    let total_bars = (timeline_width / (4.0 * pixels_per_beat)).ceil() as usize;
    let is_dark = palette.is_dark();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // -------------------------------------------------------------
                // 1. Top Time Ruler & Loop Brace (Manual p. 165)
                // -------------------------------------------------------------
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

                    // Timeline Ruler & Scrub Area on LEFT
                    let (ruler_rect, ruler_resp) = ui.allocate_exact_size(
                        Vec2::new(timeline_width, 28.0),
                        Sense::click_and_drag(),
                    );
                    let painter = ui.painter();

                    painter.rect_filled(ruler_rect, CornerRadius::ZERO, palette.ruler_bg);
                    painter.rect_stroke(
                        ruler_rect,
                        CornerRadius::ZERO,
                        Stroke::new(1.0_f32, palette.panel_border),
                        eframe::egui::StrokeKind::Middle,
                    );

                    // Loop Brace
                    if project.transport.loop_enabled {
                        let loop_x_start = ruler_rect.min.x
                            + (project.transport.loop_start_beats as f32 * pixels_per_beat);
                        let loop_w =
                            (project.transport.loop_length_beats as f32 * pixels_per_beat).max(8.0);
                        let brace_rect = Rect::from_min_size(
                            Pos2::new(loop_x_start, ruler_rect.min.y + 1.0),
                            Vec2::new(loop_w, 4.0),
                        );
                        painter.rect_filled(brace_rect, CornerRadius::same(1), palette.amber);

                        // Loop start & end flags
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(loop_x_start, ruler_rect.min.y + 1.0),
                                Vec2::new(2.0, 8.0),
                            ),
                            CornerRadius::ZERO,
                            palette.amber,
                        );
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(loop_x_start + loop_w - 2.0, ruler_rect.min.y + 1.0),
                                Vec2::new(2.0, 8.0),
                            ),
                            CornerRadius::ZERO,
                            palette.amber,
                        );
                    }

                    // Bar number markers and subdivisions
                    for bar in 1..=total_bars {
                        let bar_x = ruler_rect.min.x + (bar - 1) as f32 * 4.0 * pixels_per_beat;

                        // Major bar line
                        painter.line_segment(
                            [
                                Pos2::new(bar_x, ruler_rect.min.y + 8.0),
                                Pos2::new(bar_x, ruler_rect.max.y),
                            ],
                            Stroke::new(1.0_f32, palette.panel_border),
                        );

                        // Bar number text
                        painter.text(
                            Pos2::new(bar_x + 4.0, ruler_rect.min.y + 16.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", bar),
                            egui::FontId::proportional(10.0),
                            palette.control_foreground,
                        );

                        // Beat tick marks
                        for b in 1..4 {
                            let beat_x = bar_x + b as f32 * pixels_per_beat;
                            painter.line_segment(
                                [
                                    Pos2::new(beat_x, ruler_rect.max.y - 5.0),
                                    Pos2::new(beat_x, ruler_rect.max.y),
                                ],
                                Stroke::new(
                                    0.5_f32,
                                    if is_dark {
                                        Color32::from_rgb(0x38, 0x38, 0x38)
                                    } else {
                                        Color32::from_rgb(0x9a, 0x9a, 0x98)
                                    },
                                ),
                            );
                        }
                    }

                    // Playhead marker in ruler
                    let playhead_x = ruler_rect.min.x + (playhead_beats as f32 * pixels_per_beat);
                    if playhead_x >= ruler_rect.min.x && playhead_x <= ruler_rect.max.x {
                        let p1 = Pos2::new(playhead_x - 4.5, ruler_rect.min.y + 2.0);
                        let p2 = Pos2::new(playhead_x + 4.5, ruler_rect.min.y + 2.0);
                        let p3 = Pos2::new(playhead_x, ruler_rect.min.y + 9.0);
                        painter.add(egui::Shape::convex_polygon(
                            vec![p1, p2, p3],
                            palette.play_green,
                            Stroke::NONE,
                        ));
                        painter.line_segment(
                            [
                                Pos2::new(playhead_x, ruler_rect.min.y + 8.0),
                                Pos2::new(playhead_x, ruler_rect.max.y),
                            ],
                            Stroke::new(1.5_f32, palette.play_green),
                        );
                    }

                    if (ruler_resp.clicked() || ruler_resp.dragged())
                        && let Some(pos) = ruler_resp.interact_pointer_pos()
                    {
                        let rel_x = (pos.x - ruler_rect.min.x).max(0.0);
                        let new_beats = (rel_x / pixels_per_beat) as f64;
                        action.set_playhead = Some(new_beats);
                    }

                    // Track header top toolbar on RIGHT (Ableton Live 12 layout)
                    let (spacer_rect, _) =
                        ui.allocate_exact_size(Vec2::new(track_header_width, 28.0), Sense::hover());
                    if ui.is_rect_visible(spacer_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(spacer_rect, CornerRadius::ZERO, palette.button_idle);
                        painter.rect_stroke(
                            spacer_rect,
                            CornerRadius::ZERO,
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );

                        // "Set" button
                        let set_box = Rect::from_min_size(
                            Pos2::new(spacer_rect.min.x + 8.0, spacer_rect.min.y + 4.0),
                            Vec2::new(32.0, 20.0),
                        );
                        painter.rect_filled(set_box, CornerRadius::same(2), palette.surface_area);
                        painter.rect_stroke(
                            set_box,
                            CornerRadius::same(2),
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );
                        painter.text(
                            set_box.center(),
                            egui::Align2::CENTER_CENTER,
                            "Set",
                            egui::FontId::proportional(10.0),
                            palette.control_foreground,
                        );

                        // Locator cue icon
                        let cue_c = Pos2::new(spacer_rect.min.x + 52.0, spacer_rect.center().y);
                        painter.line_segment(
                            [
                                Pos2::new(cue_c.x - 4.0, cue_c.y + 4.0),
                                Pos2::new(cue_c.x + 4.0, cue_c.y - 4.0),
                            ],
                            Stroke::new(1.5_f32, palette.control_foreground),
                        );
                        painter.circle_filled(
                            Pos2::new(cue_c.x + 4.0, cue_c.y - 4.0),
                            2.5,
                            palette.amber,
                        );
                    }
                });

                // -------------------------------------------------------------
                // 2. Track Lanes (Manual p. 165-175): Timeline on Left, Track Strips on Right
                // -------------------------------------------------------------
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let is_selected = trk_idx == selected_track_idx;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

                        // A. Timeline Lane on the LEFT
                        let (lane_rect, lane_resp) = ui.allocate_exact_size(
                            Vec2::new(timeline_width, track_lane_height),
                            Sense::click(),
                        );
                        let lane_painter = ui.painter();

                        let lane_bg = if trk_idx % 2 == 0 {
                            palette.clip_slot_bg
                        } else if is_dark {
                            Color32::from_rgb(0x25, 0x25, 0x25)
                        } else {
                            Color32::from_rgb(0xbb, 0xbb, 0xb9)
                        };
                        lane_painter.rect_filled(lane_rect, CornerRadius::ZERO, lane_bg);
                        lane_painter.rect_stroke(
                            lane_rect,
                            CornerRadius::ZERO,
                            Stroke::new(1.0_f32, palette.panel_border),
                            eframe::egui::StrokeKind::Middle,
                        );

                        // Left track color stripe
                        let [r, g, b, _] = parse_hex_color(&trk.color);
                        lane_painter.rect_filled(
                            Rect::from_min_size(lane_rect.min, Vec2::new(3.0, lane_rect.height())),
                            CornerRadius::ZERO,
                            Color32::from_rgb(r, g, b),
                        );

                        // Vertical bar grid lines
                        for bar in 1..=total_bars {
                            let bar_x = lane_rect.min.x + (bar - 1) as f32 * 4.0 * pixels_per_beat;
                            lane_painter.line_segment(
                                [
                                    Pos2::new(bar_x, lane_rect.min.y),
                                    Pos2::new(bar_x, lane_rect.max.y),
                                ],
                                Stroke::new(
                                    1.0_f32,
                                    if is_dark {
                                        Color32::from_rgb(0x32, 0x32, 0x32)
                                    } else {
                                        Color32::from_rgb(0xa8, 0xa8, 0xa6)
                                    },
                                ),
                            );

                            for b in 1..4 {
                                let beat_x = bar_x + b as f32 * pixels_per_beat;
                                lane_painter.line_segment(
                                    [
                                        Pos2::new(beat_x, lane_rect.min.y),
                                        Pos2::new(beat_x, lane_rect.max.y),
                                    ],
                                    Stroke::new(
                                        0.5_f32,
                                        if is_dark {
                                            Color32::from_rgb(0x28, 0x28, 0x28)
                                        } else {
                                            Color32::from_rgb(0xb4, 0xb4, 0xb2)
                                        },
                                    ),
                                );
                            }
                        }

                        // Render Arrangement Clips
                        let clips_to_render: Vec<(f64, f64, &livecraft_model::model::Clip)> =
                            if !trk.arrangement_clips.is_empty() {
                                trk.arrangement_clips
                                    .iter()
                                    .map(|ac| (ac.start_beat, ac.length_beats, &ac.clip))
                                    .collect()
                            } else if let Some(c) =
                                trk.clip_slots.first().and_then(|s| s.clip.as_ref())
                            {
                                vec![(0.0, c.length_beats, c)]
                            } else {
                                Vec::new()
                            };

                        for (start_beat, length_beats, clip) in clips_to_render {
                            let clip_w = (length_beats as f32 * pixels_per_beat).max(14.0);
                            let clip_x = lane_rect.min.x + (start_beat as f32 * pixels_per_beat);
                            let clip_rect = Rect::from_min_size(
                                Pos2::new(clip_x, lane_rect.min.y + 2.0),
                                Vec2::new(clip_w, lane_rect.height() - 4.0),
                            );

                            let [cr, cg, cb, _] = parse_hex_color(&clip.color);
                            let clip_color = Color32::from_rgb(cr, cg, cb);

                            // Clip Body Fill
                            let clip_body_bg = if is_dark {
                                Color32::from_rgba_unmultiplied(cr, cg, cb, 80)
                            } else {
                                Color32::from_rgba_unmultiplied(cr, cg, cb, 140)
                            };
                            lane_painter.rect_filled(
                                clip_rect,
                                CornerRadius::same(2),
                                clip_body_bg,
                            );
                            lane_painter.rect_stroke(
                                clip_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, clip_color),
                                eframe::egui::StrokeKind::Middle,
                            );

                            // Clip Title Header Bar
                            let hdr_bar = Rect::from_min_size(
                                clip_rect.min,
                                Vec2::new(clip_rect.width(), 14.0),
                            );
                            lane_painter.rect_filled(hdr_bar, CornerRadius::same(2), clip_color);
                            lane_painter.text(
                                Pos2::new(hdr_bar.min.x + 4.0, hdr_bar.center().y),
                                egui::Align2::LEFT_CENTER,
                                &clip.name,
                                egui::FontId::proportional(9.5),
                                Color32::BLACK,
                            );

                            // Mini preview notes / waveform inside clip
                            if let ClipPayload::Midi(ref mdata) = clip.payload {
                                for note in &mdata.notes {
                                    let n_x = clip_rect.min.x
                                        + (note.start_beat as f32 * pixels_per_beat);
                                    let n_w =
                                        (note.duration_beats as f32 * pixels_per_beat).max(3.0);
                                    let pitch_norm =
                                        ((note.pitch as f32 - 48.0) / 36.0).clamp(0.0, 1.0);
                                    let n_y = clip_rect.max.y
                                        - 4.0
                                        - pitch_norm * (clip_rect.height() - 20.0);
                                    let n_rect = Rect::from_min_size(
                                        Pos2::new(n_x, n_y),
                                        Vec2::new(n_w, 2.5),
                                    );
                                    lane_painter.rect_filled(
                                        n_rect,
                                        CornerRadius::same(0),
                                        if is_dark {
                                            Color32::WHITE
                                        } else {
                                            Color32::BLACK
                                        },
                                    );
                                }
                            }
                        }

                        // Playhead line passing vertically through this lane
                        let playhead_x =
                            lane_rect.min.x + (playhead_beats as f32 * pixels_per_beat);
                        if playhead_x >= lane_rect.min.x && playhead_x <= lane_rect.max.x {
                            lane_painter.line_segment(
                                [
                                    Pos2::new(playhead_x, lane_rect.min.y),
                                    Pos2::new(playhead_x, lane_rect.max.y),
                                ],
                                Stroke::new(1.5_f32, palette.play_green),
                            );
                        }

                        if lane_resp.clicked()
                            && let Some(pos) = lane_resp.interact_pointer_pos()
                        {
                            let rel_x = (pos.x - lane_rect.min.x).max(0.0);
                            let new_beats = (rel_x / pixels_per_beat) as f64;
                            action.set_playhead = Some(new_beats);
                            action.select_track = Some(trk_idx);
                        }

                        // B. Right Track Header Strip (Ableton Live 12 layout)
                        let (hdr_rect, hdr_resp) = ui.allocate_exact_size(
                            Vec2::new(track_header_width, track_lane_height),
                            Sense::click(),
                        );
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            let bg_color = if is_selected {
                                palette.surface_highlight
                            } else {
                                palette.button_idle
                            };

                            painter.rect_filled(hdr_rect, CornerRadius::same(1), bg_color);
                            let border_stroke = if is_selected {
                                Stroke::new(1.5_f32, palette.amber)
                            } else {
                                Stroke::new(1.0_f32, palette.panel_border)
                            };
                            painter.rect_stroke(
                                hdr_rect,
                                CornerRadius::same(1),
                                border_stroke,
                                eframe::egui::StrokeKind::Middle,
                            );

                            // Left color accent bar
                            let [r, g, b, _] = parse_hex_color(&trk.color);
                            painter.rect_filled(
                                Rect::from_min_size(
                                    hdr_rect.min,
                                    Vec2::new(4.0, hdr_rect.height()),
                                ),
                                CornerRadius::same(1),
                                Color32::from_rgb(r, g, b),
                            );

                            // Track Title
                            painter.text(
                                Pos2::new(hdr_rect.min.x + 10.0, hdr_rect.min.y + 14.0),
                                egui::Align2::LEFT_CENTER,
                                &trk.name,
                                egui::FontId::proportional(11.0),
                                if is_selected {
                                    if is_dark {
                                        Color32::WHITE
                                    } else {
                                        Color32::BLACK
                                    }
                                } else {
                                    palette.control_foreground
                                },
                            );

                            // Volume and Pan display
                            let vol_str = if trk.volume_db <= -59.5 {
                                "-inf dB".to_string()
                            } else {
                                format!("{:.1} dB", trk.volume_db)
                            };
                            let pan_str = if trk.pan.abs() < 0.05 {
                                "C".to_string()
                            } else if trk.pan < 0.0 {
                                format!("{}L", (-trk.pan * 50.0).round() as i32)
                            } else {
                                format!("{}R", (trk.pan * 50.0).round() as i32)
                            };
                            painter.text(
                                Pos2::new(hdr_rect.min.x + 10.0, hdr_rect.min.y + 36.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{} | {}", vol_str, pan_str),
                                egui::FontId::monospace(9.0),
                                if is_dark {
                                    Color32::from_rgb(0xa0, 0xa0, 0xa0)
                                } else {
                                    Color32::from_rgb(0x40, 0x40, 0x40)
                                },
                            );

                            // Buttons: Track Activator [1], Solo [S], Arm [●]
                            let btn_y = hdr_rect.center().y;
                            let act_color = if !trk.is_muted {
                                palette.amber
                            } else {
                                palette.button_idle
                            };
                            let act_rect = Rect::from_center_size(
                                Pos2::new(hdr_rect.max.x - 52.0, btn_y),
                                Vec2::new(18.0, 18.0),
                            );
                            painter.rect_filled(act_rect, CornerRadius::same(2), act_color);
                            painter.rect_stroke(
                                act_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, palette.panel_border),
                                eframe::egui::StrokeKind::Middle,
                            );
                            painter.text(
                                act_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}", trk_idx + 1),
                                egui::FontId::proportional(10.0),
                                if !trk.is_muted {
                                    Color32::BLACK
                                } else {
                                    palette.control_disabled
                                },
                            );

                            let solo_color = if trk.is_soloed {
                                palette.solo_blue
                            } else {
                                palette.button_idle
                            };
                            let solo_rect = Rect::from_center_size(
                                Pos2::new(hdr_rect.max.x - 30.0, btn_y),
                                Vec2::new(18.0, 18.0),
                            );
                            painter.rect_filled(solo_rect, CornerRadius::same(2), solo_color);
                            painter.rect_stroke(
                                solo_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, palette.panel_border),
                                eframe::egui::StrokeKind::Middle,
                            );
                            painter.text(
                                solo_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "S",
                                egui::FontId::proportional(10.0),
                                if trk.is_soloed {
                                    Color32::WHITE
                                } else {
                                    palette.control_disabled
                                },
                            );

                            let arm_color = if trk.is_armed {
                                palette.record_red
                            } else {
                                palette.button_idle
                            };
                            let arm_rect = Rect::from_center_size(
                                Pos2::new(hdr_rect.max.x - 10.0, btn_y),
                                Vec2::new(16.0, 16.0),
                            );
                            painter.rect_filled(arm_rect, CornerRadius::same(2), arm_color);
                            painter.rect_stroke(
                                arm_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, palette.panel_border),
                                eframe::egui::StrokeKind::Middle,
                            );
                            painter.circle_filled(
                                arm_rect.center(),
                                3.5,
                                if trk.is_armed {
                                    Color32::WHITE
                                } else {
                                    palette.control_disabled
                                },
                            );
                        }

                        if hdr_resp.clicked() {
                            action.select_track = Some(trk_idx);
                        }
                    });
                }
            });
        });

    action
}
