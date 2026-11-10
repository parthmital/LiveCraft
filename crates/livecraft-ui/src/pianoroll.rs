use crate::Edits;
use crate::style::LivePalette;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use livecraft_model::color::parse_hex_color;
use livecraft_model::ids::ClipId;
use livecraft_model::model::{
    AudioClipData, Clip, ClipPayload, MidiClipData, MidiNote, NOTE_NAMES, ScaleConfig, ScaleType,
};
use livecraft_session::commands::clip::{SetClipLoop, SetClipRamMode};
use livecraft_session::commands::note::{AddNote, RemoveNote};
use livecraft_session::commands::scale::SetScale;
use livecraft_session::{CachedSampleAnalysis, PeakFrame};

/// Actions requested by Clip View / Piano Roll buttons for the app shell to handle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PianoRollAction {
    pub export_midi: Option<ClipId>,
    pub save_live_clip: Option<ClipId>,
    pub replace_sample_requested: Option<ClipId>,
    pub refresh_sample_requested: Option<ClipId>,
    pub export_alc_requested: bool,
    pub export_midi_requested: bool,
}

/// Pyramid level selected for rendering a waveform overview at a given canvas width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyramidLevel {
    Block256,
    Block4096,
    Block65536,
}

impl PyramidLevel {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Block256 => "256",
            Self::Block4096 => "4096",
            Self::Block65536 => "65536",
        }
    }
}

/// Chooses the `.lcanalysis` pyramid level (`block_256`, `block_4096`, `block_65536`) from the
/// ratio of total sample frames to available canvas pixel width.
#[must_use]
pub fn choose_pyramid_level(total_frames: u64, canvas_width_px: f32) -> PyramidLevel {
    let width = canvas_width_px.max(1.0);
    let frames_per_px = total_frames as f32 / width;
    if frames_per_px >= 65_536.0 {
        PyramidLevel::Block65536
    } else if frames_per_px >= 4_096.0 {
        PyramidLevel::Block4096
    } else {
        PyramidLevel::Block256
    }
}

/// Selects the peak block slice for `canvas_width_px`, falling back cleanly if a coarser level is
/// absent (for example, legacy M08 analysis sidecars).
#[must_use]
pub fn select_pyramid_blocks(
    analysis: &CachedSampleAnalysis,
    canvas_width_px: f32,
) -> (PyramidLevel, &[PeakFrame]) {
    match choose_pyramid_level(analysis.frames, canvas_width_px) {
        PyramidLevel::Block65536 if !analysis.overview_65536.is_empty() => {
            (PyramidLevel::Block65536, &analysis.overview_65536)
        }
        PyramidLevel::Block65536 | PyramidLevel::Block4096
            if !analysis.overview_4096.is_empty() =>
        {
            (PyramidLevel::Block4096, &analysis.overview_4096)
        }
        _ => (PyramidLevel::Block256, &analysis.peak_overview),
    }
}

fn format_pitch(pitch_cents: i32) -> String {
    if pitch_cents == 0 {
        "Pitch: 0 st".to_owned()
    } else if pitch_cents % 100 == 0 {
        format!("Pitch: {:+} st", pitch_cents / 100)
    } else {
        format!(
            "Pitch: {:+.2} st ({:+} ct)",
            pitch_cents as f32 / 100.0,
            pitch_cents
        )
    }
}

fn clip_playhead_x(clip: &Clip, playhead_beats: f64, canvas_rect: Rect) -> f32 {
    let clip_len = clip.length_beats.max(1.0);
    let clip_pos = if clip.is_looping {
        playhead_beats.rem_euclid(clip_len)
    } else {
        playhead_beats.clamp(0.0, clip_len)
    };
    let norm = (clip_pos / clip_len).clamp(0.0, 1.0) as f32;
    canvas_rect.min.x + norm * canvas_rect.width()
}

fn render_common_clip_header(ui: &mut Ui, clip: &Clip, palette: &LivePalette, edits: &mut Edits) {
    crate::widgets::live_lcd_box(ui, &clip.name, Vec2::new(90.0, 22.0), palette.amber, true);

    let len_str = format!("{:.1} Bars", clip.length_beats / 4.0);
    crate::widgets::live_lcd_box(ui, &len_str, Vec2::new(60.0, 22.0), palette.amber, false);

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
}

/// Renders the Audio Clip View (manual 8.4.5, 8.12): header controls (`RAM`, `Replace Sample...`,
/// `Reload`, `Save .alc`), sample metadata, and `.lcanalysis` 3-level waveform overview canvas.
pub fn render_audio_clip_view(
    ui: &mut Ui,
    clip: &Clip,
    audio: &AudioClipData,
    playhead_beats: f64,
    palette: &LivePalette,
    sample_analysis: Option<&CachedSampleAnalysis>,
    edits: &mut Edits,
) -> PianoRollAction {
    let mut action = PianoRollAction::default();
    let has_path = !audio.file_path.trim().is_empty()
        || audio
            .relative_path
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

            render_common_clip_header(ui, clip, palette, edits);
            crate::widgets::live_divider(ui, 22.0);

            ui.label(
                egui::RichText::new(format!("Gain: {:+.1} dB", audio.gain_db))
                    .size(10.0)
                    .monospace()
                    .color(palette.control_foreground),
            );
            ui.label(
                egui::RichText::new(format_pitch(audio.pitch_cents))
                    .size(10.0)
                    .monospace()
                    .color(palette.control_foreground),
            );

            crate::widgets::live_divider(ui, 22.0);

            let ram_btn = crate::widgets::live_toggle_button(
                ui,
                audio.ram_mode,
                palette.amber,
                "RAM",
                Vec2::new(38.0, 22.0),
            );
            if ram_btn.clicked() {
                edits.push(Box::new(SetClipRamMode::new(clip.id, !audio.ram_mode)));
            }

            if ui
                .button(egui::RichText::new("Replace Sample...").size(10.0))
                .clicked()
            {
                action.replace_sample_requested = Some(clip.id);
            }
            if ui
                .button(egui::RichText::new("Reload").size(10.0))
                .clicked()
            {
                action.refresh_sample_requested = Some(clip.id);
            }
            if ui
                .button(egui::RichText::new("Save .alc").size(10.0))
                .clicked()
            {
                action.save_live_clip = Some(clip.id);
                action.export_alc_requested = true;
            }
        });

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            let sample_label = audio
                .relative_path
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    (!audio.file_path.trim().is_empty()).then_some(audio.file_path.as_str())
                })
                .unwrap_or("(no sample)");
            ui.label(
                egui::RichText::new(format!("Sample: {sample_label}"))
                    .size(10.0)
                    .monospace()
                    .color(palette.control_foreground),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{:.1} kHz | {} ch | {}",
                    f64::from(audio.sample_rate) / 1000.0,
                    audio.channels,
                    if audio.is_warped {
                        format!("Warp: {:?}", audio.warp_mode)
                    } else {
                        "Unwarped".to_owned()
                    }
                ))
                .size(10.0)
                .monospace()
                .color(palette.control_disabled),
            );
            if let Some(analysis) = sample_analysis {
                let (level, _) = select_pyramid_blocks(analysis, ui.available_width().max(120.0));
                ui.label(
                    egui::RichText::new(format!(
                        "Pyramid: {} ({} frames)",
                        level.label(),
                        analysis.frames
                    ))
                    .size(9.5)
                    .monospace()
                    .color(palette.amber),
                );
            } else {
                let state_text = if has_path { "Offline" } else { "Empty" };
                ui.label(
                    egui::RichText::new(state_text)
                        .size(9.5)
                        .monospace()
                        .color(palette.record_red),
                );
            }
        });

        ui.separator();

        let avail = ui.available_size();
        let canvas_size = Vec2::new(avail.x.max(120.0), avail.y.max(48.0));
        let (canvas_rect, _) = ui.allocate_exact_size(canvas_size, Sense::hover());

        if ui.is_rect_visible(canvas_rect) {
            let painter = ui.painter();
            let is_dark = palette.is_dark();
            let canvas_bg = if is_dark {
                Color32::from_rgb(0x18, 0x18, 0x18)
            } else {
                Color32::from_rgb(0xc8, 0xc8, 0xc6)
            };
            painter.rect_filled(canvas_rect, CornerRadius::same(2), canvas_bg);
            painter.rect_stroke(
                canvas_rect,
                CornerRadius::same(2),
                Stroke::new(1.0_f32, palette.panel_border),
                eframe::egui::StrokeKind::Middle,
            );

            let center_y = canvas_rect.center().y;
            painter.line_segment(
                [
                    Pos2::new(canvas_rect.min.x + 1.0, center_y),
                    Pos2::new(canvas_rect.max.x - 1.0, center_y),
                ],
                Stroke::new(0.75_f32, palette.grid_line),
            );

            if let Some(analysis) = sample_analysis {
                let (_, peaks) = select_pyramid_blocks(analysis, canvas_rect.width());
                if peaks.is_empty() {
                    painter.text(
                        canvas_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Empty Audio Waveform",
                        egui::FontId::proportional(11.0),
                        palette.control_disabled,
                    );
                } else {
                    let [r, g, b, _] = parse_hex_color(&clip.color);
                    let wave_color = Color32::from_rgb(r, g, b);
                    let inner_w = (canvas_rect.width() - 2.0).max(1.0);
                    let px_count = (inner_w.floor() as usize).max(1);
                    let half_h = (canvas_rect.height() * 0.5 - 3.0).max(1.0);
                    let start_x = canvas_rect.min.x + 1.0;

                    for px in 0..px_count {
                        let start_idx = (px * peaks.len()) / px_count;
                        let end_idx = (((px + 1) * peaks.len()) / px_count)
                            .max(start_idx + 1)
                            .min(peaks.len());
                        let mut min_v = 1.0_f32;
                        let mut max_v = -1.0_f32;
                        for peak in &peaks[start_idx..end_idx] {
                            if peak.min.is_finite() {
                                min_v = min_v.min(peak.min);
                            }
                            if peak.max.is_finite() {
                                max_v = max_v.max(peak.max);
                            }
                        }
                        if min_v > max_v {
                            min_v = 0.0;
                            max_v = 0.0;
                        }
                        min_v = min_v.clamp(-1.0, 1.0);
                        max_v = max_v.clamp(-1.0, 1.0);

                        let y_top = center_y - max_v * half_h;
                        let y_bot = center_y - min_v * half_h;
                        let (bar_top, bar_bot) = if (y_bot - y_top).abs() < 1.0 {
                            (center_y - 0.5, center_y + 0.5)
                        } else {
                            (y_top.min(y_bot), y_top.max(y_bot))
                        };
                        let x = start_x + px as f32 + 0.5;
                        painter.line_segment(
                            [Pos2::new(x, bar_top), Pos2::new(x, bar_bot)],
                            Stroke::new(1.0_f32, wave_color),
                        );
                    }
                }
            } else {
                let placeholder = if has_path {
                    "Sample Offline"
                } else {
                    "No Sample Loaded"
                };
                painter.text(
                    canvas_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    placeholder,
                    egui::FontId::proportional(11.5),
                    palette.control_disabled,
                );
            }

            let p_x = clip_playhead_x(clip, playhead_beats, canvas_rect);
            painter.line_segment(
                [
                    Pos2::new(p_x, canvas_rect.min.y + 1.0),
                    Pos2::new(p_x, canvas_rect.max.y - 1.0),
                ],
                Stroke::new(2.0_f32, palette.amber),
            );
        }
    });

    action
}

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
    sample_analysis: Option<&CachedSampleAnalysis>,
    edits: &mut Edits,
) -> PianoRollAction {
    if let ClipPayload::Audio(audio) = &clip.payload {
        return render_audio_clip_view(
            ui,
            clip,
            audio,
            playhead_beats,
            palette,
            sample_analysis,
            edits,
        );
    }

    let ClipPayload::Midi(midi_data) = &clip.payload else {
        return PianoRollAction::default();
    };

    let mut action = PianoRollAction::default();

    // Retrieve or initialize Fold state in egui memory
    let fold_id = ui.scope_id().with("pianoroll_fold_scale");
    let mut fold_scale: bool = ui.data_mut(|d| *d.get_temp_mut_or(fold_id, false));

    ui.vertical(|ui| {
        // Clip Header: Name, Length, Loop, Scale Controls & Export actions (Manual p. 201, 252)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

            render_common_clip_header(ui, clip, palette, edits);
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

            crate::widgets::live_divider(ui, 22.0);
            if ui
                .button(egui::RichText::new("Export MIDI").size(10.0))
                .clicked()
            {
                action.export_midi = Some(clip.id);
                action.export_midi_requested = true;
            }
            if ui
                .button(egui::RichText::new("Save .alc").size(10.0))
                .clicked()
            {
                action.save_live_clip = Some(clip.id);
                action.export_alc_requested = true;
            }
        });

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

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_analysis(frames: u64) -> CachedSampleAnalysis {
        CachedSampleAnalysis {
            version: 1,
            sample_rate: 44_100,
            channels: 2,
            frames,
            overview_block_frames: 256,
            peak_overview: vec![PeakFrame {
                min: -0.5,
                max: 0.5,
            }],
            overview_4096: vec![PeakFrame {
                min: -0.6,
                max: 0.6,
            }],
            overview_65536: vec![PeakFrame {
                min: -0.7,
                max: 0.7,
            }],
            source_len_bytes: Some(1024),
            source_mtime_unix_secs: Some(1),
            metrics: None,
        }
    }

    #[test]
    fn choose_pyramid_level_picks_resolution_from_frames_per_pixel() {
        assert_eq!(choose_pyramid_level(44_100, 800.0), PyramidLevel::Block256);
        assert_eq!(
            choose_pyramid_level(5_000_000, 800.0),
            PyramidLevel::Block4096
        );
        assert_eq!(
            choose_pyramid_level(100_000_000, 800.0),
            PyramidLevel::Block65536
        );
    }

    #[test]
    fn select_pyramid_blocks_falls_back_when_coarse_levels_empty() {
        let mut legacy = sample_analysis(100_000_000);
        legacy.overview_65536.clear();
        assert_eq!(
            select_pyramid_blocks(&legacy, 800.0).0,
            PyramidLevel::Block4096
        );

        legacy.overview_4096.clear();
        assert_eq!(
            select_pyramid_blocks(&legacy, 800.0).0,
            PyramidLevel::Block256
        );
    }
}
