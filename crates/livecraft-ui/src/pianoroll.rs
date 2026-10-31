use crate::Edits;
use crate::style::LivePalette;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use livecraft_model::color::parse_hex_color;
use livecraft_model::model::{
    Clip, ClipPayload, MidiClipData, MidiNote, NOTE_NAMES, ScaleConfig, ScaleType,
};
use livecraft_session::commands::clip::SetClipLoop;
use livecraft_session::commands::note::{AddNote, RemoveNote};
use livecraft_session::commands::scale::SetScale;

/// Replaces a clip's notes with the result of a tool, as one undo step: the old notes are removed,
/// then the new notes are added.
fn apply_tool(
    edits: &mut Edits,
    clip: &Clip,
    midi: &MidiClipData,
    tool: impl FnOnce(&mut MidiClipData),
) {
    let mut result = midi.clone();
    tool(&mut result);
    for index in (0..midi.notes.len()).rev() {
        edits.push(Box::new(RemoveNote::new(clip.id, index)));
    }
    for note in result.notes {
        edits.push(Box::new(AddNote::new(clip.id, note)));
    }
}

pub fn render_piano_roll(
    ui: &mut Ui,
    clip: &Clip,
    scale: &ScaleConfig,
    playhead_beats: f64,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    let ClipPayload::Midi(midi_data) = &clip.payload else {
        ui.label("Audio Clip Waveform Editor");
        return;
    };

    let key_width = 40.0;
    let row_height = 9.5;
    let (note_min, note_max) = if !midi_data.notes.is_empty() {
        let n_min = midi_data.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let n_max = midi_data.notes.iter().map(|n| n.pitch).max().unwrap_or(72);
        (n_min, n_max)
    } else {
        (60, 72)
    };
    let max_pitch = (note_max + 1).min(127);
    let min_pitch = note_min.saturating_sub(1);
    let total_pitches = (max_pitch.saturating_sub(min_pitch) + 1).max(12);
    let total_beats = clip.length_beats.max(4.0) as f32;
    let pixels_per_beat = 48.0;
    let grid_width = total_beats * pixels_per_beat;

    // Retrieve or initialize Fold state in egui memory
    let fold_id = ui.scope_id().with("pianoroll_fold_scale");
    let mut fold_scale: bool = ui.data_mut(|d| *d.get_temp_mut_or(fold_id, false));

    // Determine list of visible pitches (folded or unfolded)
    let visible_pitches: Vec<u8> = if fold_scale && scale.is_active {
        (min_pitch..min_pitch + total_pitches)
            .rev()
            .filter(|&p| scale.is_in_scale(p))
            .collect()
    } else {
        (min_pitch..min_pitch + total_pitches).rev().collect()
    };
    let num_rows = visible_pitches.len();

    ui.vertical(|ui| {
        // Clip Header: Name, Length, Loop, Scale Controls & MIDI Tools (Manual p. 201, 252)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

            // Clip Name LCD
            crate::widgets::live_lcd_box(
                ui,
                &clip.name,
                Vec2::new(90.0, 22.0),
                palette.amber,
                true,
            );

            // Length LCD
            let len_str = format!("{:.1} Bars", clip.length_beats / 4.0);
            crate::widgets::live_lcd_box(ui, &len_str, Vec2::new(60.0, 22.0), palette.amber, false);

            // Loop Toggle Button
            let loop_btn = crate::widgets::live_toggle_button(
                ui,
                clip.is_looping,
                palette.amber,
                "Loop",
                Vec2::new(40.0, 22.0),
            );
            if loop_btn.clicked() {
                edits.push(Box::new(SetClipLoop::new(
                    clip.id,
                    clip.loop_start_beat,
                    clip.loop_end_beat,
                    !clip.is_looping,
                )));
            }

            crate::widgets::live_divider(ui, 22.0);

            // Ableton 12 Scale Controls
            let scale_btn = crate::widgets::live_toggle_button(
                ui,
                scale.is_active,
                palette.amber,
                "Scale",
                Vec2::new(42.0, 22.0),
            );
            if scale_btn.clicked() {
                edits.push(Box::new(SetScale::new(ScaleConfig {
                    is_active: !scale.is_active,
                    ..*scale
                })));
            }

            if scale.is_active {
                let current_root = NOTE_NAMES[scale.root_note as usize % 12];
                let mut root_note = scale.root_note;
                egui::ComboBox::from_id_salt("scale_root_select")
                    .selected_text(current_root)
                    .width(36.0)
                    .show_ui(ui, |ui| {
                        for (idx, &name) in NOTE_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut root_note, idx as u8, name);
                        }
                    });
                if root_note != scale.root_note {
                    let changed = ScaleConfig {
                        root_note,
                        ..*scale
                    };
                    edits.push(Box::new(SetScale::new(changed)));
                }

                let mut scale_type = scale.scale_type;
                egui::ComboBox::from_id_salt("scale_type_select")
                    .selected_text(scale.scale_type.name())
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for &st in ScaleType::ALL {
                            ui.selectable_value(&mut scale_type, st, st.name());
                        }
                    });

                if scale_type != scale.scale_type {
                    let changed = ScaleConfig {
                        scale_type,
                        ..*scale
                    };
                    edits.push(Box::new(SetScale::new(changed)));
                }

                let fold_btn = crate::widgets::live_toggle_button(
                    ui,
                    fold_scale,
                    palette.amber,
                    "Fold",
                    Vec2::new(36.0, 22.0),
                );
                if fold_btn.clicked() {
                    fold_scale = !fold_scale;
                    ui.data_mut(|d| d.insert_temp(fold_id, fold_scale));
                }
            }
        });

        // MIDI Transformations Toolbar
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.label(
                egui::RichText::new("Tools:")
                    .size(10.0)
                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
            );

            if ui
                .button(egui::RichText::new("Quantize 1/16").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| d.quantize(0.25));
            }

            if ui.button(egui::RichText::new("+1st").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| {
                    d.transpose(1, if scale.is_active { Some(scale) } else { None })
                });
            }
            if ui.button(egui::RichText::new("-1st").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| {
                    d.transpose(-1, if scale.is_active { Some(scale) } else { None })
                });
            }
            if ui.button(egui::RichText::new("+12st").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| d.transpose(12, None));
            }
            if ui.button(egui::RichText::new("-12st").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| d.transpose(-12, None));
            }
            if ui.button(egui::RichText::new("Strum").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| d.strum(0.10, true));
            }
            if ui
                .button(egui::RichText::new("Humanize").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| d.humanize(0.04, 12, 12345));
            }
            if ui
                .button(egui::RichText::new("Invert").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| d.invert_pitches());
            }
            if ui
                .button(egui::RichText::new("Reverse").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| {
                    d.reverse_time(clip.length_beats)
                });
            }
            if ui
                .button(egui::RichText::new("Ramp Vel").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| d.velocity_ramp(50, 115));
            }
            if ui.button(egui::RichText::new("Chop").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| d.chop(2));
            }
            if ui
                .button(egui::RichText::new("Ornament").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| d.ornament(0.0625, 0.7));
            }
            if ui.button(egui::RichText::new("Span").size(10.0)).clicked() {
                apply_tool(edits, clip, midi_data, |d| d.span());
            }
            if ui
                .button(egui::RichText::new("Euclid").size(10.0))
                .clicked()
            {
                let p = if scale.is_active {
                    scale.snap_to_scale(60)
                } else {
                    60
                };
                apply_tool(edits, clip, midi_data, |d| {
                    d.generate_euclidean(3, 8, p, 4.0, 100)
                });
            }
            if ui
                .button(egui::RichText::new("Stacks").size(10.0))
                .clicked()
            {
                apply_tool(edits, clip, midi_data, |d| {
                    d.generate_stacks(
                        &[0, 4, 7],
                        60,
                        2.0,
                        100,
                        if scale.is_active { Some(scale) } else { None },
                    );
                });
            }
        });

        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left Piano Keyboard
                    let (keys_rect, _) = ui.allocate_exact_size(
                        Vec2::new(key_width, num_rows as f32 * row_height),
                        Sense::hover(),
                    );
                    let painter = ui.painter();

                    for (row_idx, &pitch) in visible_pitches.iter().enumerate() {
                        let row_y = keys_rect.min.y + row_idx as f32 * row_height;
                        let k_rect = Rect::from_min_size(
                            Pos2::new(keys_rect.min.x, row_y),
                            Vec2::new(key_width, row_height),
                        );

                        let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
                        let is_root = scale.is_active && (pitch % 12 == scale.root_note % 12);
                        let in_scale = scale.is_in_scale(pitch);

                        let fill = if is_root {
                            Color32::from_rgb(0x4a, 0x38, 0x18) // Amber tint for root
                        } else if is_black {
                            Color32::from_rgb(0x18, 0x18, 0x18)
                        } else if in_scale && scale.is_active {
                            Color32::from_rgb(0xe0, 0xe0, 0xd8)
                        } else {
                            Color32::from_rgb(0xc8, 0xc8, 0xc8)
                        };

                        painter.rect_filled(k_rect, CornerRadius::ZERO, fill);
                        painter.rect_stroke(
                            k_rect,
                            CornerRadius::ZERO,
                            Stroke::new(0.5_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
                            eframe::egui::StrokeKind::Middle,
                        );

                        // Draw key note label
                        let label = livecraft_model::model::note_name(pitch);
                        let label_color = if is_root {
                            palette.amber
                        } else if is_black {
                            Color32::WHITE
                        } else {
                            Color32::BLACK
                        };

                        // Show note label on C notes, roots, or when folded
                        if pitch.is_multiple_of(12) || is_root || fold_scale {
                            painter.text(
                                Pos2::new(k_rect.min.x + 3.0, k_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(8.5),
                                label_color,
                            );
                        }
                    }

                    // Right Piano Roll Note Grid
                    let (grid_rect, grid_resp) = ui.allocate_exact_size(
                        Vec2::new(grid_width, num_rows as f32 * row_height),
                        Sense::click(),
                    );
                    let grid_painter = ui.painter();
                    let is_dark = palette.is_dark();

                    let grid_base_bg = if is_dark {
                        Color32::from_rgb(0x22, 0x22, 0x22)
                    } else {
                        Color32::from_rgb(0xd4, 0xd4, 0xd2)
                    };
                    grid_painter.rect_filled(grid_rect, CornerRadius::ZERO, grid_base_bg);

                    // Horizontal pitch lines
                    for (row_idx, &pitch) in visible_pitches.iter().enumerate() {
                        let row_y = grid_rect.min.y + row_idx as f32 * row_height;
                        let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
                        let is_root = scale.is_active && (pitch % 12 == scale.root_note % 12);
                        let in_scale = scale.is_in_scale(pitch);

                        let bg = if is_root {
                            if is_dark {
                                Color32::from_rgb(0x2a, 0x26, 0x1a)
                            } else {
                                Color32::from_rgb(0xf2, 0xe8, 0xd0)
                            }
                        } else if in_scale && scale.is_active {
                            if is_dark {
                                Color32::from_rgb(0x26, 0x26, 0x28)
                            } else {
                                Color32::from_rgb(0xde, 0xde, 0xdc)
                            }
                        } else if is_black {
                            if is_dark {
                                Color32::from_rgb(0x1a, 0x1a, 0x1a)
                            } else {
                                Color32::from_rgb(0xce, 0xce, 0xcc)
                            }
                        } else {
                            if is_dark {
                                Color32::from_rgb(0x22, 0x22, 0x22)
                            } else {
                                Color32::from_rgb(0xd8, 0xd8, 0xd6)
                            }
                        };

                        grid_painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(grid_rect.min.x, row_y),
                                Vec2::new(grid_width, row_height),
                            ),
                            CornerRadius::ZERO,
                            bg,
                        );

                        let line_stroke = if is_root {
                            Stroke::new(
                                0.75_f32,
                                if is_dark {
                                    Color32::from_rgb(0x60, 0x4d, 0x25)
                                } else {
                                    Color32::from_rgb(0xc0, 0xa0, 0x60)
                                },
                            )
                        } else {
                            Stroke::new(
                                0.5_f32,
                                if is_dark {
                                    Color32::from_rgb(0x30, 0x30, 0x30)
                                } else {
                                    Color32::from_rgb(0xb8, 0xb8, 0xb6)
                                },
                            )
                        };

                        grid_painter.line_segment(
                            [
                                Pos2::new(grid_rect.min.x, row_y),
                                Pos2::new(grid_rect.max.x, row_y),
                            ],
                            line_stroke,
                        );
                    }

                    // Vertical beat lines
                    let sixteenth_w = pixels_per_beat * 0.25;
                    let total_sixteenths = (total_beats * 4.0) as usize;
                    for s in 0..total_sixteenths {
                        let x = grid_rect.min.x + s as f32 * sixteenth_w;
                        let stroke = if s % 16 == 0 {
                            Stroke::new(1.0_f32, Color32::from_rgb(0x60, 0x60, 0x60))
                        } else if s % 4 == 0 {
                            Stroke::new(0.75_f32, Color32::from_rgb(0x45, 0x45, 0x45))
                        } else {
                            Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30))
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
                        if let Some(row_idx) = visible_pitches.iter().position(|&p| p == note.pitch)
                        {
                            let n_y = grid_rect.min.y + row_idx as f32 * row_height;
                            let n_x = grid_rect.min.x + (note.start_beat as f32 * pixels_per_beat);
                            let n_w = (note.duration_beats as f32 * pixels_per_beat).max(4.0);

                            let n_rect = Rect::from_min_size(
                                Pos2::new(n_x, n_y + 1.0),
                                Vec2::new(n_w, row_height - 2.0),
                            );

                            grid_painter.rect_filled(n_rect, CornerRadius::same(2), note_color);
                            grid_painter.rect_stroke(
                                n_rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0_f32, Color32::WHITE),
                                eframe::egui::StrokeKind::Middle,
                            );

                            // Note label
                            let label = livecraft_model::model::note_name(note.pitch);
                            grid_painter.text(
                                Pos2::new(n_rect.min.x + 3.0, n_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(8.0),
                                Color32::BLACK,
                            );

                            // Click to delete
                            if grid_resp.clicked()
                                && let Some(pos) = grid_resp.interact_pointer_pos()
                                && n_rect.contains(pos)
                            {
                                note_to_remove = Some(n_idx);
                            }
                        }
                    }

                    // Click to add note if not clicking on an existing note
                    if grid_resp.clicked()
                        && note_to_remove.is_none()
                        && let Some(pos) = grid_resp.interact_pointer_pos()
                    {
                        let rel_x = pos.x - grid_rect.min.x;
                        let rel_y = pos.y - grid_rect.min.y;

                        let beat_clicked = ((rel_x / sixteenth_w).floor() * 0.25) as f64;
                        let row_clicked = (rel_y / row_height).floor() as usize;

                        if row_clicked < visible_pitches.len() {
                            let pitch = visible_pitches[row_clicked];
                            edits.push(Box::new(AddNote::new(
                                clip.id,
                                MidiNote::new(pitch, beat_clicked, 1.0, 100),
                            )));
                        }
                    }

                    if let Some(idx) = note_to_remove {
                        edits.push(Box::new(RemoveNote::new(clip.id, idx)));
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
