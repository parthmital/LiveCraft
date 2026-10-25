use crate::style::LivePalette;
use crate::widgets::{live_divider, live_lcd_box, live_toggle_button};
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::{Project, ScaleType, NOTE_NAMES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Session,
    Arrangement,
}

pub struct TransportBarResponse {
    pub play_toggled: bool,
    pub stop_clicked: bool,
    pub record_toggled: bool,
    pub view_mode_changed: Option<ViewMode>,
    pub browser_toggled: bool,
}

pub fn render_transport_bar(
    ui: &mut Ui,
    project: &mut Project,
    current_view: ViewMode,
    palette: &LivePalette,
    cpu_percent: f32,
    browser_visible: bool,
) -> TransportBarResponse {
    let mut resp = TransportBarResponse {
        play_toggled: false,
        stop_clicked: false,
        record_toggled: false,
        view_mode_changed: None,
        browser_toggled: false,
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

        // -------------------------------------------------------------
        // SECTION 1: Browser Options (Manual p. 35)
        // -------------------------------------------------------------
        let (b_rect, b_resp) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
        if ui.is_rect_visible(b_rect) {
            let painter = ui.painter();
            let bg = if b_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(b_rect, Rounding::same(2.0), bg);
            let border_color = if browser_visible {
                palette.amber
            } else {
                Color32::from_rgb(0x38, 0x38, 0x38)
            };
            painter.rect_stroke(
                b_rect,
                Rounding::same(2.0),
                Stroke::new(1.0_f32, border_color),
            );

            // Sidebar icon: vertical line + left arrow / amber indicator
            let c = b_rect.center();
            painter.line_segment(
                [
                    Pos2::new(c.x - 4.0, c.y - 6.0),
                    Pos2::new(c.x - 4.0, c.y + 6.0),
                ],
                Stroke::new(
                    1.5_f32,
                    if browser_visible {
                        palette.amber
                    } else {
                        palette.control_foreground
                    },
                ),
            );
            let arrow_color = if browser_visible {
                palette.amber
            } else {
                Color32::from_rgb(0x88, 0x88, 0x88)
            };
            if browser_visible {
                let p1 = Pos2::new(c.x + 3.0, c.y - 4.0);
                let p2 = Pos2::new(c.x + 3.0, c.y + 4.0);
                let p3 = Pos2::new(c.x - 1.0, c.y);
                painter.add(egui::Shape::convex_polygon(
                    vec![p1, p2, p3],
                    arrow_color,
                    Stroke::NONE,
                ));
            } else {
                let p1 = Pos2::new(c.x - 1.0, c.y - 4.0);
                let p2 = Pos2::new(c.x - 1.0, c.y + 4.0);
                let p3 = Pos2::new(c.x + 3.0, c.y);
                painter.add(egui::Shape::convex_polygon(
                    vec![p1, p2, p3],
                    arrow_color,
                    Stroke::NONE,
                ));
            }
        }
        if b_resp.clicked() {
            resp.browser_toggled = true;
        }

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 2: Tempo Settings and Metronome (Manual p. 36)
        // -------------------------------------------------------------
        // Link toggle
        let _link_btn = live_toggle_button(ui, false, palette.amber, "Link", Vec2::new(32.0, 24.0));

        // Follow arrangement toggle
        let _follow_btn =
            live_toggle_button(ui, true, palette.amber, "Follow", Vec2::new(42.0, 24.0));

        // TAP tempo button
        let tap_btn = live_toggle_button(ui, false, palette.amber, "TAP", Vec2::new(30.0, 24.0));
        if tap_btn.clicked() {
            // TAP tempo
        }

        // Tempo BPM LCD well with nudge
        let mut bpm = project.transport.bpm;
        let bpm_str = format!("{:.2}", bpm);
        let bpm_lcd = live_lcd_box(ui, &bpm_str, Vec2::new(56.0, 24.0), palette.amber, false);
        if bpm_lcd.dragged() {
            bpm = (bpm - bpm_lcd.drag_delta().y * 0.2).clamp(20.0, 999.0);
            project.transport.bpm = bpm;
        }

        // Nudge buttons
        let nudge_down = live_toggle_button(ui, false, palette.amber, "<", Vec2::new(16.0, 24.0));
        if nudge_down.clicked() {
            project.transport.bpm = (project.transport.bpm - 0.5).max(20.0);
        }
        let nudge_up = live_toggle_button(ui, false, palette.amber, ">", Vec2::new(16.0, 24.0));
        if nudge_up.clicked() {
            project.transport.bpm = (project.transport.bpm + 0.5).min(999.0);
        }

        // Time Signature LCD well
        let time_sig_str = format!(
            "{}/{}",
            project.transport.time_signature.0, project.transport.time_signature.1
        );
        let _sig_lcd = live_lcd_box(
            ui,
            &time_sig_str,
            Vec2::new(34.0, 24.0),
            palette.amber,
            false,
        );

        // Metronome Button with authentic Live dual-circle symbol
        let metro_active = project.transport.metronome_enabled;
        let (metro_rect, metro_resp) =
            ui.allocate_exact_size(Vec2::new(28.0, 24.0), Sense::click());
        if ui.is_rect_visible(metro_rect) {
            let painter = ui.painter();
            let bg = if metro_active {
                palette.amber
            } else if metro_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(metro_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                metro_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if metro_active {
                        palette.amber
                    } else {
                        Color32::from_rgb(0x3a, 0x3a, 0x3a)
                    },
                ),
            );

            let icon_color = if metro_active {
                Color32::BLACK
            } else {
                palette.control_foreground
            };
            let c1 = Pos2::new(metro_rect.center().x - 4.5, metro_rect.center().y);
            let c2 = Pos2::new(metro_rect.center().x + 4.5, metro_rect.center().y);
            painter.circle_stroke(c1, 3.0, Stroke::new(1.2_f32, icon_color));
            painter.circle_filled(c2, 3.0, icon_color);
        }
        if metro_resp.clicked() {
            project.transport.metronome_enabled = !project.transport.metronome_enabled;
        }

        // Quantization Menu
        let _quant_btn =
            live_toggle_button(ui, false, palette.amber, "1 Bar", Vec2::new(42.0, 24.0));

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 3: Scale & Tuning (Live 12 New Flagship Feature! Manual p. 36)
        // -------------------------------------------------------------
        // Purple musical badge 'b#'
        let scale_active = project.scale.is_active;
        let (badge_rect, badge_resp) =
            ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
        if ui.is_rect_visible(badge_rect) {
            let painter = ui.painter();
            let bg = if scale_active {
                Color32::from_rgb(0x81, 0x4c, 0xff) // Purple Ableton scale active
            } else {
                palette.button_idle
            };
            painter.rect_filled(badge_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                badge_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if scale_active {
                        Color32::from_rgb(0x99, 0x66, 0xff)
                    } else {
                        Color32::from_rgb(0x38, 0x38, 0x38)
                    },
                ),
            );
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                "b#",
                egui::FontId::monospace(10.5),
                if scale_active {
                    Color32::WHITE
                } else {
                    palette.control_disabled
                },
            );
        }
        if badge_resp.clicked() {
            project.scale.is_active = !project.scale.is_active;
        }

        let root_idx = (project.scale.root_note % 12) as usize;
        egui::ComboBox::from_id_salt("transport_scale_root")
            .selected_text(NOTE_NAMES[root_idx])
            .width(36.0)
            .show_ui(ui, |ui| {
                for (idx, &name) in NOTE_NAMES.iter().enumerate() {
                    ui.selectable_value(&mut project.scale.root_note, idx as u8, name);
                }
            });

        egui::ComboBox::from_id_salt("transport_scale_type")
            .selected_text(project.scale.scale_type.name())
            .width(72.0)
            .show_ui(ui, |ui| {
                for &st in ScaleType::ALL {
                    ui.selectable_value(&mut project.scale.scale_type, st, st.name());
                }
            });

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 4: Arrangement Position Display (Manual p. 36)
        // -------------------------------------------------------------
        let beats = project.transport.playhead_beats;
        let bar = (beats / 4.0) as u32 + 1;
        let beat = (beats % 4.0) as u32 + 1;
        let sixteenth = ((beats * 4.0) % 4.0) as u32 + 1;
        let pos_str = format!("{:2} . {:1} . {:1}", bar, beat, sixteenth);
        let pos_lcd = live_lcd_box(
            ui,
            &pos_str,
            Vec2::new(66.0, 24.0),
            palette.amber,
            project.transport.is_playing,
        );
        if pos_lcd.dragged() {
            let delta = -pos_lcd.drag_delta().y * 0.25;
            project.transport.playhead_beats =
                (project.transport.playhead_beats + delta as f64).max(0.0);
        }

        // -------------------------------------------------------------
        // SECTION 5: Transport Controls (Play, Stop, Record) (Manual p. 36)
        // -------------------------------------------------------------
        let is_playing = project.transport.is_playing;
        let (play_rect, play_resp) = ui.allocate_exact_size(Vec2::new(30.0, 24.0), Sense::click());
        if ui.is_rect_visible(play_rect) {
            let painter = ui.painter();
            let bg = if is_playing {
                Color32::from_rgb(0x1a, 0x48, 0x24)
            } else if play_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(play_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                play_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if is_playing {
                        palette.play_green
                    } else {
                        Color32::from_rgb(0x3a, 0x3a, 0x3a)
                    },
                ),
            );

            let c = play_rect.center();
            let arrow_color = if is_playing {
                palette.play_green
            } else {
                Color32::from_rgb(0xaa, 0xaa, 0xaa)
            };
            let p1 = Pos2::new(c.x - 4.0, c.y - 5.5);
            let p2 = Pos2::new(c.x - 4.0, c.y + 5.5);
            let p3 = Pos2::new(c.x + 5.5, c.y);
            painter.add(egui::Shape::convex_polygon(
                vec![p1, p2, p3],
                arrow_color,
                Stroke::NONE,
            ));
        }
        if play_resp.clicked() {
            project.transport.is_playing = !project.transport.is_playing;
            resp.play_toggled = true;
        }

        // Stop Button
        let (stop_rect, stop_resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
        if ui.is_rect_visible(stop_rect) {
            let painter = ui.painter();
            let bg = if stop_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(stop_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                stop_rect,
                Rounding::same(2.0),
                Stroke::new(1.0_f32, Color32::from_rgb(0x3a, 0x3a, 0x3a)),
            );
            let c = stop_rect.center();
            let sq = Rect::from_center_size(c, Vec2::new(8.0, 8.0));
            painter.rect_filled(sq, Rounding::ZERO, Color32::from_rgb(0xbb, 0xbb, 0xbb));
        }
        if stop_resp.clicked() {
            project.transport.is_playing = false;
            project.transport.playhead_beats = 0.0;
            resp.stop_clicked = true;
        }

        // Arrangement Record Button
        let is_recording = project.transport.is_recording;
        let (rec_rect, rec_resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
        if ui.is_rect_visible(rec_rect) {
            let painter = ui.painter();
            let bg = if is_recording {
                Color32::from_rgb(0x4a, 0x16, 0x16)
            } else if rec_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(rec_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                rec_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if is_recording {
                        palette.record_red
                    } else {
                        Color32::from_rgb(0x3a, 0x3a, 0x3a)
                    },
                ),
            );
            let c = rec_rect.center();
            let dot_color = if is_recording {
                palette.record_red
            } else {
                Color32::from_rgb(0xcc, 0x60, 0x60)
            };
            painter.circle_filled(c, 5.0, dot_color);
        }
        if rec_resp.clicked() {
            project.transport.is_recording = !project.transport.is_recording;
            resp.record_toggled = true;
        }

        // MIDI Overdub & Automation Arm
        let _overdub_btn = live_toggle_button(ui, false, palette.amber, "+", Vec2::new(20.0, 24.0));

        let (arm_rect, arm_resp) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::click());
        if ui.is_rect_visible(arm_rect) {
            let painter = ui.painter();
            let is_dark = ui.visuals().dark_mode;
            let bg = if arm_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(arm_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                arm_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if is_dark {
                        Color32::from_rgb(0x38, 0x38, 0x38)
                    } else {
                        Color32::from_rgb(0x9c, 0x9c, 0x9a)
                    },
                ),
            );
            let c = arm_rect.center();
            painter.circle_stroke(
                Pos2::new(c.x - 3.0, c.y),
                3.0,
                Stroke::new(1.5_f32, palette.amber),
            );
            painter.circle_stroke(
                Pos2::new(c.x + 3.0, c.y),
                3.0,
                Stroke::new(1.5_f32, palette.amber),
            );
        }

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 6: Loop Settings (Manual p. 36)
        // -------------------------------------------------------------
        let loop_active = project.transport.loop_enabled;
        let loop_start_str = format!(
            "{:2} . 1 . 1",
            (project.transport.loop_start_beats / 4.0) as u32 + 1
        );
        let _loop_start_lcd = live_lcd_box(
            ui,
            &loop_start_str,
            Vec2::new(48.0, 24.0),
            palette.amber,
            false,
        );

        let (loop_rect, loop_resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
        if ui.is_rect_visible(loop_rect) {
            let painter = ui.painter();
            let is_dark = ui.visuals().dark_mode;
            let bg = if loop_active {
                palette.amber
            } else if loop_resp.hovered() {
                palette.button_hover
            } else {
                palette.button_idle
            };
            painter.rect_filled(loop_rect, Rounding::same(2.0), bg);
            painter.rect_stroke(
                loop_rect,
                Rounding::same(2.0),
                Stroke::new(
                    1.0_f32,
                    if loop_active {
                        palette.amber
                    } else if is_dark {
                        Color32::from_rgb(0x38, 0x38, 0x38)
                    } else {
                        Color32::from_rgb(0x9c, 0x9c, 0x9a)
                    },
                ),
            );
            let icon_col = if loop_active {
                Color32::BLACK
            } else {
                palette.control_foreground
            };
            let c = loop_rect.center();
            // Clean painted loop arrows
            painter.line_segment(
                [
                    Pos2::new(c.x - 5.0, c.y - 3.0),
                    Pos2::new(c.x + 4.0, c.y - 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + 2.0, c.y - 5.0),
                    Pos2::new(c.x + 4.0, c.y - 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + 2.0, c.y - 1.0),
                    Pos2::new(c.x + 4.0, c.y - 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + 4.0, c.y + 3.0),
                    Pos2::new(c.x - 5.0, c.y + 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - 3.0, c.y + 1.0),
                    Pos2::new(c.x - 5.0, c.y + 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - 3.0, c.y + 5.0),
                    Pos2::new(c.x - 5.0, c.y + 3.0),
                ],
                Stroke::new(1.5_f32, icon_col),
            );
        }
        if loop_resp.clicked() {
            project.transport.loop_enabled = !project.transport.loop_enabled;
        }

        let loop_len_str = format!(
            "{:2} . 0 . 0",
            (project.transport.loop_length_beats / 4.0) as u32
        );
        let _loop_len_lcd = live_lcd_box(
            ui,
            &loop_len_str,
            Vec2::new(48.0, 24.0),
            palette.amber,
            false,
        );

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 7: Tools, CPU & View Mode Switcher
        // -------------------------------------------------------------
        let _draw_btn = live_toggle_button(ui, false, palette.amber, "Draw", Vec2::new(32.0, 24.0));
        let _kb_btn =
            live_toggle_button(ui, true, palette.amber, "MIDI Key", Vec2::new(48.0, 24.0));
        let _key_map_btn =
            live_toggle_button(ui, false, palette.amber, "Key", Vec2::new(28.0, 24.0));
        let _midi_map_btn =
            live_toggle_button(ui, false, palette.amber, "MIDI", Vec2::new(32.0, 24.0));

        // CPU Load meter well
        let (cpu_rect, _) = ui.allocate_exact_size(Vec2::new(46.0, 24.0), Sense::hover());
        if ui.is_rect_visible(cpu_rect) {
            let painter = ui.painter();
            painter.rect_filled(cpu_rect, Rounding::same(2.0), palette.lcd_bg);
            painter.rect_stroke(
                cpu_rect,
                Rounding::same(2.0),
                Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
            );

            let load_norm = (cpu_percent / 100.0).clamp(0.0, 1.0);
            let bar_w = (cpu_rect.width() - 4.0) * load_norm;
            let bar_rect = Rect::from_min_size(
                Pos2::new(cpu_rect.min.x + 2.0, cpu_rect.min.y + 2.0),
                Vec2::new(bar_w, cpu_rect.height() - 4.0),
            );
            painter.rect_filled(
                bar_rect,
                Rounding::same(1.0),
                Color32::from_rgb(0x28, 0x88, 0x44),
            );

            painter.text(
                cpu_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.1}%", cpu_percent),
                egui::FontId::monospace(9.5),
                palette.control_foreground,
            );
        }

        // View Selectors pushed to right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (sess_click, arr_click) = crate::widgets::live_view_mode_toggle(
                ui,
                current_view == ViewMode::Session,
                palette.amber,
            );
            if sess_click {
                resp.view_mode_changed = Some(ViewMode::Session);
            }
            if arr_click {
                resp.view_mode_changed = Some(ViewMode::Arrangement);
            }
        });
    });

    resp
}
