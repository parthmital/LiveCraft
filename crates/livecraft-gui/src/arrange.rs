use crate::style::LivePalette;
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::{ClipPayload, Project};
use livecraft_core::theme::parse_hex_color;

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

    let total_bars = 48;
    let pixels_per_beat = 28.0;
    let timeline_width = (total_bars * 4) as f32 * pixels_per_beat;
    let track_header_width = 150.0;
    let track_lane_height = 54.0;

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // -------------------------------------------------------------
                // 1. Top Time Ruler & Loop Brace (Manual p. 165)
                // -------------------------------------------------------------
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

                    // Track title spacer on left
                    let (spacer_rect, _) =
                        ui.allocate_exact_size(Vec2::new(track_header_width, 28.0), Sense::hover());
                    if ui.is_rect_visible(spacer_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(
                            spacer_rect,
                            Rounding::ZERO,
                            Color32::from_rgb(0x22, 0x22, 0x22),
                        );
                        painter.rect_stroke(
                            spacer_rect,
                            Rounding::ZERO,
                            Stroke::new(1.0_f32, palette.panel_border),
                        );
                        painter.text(
                            Pos2::new(spacer_rect.min.x + 8.0, spacer_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Track Title",
                            egui::FontId::proportional(10.0),
                            palette.control_disabled,
                        );
                    }

                    // Timeline Ruler & Scrub Area
                    let (ruler_rect, ruler_resp) = ui.allocate_exact_size(
                        Vec2::new(timeline_width, 28.0),
                        Sense::click_and_drag(),
                    );
                    let painter = ui.painter();

                    painter.rect_filled(
                        ruler_rect,
                        Rounding::ZERO,
                        Color32::from_rgb(0x1c, 0x1c, 0x1c),
                    );
                    painter.rect_stroke(
                        ruler_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0_f32, palette.panel_border),
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
                        painter.rect_filled(brace_rect, Rounding::same(1.0), palette.amber);

                        // Loop start & end flags
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(loop_x_start, ruler_rect.min.y + 1.0),
                                Vec2::new(2.0, 8.0),
                            ),
                            Rounding::ZERO,
                            palette.amber,
                        );
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(loop_x_start + loop_w - 2.0, ruler_rect.min.y + 1.0),
                                Vec2::new(2.0, 8.0),
                            ),
                            Rounding::ZERO,
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
                            Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
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
                                Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
                            );
                        }
                    }

                    // Playhead scrubber marker in ruler
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

                    if ruler_resp.clicked() || ruler_resp.dragged() {
                        if let Some(pos) = ruler_resp.interact_pointer_pos() {
                            let rel_x = (pos.x - ruler_rect.min.x).max(0.0);
                            let new_beats = (rel_x / pixels_per_beat) as f64;
                            action.set_playhead = Some(new_beats);
                        }
                    }
                });

                // -------------------------------------------------------------
                // 2. Track Lanes (Manual p. 165-175)
                // -------------------------------------------------------------
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let is_selected = trk_idx == selected_track_idx;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

                        // Left Track Header
                        let (hdr_rect, hdr_resp) = ui.allocate_exact_size(
                            Vec2::new(track_header_width, track_lane_height),
                            Sense::click(),
                        );
                        if ui.is_rect_visible(hdr_rect) {
                            let painter = ui.painter();
                            let bg_color = if is_selected {
                                palette.surface_highlight
                            } else {
                                Color32::from_rgb(0x28, 0x28, 0x28)
                            };

                            painter.rect_filled(hdr_rect, Rounding::same(1.0), bg_color);
                            let border_stroke = if is_selected {
                                Stroke::new(1.5_f32, palette.amber)
                            } else {
                                Stroke::new(1.0_f32, palette.panel_border)
                            };
                            painter.rect_stroke(hdr_rect, Rounding::same(1.0), border_stroke);

                            // Left color accent bar
                            let [r, g, b, _] = parse_hex_color(&trk.color);
                            painter.rect_filled(
                                Rect::from_min_size(
                                    hdr_rect.min,
                                    Vec2::new(4.0, hdr_rect.height()),
                                ),
                                Rounding::same(1.0),
                                Color32::from_rgb(r, g, b),
                            );

                            // Track Title
                            painter.text(
                                Pos2::new(hdr_rect.min.x + 10.0, hdr_rect.min.y + 12.0),
                                egui::Align2::LEFT_CENTER,
                                &trk.name,
                                egui::FontId::proportional(11.0),
                                if is_selected {
                                    Color32::WHITE
                                } else {
                                    palette.control_foreground
                                },
                            );

                            // Controls info (Mute, Solo, Arm status)
                            let act_color = if !trk.is_muted {
                                palette.amber
                            } else {
                                palette.control_disabled
                            };
                            let solo_color = if trk.is_soloed {
                                palette.solo_blue
                            } else {
                                palette.control_disabled
                            };
                            let arm_color = if trk.is_armed {
                                palette.record_red
                            } else {
                                palette.control_disabled
                            };

                            painter.text(
                                Pos2::new(hdr_rect.min.x + 10.0, hdr_rect.min.y + 32.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.1} dB | Pan {:.2}", trk.volume_db, trk.pan),
                                egui::FontId::monospace(8.5),
                                Color32::from_rgb(0x88, 0x88, 0x88),
                            );

                            // Small status dots
                            painter.circle_filled(
                                Pos2::new(hdr_rect.max.x - 36.0, hdr_rect.min.y + 32.0),
                                3.0,
                                act_color,
                            );
                            painter.circle_filled(
                                Pos2::new(hdr_rect.max.x - 24.0, hdr_rect.min.y + 32.0),
                                3.0,
                                solo_color,
                            );
                            painter.circle_filled(
                                Pos2::new(hdr_rect.max.x - 12.0, hdr_rect.min.y + 32.0),
                                3.0,
                                arm_color,
                            );
                        }

                        if hdr_resp.clicked() {
                            action.select_track = Some(trk_idx);
                        }

                        // Right Timeline Lane
                        let (lane_rect, lane_resp) = ui.allocate_exact_size(
                            Vec2::new(timeline_width, track_lane_height),
                            Sense::click(),
                        );
                        let lane_painter = ui.painter();

                        // Alternating lane background
                        let lane_bg = if trk_idx % 2 == 0 {
                            Color32::from_rgb(0x22, 0x22, 0x22)
                        } else {
                            Color32::from_rgb(0x25, 0x25, 0x25)
                        };
                        lane_painter.rect_filled(lane_rect, Rounding::ZERO, lane_bg);
                        lane_painter.rect_stroke(
                            lane_rect,
                            Rounding::ZERO,
                            Stroke::new(1.0_f32, Color32::from_rgb(0x2e, 0x2e, 0x2e)),
                        );

                        // Vertical bar grid lines
                        for bar in 1..=total_bars {
                            let bar_x = lane_rect.min.x + (bar - 1) as f32 * 4.0 * pixels_per_beat;
                            lane_painter.line_segment(
                                [
                                    Pos2::new(bar_x, lane_rect.min.y),
                                    Pos2::new(bar_x, lane_rect.max.y),
                                ],
                                Stroke::new(1.0_f32, Color32::from_rgb(0x2e, 0x2e, 0x2e)),
                            );

                            for b in 1..4 {
                                let beat_x = bar_x + b as f32 * pixels_per_beat;
                                lane_painter.line_segment(
                                    [
                                        Pos2::new(beat_x, lane_rect.min.y),
                                        Pos2::new(beat_x, lane_rect.max.y),
                                    ],
                                    Stroke::new(0.5_f32, Color32::from_rgb(0x28, 0x28, 0x28)),
                                );
                            }
                        }

                        // Render Arrangement Clips (or fallback clip slot 0 for preview)
                        let clips_to_render: Vec<(f64, f64, &livecraft_core::model::Clip)> =
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

                            let [r, g, b, _] = parse_hex_color(&clip.color);
                            let clip_color = Color32::from_rgb(r, g, b);

                            // Clip Body Fill
                            lane_painter.rect_filled(
                                clip_rect,
                                Rounding::same(2.0),
                                Color32::from_rgba_unmultiplied(r, g, b, 70),
                            );
                            lane_painter.rect_stroke(
                                clip_rect,
                                Rounding::same(2.0),
                                Stroke::new(1.0_f32, clip_color),
                            );

                            // Clip Header Bar
                            let hdr_bar = Rect::from_min_size(
                                clip_rect.min,
                                Vec2::new(clip_rect.width(), 14.0),
                            );
                            lane_painter.rect_filled(hdr_bar, Rounding::same(1.0), clip_color);
                            lane_painter.text(
                                Pos2::new(hdr_bar.min.x + 4.0, hdr_bar.center().y),
                                egui::Align2::LEFT_CENTER,
                                &clip.name,
                                egui::FontId::proportional(9.5),
                                Color32::BLACK,
                            );

                            // Draw preview contents: MIDI notes or Audio waveform
                            match clip.payload {
                                ClipPayload::Midi(ref mdata) => {
                                    for note in &mdata.notes {
                                        let note_x = clip_rect.min.x
                                            + (note.start_beat as f32 * pixels_per_beat);
                                        let note_w =
                                            (note.duration_beats as f32 * pixels_per_beat).max(3.0);
                                        // Map pitch 36-84 to vertical space in clip
                                        let norm_pitch = ((note.pitch.clamp(36, 84) - 36) as f32
                                            / 48.0)
                                            .clamp(0.0, 1.0);
                                        let note_y = clip_rect.max.y
                                            - 4.0
                                            - norm_pitch * (clip_rect.height() - 20.0);
                                        let n_rect = Rect::from_min_size(
                                            Pos2::new(note_x, note_y),
                                            Vec2::new(note_w, 2.5),
                                        );
                                        lane_painter.rect_filled(
                                            n_rect,
                                            Rounding::same(0.5),
                                            Color32::WHITE,
                                        );
                                    }
                                }
                                ClipPayload::Audio(_) => {
                                    // Simulated high-fidelity waveform center line & contour
                                    let mid_y = clip_rect.center().y + 6.0;
                                    let steps = (clip_rect.width() / 4.0).max(4.0) as usize;
                                    for i in 0..steps {
                                        let x = clip_rect.min.x + i as f32 * 4.0;
                                        let wave_amp = ((i as f32 * 0.4).sin().abs() * 8.0) + 2.0;
                                        lane_painter.line_segment(
                                            [
                                                Pos2::new(x, mid_y - wave_amp),
                                                Pos2::new(x, mid_y + wave_amp),
                                            ],
                                            Stroke::new(
                                                1.5_f32,
                                                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
                                            ),
                                        );
                                    }
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

                        if lane_resp.clicked() {
                            if let Some(pos) = lane_resp.interact_pointer_pos() {
                                let rel_x = (pos.x - lane_rect.min.x).max(0.0);
                                let new_beats = (rel_x / pixels_per_beat) as f64;
                                action.set_playhead = Some(new_beats);
                                action.select_track = Some(trk_idx);
                            }
                        }
                    });
                }
            });
        });

    action
}
