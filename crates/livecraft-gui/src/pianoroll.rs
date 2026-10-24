use crate::style::LivePalette;
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::{Clip, ClipPayload, MidiNote};
use livecraft_core::theme::parse_hex_color;

pub fn render_piano_roll(ui: &mut Ui, clip: &mut Clip, playhead_beats: f64, palette: &LivePalette) {
    let ClipPayload::Midi(ref mut midi_data) = clip.payload else {
        ui.label("Audio Clip Waveform Editor");
        return;
    };

    let key_width = 36.0;
    let row_height = 14.0;
    let total_pitches: u8 = 36; // 3 octaves: C3 (48) to B5 (83)
    let min_pitch = 48u8;
    let total_beats = clip.length_beats.max(4.0) as f32;
    let pixels_per_beat = 48.0;
    let grid_width = total_beats * pixels_per_beat;

    ui.vertical(|ui| {
        // Clip Header: Name, Length, Loop
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&clip.name)
                    .strong()
                    .size(12.0)
                    .color(palette.control_foreground),
            );
            ui.separator();
            ui.label(format!("Length: {:.1} beats", clip.length_beats));
            ui.checkbox(&mut clip.is_looping, "Loop");
            ui.label(format!("Notes: {}", midi_data.notes.len()));
        });

        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left Piano Keyboard
                    let (keys_rect, _) = ui.allocate_exact_size(
                        Vec2::new(key_width, total_pitches as f32 * row_height),
                        Sense::hover(),
                    );
                    let painter = ui.painter();

                    for i in 0..total_pitches {
                        let pitch = min_pitch + (total_pitches - 1 - i);
                        let row_y = keys_rect.min.y + i as f32 * row_height;
                        let k_rect = Rect::from_min_size(
                            Pos2::new(keys_rect.min.x, row_y),
                            Vec2::new(key_width, row_height),
                        );

                        let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
                        let fill = if is_black {
                            Color32::from_rgb(0x18, 0x18, 0x18)
                        } else {
                            Color32::from_rgb(0xd8, 0xd8, 0xd8)
                        };

                        painter.rect_filled(k_rect, Rounding::ZERO, fill);
                        painter.rect_stroke(
                            k_rect,
                            Rounding::ZERO,
                            Stroke::new(0.5_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
                        );

                        if pitch.is_multiple_of(12) {
                            let octave = (pitch / 12) as i32 - 1;
                            painter.text(
                                Pos2::new(k_rect.min.x + 4.0, k_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("C{}", octave),
                                egui::FontId::proportional(9.0),
                                if is_black {
                                    Color32::WHITE
                                } else {
                                    Color32::BLACK
                                },
                            );
                        }
                    }

                    // Right Piano Roll Note Grid
                    let (grid_rect, grid_resp) = ui.allocate_exact_size(
                        Vec2::new(grid_width, total_pitches as f32 * row_height),
                        Sense::click(),
                    );
                    let grid_painter = ui.painter();

                    grid_painter.rect_filled(
                        grid_rect,
                        Rounding::ZERO,
                        Color32::from_rgb(0x22, 0x22, 0x22),
                    );

                    // Horizontal pitch lines
                    for i in 0..total_pitches {
                        let pitch = min_pitch + (total_pitches - 1 - i);
                        let row_y = grid_rect.min.y + i as f32 * row_height;
                        let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
                        let bg = if is_black {
                            Color32::from_rgb(0x1e, 0x1e, 0x1e)
                        } else {
                            Color32::from_rgb(0x26, 0x26, 0x26)
                        };

                        grid_painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(grid_rect.min.x, row_y),
                                Vec2::new(grid_width, row_height),
                            ),
                            Rounding::ZERO,
                            bg,
                        );
                        grid_painter.line_segment(
                            [
                                Pos2::new(grid_rect.min.x, row_y),
                                Pos2::new(grid_rect.max.x, row_y),
                            ],
                            Stroke::new(0.5_f32, Color32::from_rgb(0x33, 0x33, 0x33)),
                        );
                    }

                    // Vertical beat lines
                    let sixteenth_w = pixels_per_beat * 0.25;
                    let total_sixteenths = (total_beats * 4.0) as usize;
                    for s in 0..total_sixteenths {
                        let x = grid_rect.min.x + s as f32 * sixteenth_w;
                        let stroke = if s % 16 == 0 {
                            Stroke::new(1.0_f32, Color32::from_rgb(0x60, 0x60, 0x60))
                        // Bar line
                        } else if s % 4 == 0 {
                            Stroke::new(0.75_f32, Color32::from_rgb(0x45, 0x45, 0x45))
                        // Beat line
                        } else {
                            Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30))
                            // 16th line
                        };
                        grid_painter.line_segment(
                            [Pos2::new(x, grid_rect.min.y), Pos2::new(x, grid_rect.max.y)],
                            stroke,
                        );
                    }

                    // Render Notes
                    let [r, g, b, _] = parse_hex_color(&clip.color);
                    let note_color = Color32::from_rgb(r, g, b);
                    let mut note_to_remove = None;

                    for (n_idx, note) in midi_data.notes.iter().enumerate() {
                        if note.pitch >= min_pitch && note.pitch < min_pitch + total_pitches {
                            let row_idx = (min_pitch + total_pitches - 1 - note.pitch) as f32;
                            let n_y = grid_rect.min.y + row_idx * row_height;
                            let n_x = grid_rect.min.x + (note.start_beat as f32 * pixels_per_beat);
                            let n_w = (note.duration_beats as f32 * pixels_per_beat).max(4.0);

                            let n_rect = Rect::from_min_size(
                                Pos2::new(n_x, n_y + 1.0),
                                Vec2::new(n_w, row_height - 2.0),
                            );

                            grid_painter.rect_filled(n_rect, Rounding::same(2.0), note_color);
                            grid_painter.rect_stroke(
                                n_rect,
                                Rounding::same(2.0),
                                Stroke::new(1.0_f32, Color32::WHITE),
                            );

                            // Note label
                            let oct = (note.pitch / 12) as i32 - 1;
                            let name = [
                                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                            ][(note.pitch % 12) as usize];
                            grid_painter.text(
                                Pos2::new(n_rect.min.x + 3.0, n_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{}{}", name, oct),
                                egui::FontId::proportional(8.0),
                                Color32::BLACK,
                            );

                            // Check click to delete
                            if grid_resp.clicked() {
                                if let Some(pos) = grid_resp.interact_pointer_pos() {
                                    if n_rect.contains(pos) {
                                        note_to_remove = Some(n_idx);
                                    }
                                }
                            }
                        }
                    }

                    // Click to add note if not clicking on existing note
                    if grid_resp.clicked() && note_to_remove.is_none() {
                        if let Some(pos) = grid_resp.interact_pointer_pos() {
                            let rel_x = pos.x - grid_rect.min.x;
                            let rel_y = pos.y - grid_rect.min.y;

                            let beat_clicked = ((rel_x / sixteenth_w).floor() * 0.25) as f64;
                            let row_clicked = (rel_y / row_height).floor() as u8;
                            let pitch = min_pitch + total_pitches - 1 - row_clicked;

                            midi_data
                                .notes
                                .push(MidiNote::new(pitch, beat_clicked, 1.0, 100));
                        }
                    }

                    if let Some(idx) = note_to_remove {
                        midi_data.notes.remove(idx);
                    }

                    // Playhead indicator
                    let clip_pos = (playhead_beats % clip.length_beats.max(1.0)) as f32;
                    let p_x = grid_rect.min.x + clip_pos * pixels_per_beat;
                    if p_x >= grid_rect.min.x && p_x <= grid_rect.max.x {
                        grid_painter.line_segment(
                            [
                                Pos2::new(p_x, grid_rect.min.y),
                                Pos2::new(p_x, grid_rect.max.y),
                            ],
                            Stroke::new(2.0_f32, palette.amber),
                        );
                    }
                });
            });
    });
}
