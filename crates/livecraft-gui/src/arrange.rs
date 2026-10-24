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

    let total_bars = 32;
    let pixels_per_beat = 24.0;
    let timeline_width = (total_bars * 4) as f32 * pixels_per_beat;
    let track_header_width = 140.0;
    let track_lane_height = 48.0;

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Top Time Ruler
                ui.horizontal(|ui| {
                    ui.allocate_exact_size(Vec2::new(track_header_width, 24.0), Sense::hover());
                    let (ruler_rect, ruler_resp) =
                        ui.allocate_exact_size(Vec2::new(timeline_width, 24.0), Sense::click());
                    let painter = ui.painter();

                    painter.rect_filled(
                        ruler_rect,
                        Rounding::ZERO,
                        Color32::from_rgb(0x18, 0x18, 0x18),
                    );
                    painter.rect_stroke(
                        ruler_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38)),
                    );

                    // Draw bar tick marks and numbers
                    for bar in 1..=total_bars {
                        let bar_x = ruler_rect.min.x + (bar - 1) as f32 * 4.0 * pixels_per_beat;
                        painter.line_segment(
                            [
                                Pos2::new(bar_x, ruler_rect.min.y),
                                Pos2::new(bar_x, ruler_rect.max.y),
                            ],
                            Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x44, 0x44)),
                        );
                        painter.text(
                            Pos2::new(bar_x + 4.0, ruler_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", bar),
                            egui::FontId::proportional(10.0),
                            palette.control_foreground,
                        );

                        // Beat sub-ticks
                        for b in 1..4 {
                            let beat_x = bar_x + b as f32 * pixels_per_beat;
                            painter.line_segment(
                                [
                                    Pos2::new(beat_x, ruler_rect.max.y - 6.0),
                                    Pos2::new(beat_x, ruler_rect.max.y),
                                ],
                                Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
                            );
                        }
                    }

                    if ruler_resp.clicked() {
                        if let Some(pos) = ruler_resp.interact_pointer_pos() {
                            let rel_x = (pos.x - ruler_rect.min.x).max(0.0);
                            let new_beats = (rel_x / pixels_per_beat) as f64;
                            action.set_playhead = Some(new_beats);
                        }
                    }
                });

                ui.separator();

                // Track Lanes
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let is_selected = trk_idx == selected_track_idx;

                    ui.horizontal(|ui| {
                        // Left Track Lane Header
                        let (hdr_rect, hdr_resp) = ui.allocate_exact_size(
                            Vec2::new(track_header_width, track_lane_height),
                            Sense::click(),
                        );
                        let painter = ui.painter();

                        let bg_color = if is_selected {
                            palette.surface_highlight
                        } else {
                            palette.control_background
                        };
                        painter.rect_filled(hdr_rect, Rounding::same(2.0), bg_color);
                        let border_stroke = if is_selected {
                            Stroke::new(1.5_f32, palette.amber)
                        } else {
                            Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38))
                        };
                        painter.rect_stroke(hdr_rect, Rounding::same(2.0), border_stroke);

                        // Color tag
                        let [r, g, b, _] = parse_hex_color(&trk.color);
                        painter.rect_filled(
                            Rect::from_min_size(hdr_rect.min, Vec2::new(4.0, hdr_rect.height())),
                            Rounding::same(1.0),
                            Color32::from_rgb(r, g, b),
                        );

                        // Track Name & Type
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

                        // Volume / Pan preview
                        painter.text(
                            Pos2::new(hdr_rect.min.x + 10.0, hdr_rect.min.y + 30.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{:.1} dB | Pan {:.2}", trk.volume_db, trk.pan),
                            egui::FontId::proportional(9.0),
                            Color32::from_rgb(0x88, 0x88, 0x88),
                        );

                        if hdr_resp.clicked() {
                            action.select_track = Some(trk_idx);
                        }

                        // Right Timeline Lane
                        let (lane_rect, lane_resp) = ui.allocate_exact_size(
                            Vec2::new(timeline_width, track_lane_height),
                            Sense::click(),
                        );
                        let lane_painter = ui.painter();

                        lane_painter.rect_filled(
                            lane_rect,
                            Rounding::ZERO,
                            Color32::from_rgb(0x20, 0x20, 0x20),
                        );
                        lane_painter.rect_stroke(
                            lane_rect,
                            Rounding::ZERO,
                            Stroke::new(1.0_f32, Color32::from_rgb(0x2d, 0x2d, 0x2d)),
                        );

                        // Vertical bar grid lines
                        for bar in 1..=total_bars {
                            let bar_x = lane_rect.min.x + (bar - 1) as f32 * 4.0 * pixels_per_beat;
                            lane_painter.line_segment(
                                [
                                    Pos2::new(bar_x, lane_rect.min.y),
                                    Pos2::new(bar_x, lane_rect.max.y),
                                ],
                                Stroke::new(0.5_f32, Color32::from_rgb(0x2a, 0x2a, 0x2a)),
                            );
                        }

                        // Render clips in this lane (from arrangement timeline clips or clip slots fallback)
                        if !trk.arrangement_clips.is_empty() {
                            let mut to_duplicate = None;
                            let mut to_delete = None;
                            let mut to_split = None;

                            for (c_idx, arr_clip) in trk.arrangement_clips.iter_mut().enumerate() {
                                let clip = &arr_clip.clip;
                                let clip_w =
                                    (arr_clip.length_beats as f32 * pixels_per_beat).max(12.0);
                                let clip_x = lane_rect.min.x
                                    + (arr_clip.start_beat as f32 * pixels_per_beat);

                                let clip_rect = Rect::from_min_size(
                                    Pos2::new(clip_x, lane_rect.min.y + 2.0),
                                    Vec2::new(clip_w, lane_rect.height() - 4.0),
                                );

                                if clip_rect.max.x <= lane_rect.max.x {
                                    let clip_resp = ui.interact(
                                        clip_rect,
                                        ui.id().with((trk_idx, c_idx)),
                                        Sense::click_and_drag(),
                                    );

                                    if clip_resp.dragged() {
                                        let delta =
                                            (clip_resp.drag_delta().x / pixels_per_beat) as f64;
                                        arr_clip.start_beat =
                                            (arr_clip.start_beat + delta).max(0.0);
                                    }
                                    if clip_resp.drag_stopped() {
                                        arr_clip.start_beat =
                                            (arr_clip.start_beat * 4.0).round() / 4.0;
                                    }
                                    if clip_resp.clicked() {
                                        action.select_track = Some(trk_idx);
                                        action.select_clip = Some(c_idx);
                                    }

                                    if clip_resp.hovered() {
                                        if ui.input(|i| {
                                            i.modifiers.command && i.key_pressed(egui::Key::D)
                                        }) {
                                            to_duplicate = Some(c_idx);
                                        }
                                        if ui.input(|i| {
                                            i.key_pressed(egui::Key::Delete)
                                                || i.key_pressed(egui::Key::Backspace)
                                        }) {
                                            to_delete = Some(c_idx);
                                        }
                                        if ui.input(|i| {
                                            i.modifiers.command && i.key_pressed(egui::Key::E)
                                        }) {
                                            to_split = Some((c_idx, playhead_beats));
                                        }
                                    }

                                    let [cr, cg, cb, _] = parse_hex_color(&clip.color);
                                    let clip_c = Color32::from_rgb(cr, cg, cb);

                                    lane_painter.rect_filled(
                                        clip_rect,
                                        Rounding::same(3.0),
                                        Color32::from_rgba_unmultiplied(cr, cg, cb, 180),
                                    );
                                    let border_color = if clip_resp.hovered() {
                                        Color32::WHITE
                                    } else {
                                        clip_c
                                    };
                                    lane_painter.rect_stroke(
                                        clip_rect,
                                        Rounding::same(3.0),
                                        Stroke::new(1.0_f32, border_color),
                                    );

                                    // Clip Name
                                    lane_painter.text(
                                        Pos2::new(clip_rect.min.x + 4.0, clip_rect.min.y + 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        &clip.name,
                                        egui::FontId::proportional(10.0),
                                        Color32::WHITE,
                                    );

                                    // Preview notes or waveform
                                    if let ClipPayload::Midi(ref m) = clip.payload {
                                        for n in &m.notes {
                                            let n_x = clip_rect.min.x
                                                + (n.start_beat as f32 * pixels_per_beat);
                                            let n_w = (n.duration_beats as f32 * pixels_per_beat)
                                                .max(2.0);
                                            let n_y = clip_rect.max.y
                                                - 4.0
                                                - ((n.pitch % 24) as f32 / 24.0)
                                                    * (clip_rect.height() - 16.0);
                                            lane_painter.rect_filled(
                                                Rect::from_min_size(
                                                    Pos2::new(n_x, n_y),
                                                    Vec2::new(n_w, 3.0),
                                                ),
                                                Rounding::ZERO,
                                                Color32::WHITE,
                                            );
                                        }
                                    }
                                }
                            }

                            if let Some(c_idx) = to_duplicate {
                                let orig = &trk.arrangement_clips[c_idx];
                                let new_id = orig.id + 1000;
                                let new_start = orig.start_beat + orig.length_beats;
                                let dup = livecraft_core::model::ArrangementClip::new(
                                    new_id,
                                    orig.clip.clone(),
                                    new_start,
                                    orig.length_beats,
                                );
                                trk.arrangement_clips.push(dup);
                            }
                            if let Some((c_idx, split_at)) = to_split {
                                let orig = &trk.arrangement_clips[c_idx];
                                if split_at > orig.start_beat
                                    && split_at < orig.start_beat + orig.length_beats
                                {
                                    let left_len = split_at - orig.start_beat;
                                    let right_len = orig.length_beats - left_len;
                                    let right_clip = orig.clip.clone();
                                    let right_arr = livecraft_core::model::ArrangementClip::new(
                                        orig.id + 2000,
                                        right_clip,
                                        split_at,
                                        right_len,
                                    );
                                    trk.arrangement_clips[c_idx].length_beats = left_len;
                                    trk.arrangement_clips.push(right_arr);
                                }
                            }
                            if let Some(c_idx) = to_delete {
                                if c_idx < trk.arrangement_clips.len() {
                                    trk.arrangement_clips.remove(c_idx);
                                }
                            }
                        } else {
                            let mut current_beat = 0.0;
                            for slot in &trk.clip_slots {
                                if let Some(ref clip) = slot.clip {
                                    let clip_w =
                                        (clip.length_beats as f32 * pixels_per_beat).max(12.0);
                                    let clip_x =
                                        lane_rect.min.x + (current_beat as f32 * pixels_per_beat);

                                    let clip_rect = Rect::from_min_size(
                                        Pos2::new(clip_x, lane_rect.min.y + 2.0),
                                        Vec2::new(clip_w, lane_rect.height() - 4.0),
                                    );

                                    if clip_rect.max.x <= lane_rect.max.x {
                                        let [cr, cg, cb, _] = parse_hex_color(&clip.color);
                                        let clip_c = Color32::from_rgb(cr, cg, cb);

                                        lane_painter.rect_filled(
                                            clip_rect,
                                            Rounding::same(3.0),
                                            Color32::from_rgba_unmultiplied(cr, cg, cb, 180),
                                        );
                                        lane_painter.rect_stroke(
                                            clip_rect,
                                            Rounding::same(3.0),
                                            Stroke::new(1.0_f32, clip_c),
                                        );

                                        // Clip Name
                                        lane_painter.text(
                                            Pos2::new(clip_rect.min.x + 4.0, clip_rect.min.y + 8.0),
                                            egui::Align2::LEFT_CENTER,
                                            &clip.name,
                                            egui::FontId::proportional(10.0),
                                            Color32::WHITE,
                                        );

                                        // Preview notes or waveform
                                        if let ClipPayload::Midi(ref m) = clip.payload {
                                            for n in &m.notes {
                                                let n_x = clip_rect.min.x
                                                    + (n.start_beat as f32 * pixels_per_beat);
                                                let n_w = (n.duration_beats as f32
                                                    * pixels_per_beat)
                                                    .max(2.0);
                                                let n_y = clip_rect.max.y
                                                    - 4.0
                                                    - ((n.pitch % 24) as f32 / 24.0)
                                                        * (clip_rect.height() - 16.0);
                                                lane_painter.rect_filled(
                                                    Rect::from_min_size(
                                                        Pos2::new(n_x, n_y),
                                                        Vec2::new(n_w, 3.0),
                                                    ),
                                                    Rounding::ZERO,
                                                    Color32::WHITE,
                                                );
                                            }
                                        }
                                    }

                                    current_beat += clip.length_beats;
                                }
                            }
                        }

                        // Playhead vertical line
                        let playhead_x =
                            lane_rect.min.x + (playhead_beats as f32 * pixels_per_beat);
                        if playhead_x >= lane_rect.min.x && playhead_x <= lane_rect.max.x {
                            lane_painter.line_segment(
                                [
                                    Pos2::new(playhead_x, lane_rect.min.y),
                                    Pos2::new(playhead_x, lane_rect.max.y),
                                ],
                                Stroke::new(1.5_f32, palette.amber),
                            );
                        }

                        if lane_resp.clicked() {
                            if let Some(pos) = lane_resp.interact_pointer_pos() {
                                let rel_x = (pos.x - lane_rect.min.x).max(0.0);
                                action.set_playhead = Some((rel_x / pixels_per_beat) as f64);
                                action.select_track = Some(trk_idx);
                            }
                        }
                    });

                    ui.add_space(2.0);
                }
            });
        });

    action
}
