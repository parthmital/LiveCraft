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
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                let track_width = 88.0;
                let num_scenes = project.scenes.len().max(8);

                // Render regular player tracks (Audio and MIDI)
                for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
                    let is_selected = trk_idx == selected_track_idx;
                    let active_slot = active_clip_slots.get(trk_idx).copied().flatten();

                    ui.vertical(|ui| {
                        ui.set_width(track_width);

                        // Track Header
                        render_track_header(
                            ui,
                            trk.name.as_str(),
                            trk.track_type,
                            &trk.color,
                            is_selected,
                            palette,
                        );
                        if ui
                            .interact(
                                ui.min_rect(),
                                ui.id().with(format!("trk_hdr_{}", trk_idx)),
                                Sense::click(),
                            )
                            .clicked()
                        {
                            action.select_track = Some(trk_idx);
                        }

                        ui.add_space(2.0);

                        // Clip Slots
                        for slot_idx in 0..num_scenes {
                            let slot_clip =
                                trk.clip_slots.get(slot_idx).and_then(|s| s.clip.as_ref());
                            let is_slot_playing =
                                active_slot == Some(slot_idx) && project.transport.is_playing;

                            let (slot_rect, slot_resp) = ui
                                .allocate_exact_size(Vec2::new(track_width, 24.0), Sense::click());
                            let painter = ui.painter();

                            if let Some(clip) = slot_clip {
                                // Filled clip slot
                                let [r, g, b, _] = parse_hex_color(&clip.color);
                                let clip_color = Color32::from_rgb(r, g, b);

                                painter.rect_filled(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Color32::from_rgb(0x2a, 0x2a, 0x2a),
                                );
                                painter.rect_stroke(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Stroke::new(1.0_f32, clip_color),
                                );

                                // Left accent bar with clip color
                                let bar_rect = Rect::from_min_size(
                                    slot_rect.min,
                                    Vec2::new(4.0, slot_rect.height()),
                                );
                                painter.rect_filled(bar_rect, Rounding::same(1.0), clip_color);

                                // Play icon / progress
                                let play_rect = Rect::from_min_size(
                                    Pos2::new(slot_rect.min.x + 6.0, slot_rect.min.y + 4.0),
                                    Vec2::new(14.0, 16.0),
                                );

                                if is_slot_playing {
                                    // Animated progress bar inside clip
                                    let prog = ((playhead_beats % clip.length_beats.max(1.0))
                                        / clip.length_beats.max(1.0))
                                        as f32;
                                    let fill_w = (slot_rect.width() - 4.0) * prog;
                                    let prog_rect = Rect::from_min_size(
                                        Pos2::new(slot_rect.min.x + 4.0, slot_rect.min.y),
                                        Vec2::new(fill_w, slot_rect.height()),
                                    );
                                    painter.rect_filled(
                                        prog_rect,
                                        Rounding::ZERO,
                                        Color32::from_rgba_unmultiplied(r, g, b, 60),
                                    );
                                    painter.text(
                                        play_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "▶",
                                        egui::FontId::proportional(11.0),
                                        palette.play_green,
                                    );
                                } else {
                                    painter.text(
                                        play_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "▶",
                                        egui::FontId::proportional(10.0),
                                        Color32::from_rgb(0x88, 0x88, 0x88),
                                    );
                                }

                                // Clip Name
                                painter.text(
                                    Pos2::new(slot_rect.min.x + 22.0, slot_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &clip.name,
                                    egui::FontId::proportional(11.0),
                                    palette.control_foreground,
                                );

                                if slot_resp.clicked() {
                                    action.trigger_clip = Some((trk_idx, slot_idx));
                                    action.select_track = Some(trk_idx);
                                    action.select_clip = Some((trk_idx, slot_idx));
                                }
                            } else {
                                // Empty clip slot
                                let bg_color = if slot_resp.hovered() {
                                    palette.surface_highlight
                                } else {
                                    Color32::from_rgb(0x22, 0x22, 0x22)
                                };
                                painter.rect_filled(slot_rect, Rounding::same(2.0), bg_color);
                                painter.rect_stroke(
                                    slot_rect,
                                    Rounding::same(2.0),
                                    Stroke::new(1.0_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
                                );

                                // Draw Ableton empty slot circle
                                painter.circle_filled(
                                    Pos2::new(slot_rect.min.x + 12.0, slot_rect.center().y),
                                    2.5,
                                    Color32::from_rgb(0x44, 0x44, 0x44),
                                );

                                if slot_resp.double_clicked() {
                                    // Double click on empty slot creates a new clip!
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

                            ui.add_space(2.0);
                        }

                        // Stop clip button for this track
                        let stop_btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new("■")
                                    .size(10.0)
                                    .color(Color32::from_rgb(0x99, 0x99, 0x99)),
                            )
                            .fill(palette.control_background)
                            .min_size(Vec2::new(track_width, 18.0))
                            .rounding(Rounding::same(2.0)),
                        );
                        if stop_btn.clicked() {
                            action.stop_clip = Some(trk_idx);
                        }
                    });
                }

                // Return tracks headers (Reverb, Delay)
                for ret in project.return_tracks.iter_mut() {
                    ui.vertical(|ui| {
                        ui.set_width(track_width * 0.75);
                        render_track_header(
                            ui,
                            ret.name.as_str(),
                            TrackType::Return,
                            &ret.color,
                            false,
                            palette,
                        );
                        ui.add_space(2.0);
                        for _ in 0..num_scenes {
                            let (slot_rect, _) = ui.allocate_exact_size(
                                Vec2::new(track_width * 0.75, 24.0),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                slot_rect,
                                Rounding::same(1.0),
                                Color32::from_rgb(0x1a, 0x1a, 0x1a),
                            );
                            ui.add_space(2.0);
                        }
                    });
                }

                // Master / Scene Launch Column
                ui.vertical(|ui| {
                    ui.set_width(65.0);
                    render_track_header(ui, "Master", TrackType::Master, "#777777", false, palette);
                    ui.add_space(2.0);

                    for scene_idx in 0..num_scenes {
                        let (sc_rect, sc_resp) =
                            ui.allocate_exact_size(Vec2::new(65.0, 24.0), Sense::click());
                        let painter = ui.painter();

                        let bg_color = if sc_resp.hovered() {
                            palette.surface_highlight
                        } else {
                            palette.control_background
                        };
                        painter.rect_filled(sc_rect, Rounding::same(2.0), bg_color);
                        painter.rect_stroke(
                            sc_rect,
                            Rounding::same(2.0),
                            Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
                        );

                        // Scene number and launch arrow
                        painter.text(
                            Pos2::new(sc_rect.min.x + 8.0, sc_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", scene_idx + 1),
                            egui::FontId::proportional(11.0),
                            palette.control_foreground,
                        );
                        painter.text(
                            Pos2::new(sc_rect.max.x - 12.0, sc_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            "▶",
                            egui::FontId::proportional(10.0),
                            palette.play_green,
                        );

                        if sc_resp.clicked() {
                            action.trigger_scene = Some(scene_idx);
                        }

                        ui.add_space(2.0);
                    }

                    // Stop All Clips Button
                    let stop_all = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Stop All")
                                .size(10.0)
                                .color(palette.amber),
                        )
                        .fill(palette.control_background)
                        .min_size(Vec2::new(65.0, 18.0))
                        .rounding(Rounding::same(2.0)),
                    );
                    if stop_all.clicked() {
                        action.stop_all_clips = true;
                    }
                });
            });
        });

    action
}

fn render_track_header(
    ui: &mut Ui,
    name: &str,
    _track_type: TrackType,
    hex_color: &str,
    is_selected: bool,
    palette: &LivePalette,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    let painter = ui.painter();

    let bg_color = if is_selected {
        palette.surface_highlight
    } else {
        palette.control_background
    };

    painter.rect_filled(rect, Rounding::same(2.0), bg_color);
    let border_stroke = if is_selected {
        Stroke::new(1.5_f32, palette.amber)
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38))
    };
    painter.rect_stroke(rect, Rounding::same(2.0), border_stroke);

    // Track color accent bar at bottom of header
    let [r, g, b, _] = parse_hex_color(hex_color);
    let color_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 1.0, rect.max.y - 3.0),
        Vec2::new(rect.width() - 2.0, 2.0),
    );
    painter.rect_filled(color_rect, Rounding::ZERO, Color32::from_rgb(r, g, b));

    // Track Name
    painter.text(
        Pos2::new(rect.center().x, rect.center().y - 1.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(11.0),
        if is_selected {
            Color32::WHITE
        } else {
            palette.control_foreground
        },
    );
}
