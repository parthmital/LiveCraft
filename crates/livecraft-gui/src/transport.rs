use crate::style::LivePalette;
use crate::widgets::{live_divider, live_lcd_box, live_toggle_button};
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::Project;

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
        let browser_btn = live_toggle_button(
            ui,
            browser_visible,
            palette.amber,
            if browser_visible { "◀" } else { "▶" },
            Vec2::new(20.0, 24.0),
        );
        if browser_btn.clicked() {
            resp.browser_toggled = true;
        }

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 2: Tempo Settings and Metronome (Manual p. 36)
        // -------------------------------------------------------------
        // Link toggle
        let link_active = false;
        let _link_btn = live_toggle_button(
            ui,
            link_active,
            palette.amber,
            "Link",
            Vec2::new(32.0, 24.0),
        );

        // Tap tempo
        let tap_btn = live_toggle_button(ui, false, palette.amber, "TAP", Vec2::new(30.0, 24.0));
        if tap_btn.clicked() {
            // TAP tempo
        }

        // Tempo BPM LCD well with up/down nudge
        let mut bpm = project.transport.bpm;
        let bpm_str = format!("{:.2}", bpm);
        let bpm_lcd = live_lcd_box(
            ui,
            &format!("{} BPM", bpm_str),
            Vec2::new(66.0, 24.0),
            palette.amber,
            false,
        );
        if bpm_lcd.dragged() {
            bpm = (bpm - bpm_lcd.drag_delta().y * 0.2).clamp(20.0, 999.0);
            project.transport.bpm = bpm;
        }

        // Time Signature LCD well
        let time_sig_str = format!(
            "{}/{}",
            project.transport.time_signature.0, project.transport.time_signature.1
        );
        let _sig_lcd = live_lcd_box(
            ui,
            &time_sig_str,
            Vec2::new(32.0, 24.0),
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
            live_toggle_button(ui, false, palette.amber, "1 Bar ▼", Vec2::new(44.0, 24.0));

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 3: Scale Settings (Live 12 New Feature! Manual p. 36)
        // -------------------------------------------------------------
        let scale_active = project.scale.is_active;
        let scale_btn = live_toggle_button(
            ui,
            scale_active,
            palette.amber,
            "Scale",
            Vec2::new(36.0, 24.0),
        );
        if scale_btn.clicked() {
            project.scale.is_active = !project.scale.is_active;
        }

        if project.scale.is_active {
            let root_idx = (project.scale.root_note % 12) as usize;
            let root_str = livecraft_core::model::NOTE_NAMES[root_idx];
            let _root_btn = live_lcd_box(ui, root_str, Vec2::new(26.0, 24.0), palette.amber, true);

            let mode_str = format!("{:?}", project.scale.scale_type);
            let _mode_btn =
                live_lcd_box(ui, &mode_str, Vec2::new(50.0, 24.0), palette.amber, false);
        }

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 4: Follow and Arrangement Position (Manual p. 36)
        // -------------------------------------------------------------
        let follow_btn =
            live_toggle_button(ui, true, palette.amber, "Follow", Vec2::new(42.0, 24.0));
        if follow_btn.clicked() {
            // Follow toggle
        }

        // Arrangement Position LCD (Bar . Beat . 16th)
        let beats = project.transport.playhead_beats;
        let bar = (beats / 4.0) as u32 + 1;
        let beat = (beats % 4.0) as u32 + 1;
        let sixteenth = ((beats * 4.0) % 4.0) as u32 + 1;
        let pos_str = format!("{:2} . {:1} . {:1}", bar, beat, sixteenth);
        let pos_lcd = live_lcd_box(
            ui,
            &pos_str,
            Vec2::new(68.0, 24.0),
            palette.amber,
            project.transport.is_playing,
        );
        if pos_lcd.dragged() {
            let delta = -pos_lcd.drag_delta().y * 0.25;
            project.transport.playhead_beats =
                (project.transport.playhead_beats + delta as f64).max(0.0);
        }

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 5: Transport Controls (Play, Stop, Record) (Manual p. 36)
        // -------------------------------------------------------------
        let is_playing = project.transport.is_playing;
        let (play_rect, play_resp) = ui.allocate_exact_size(Vec2::new(32.0, 24.0), Sense::click());
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

            // Crisp right-pointing solid triangle
            let c = play_rect.center();
            let arrow_color = if is_playing {
                palette.play_green
            } else {
                Color32::from_rgb(0xaa, 0xaa, 0xaa)
            };
            let p1 = Pos2::new(c.x - 4.0, c.y - 6.0);
            let p2 = Pos2::new(c.x - 4.0, c.y + 6.0);
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
            let sq = Rect::from_center_size(c, Vec2::new(9.0, 9.0));
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

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 6: Automation & Capture MIDI (Manual p. 36)
        // -------------------------------------------------------------
        let _overdub_btn = live_toggle_button(ui, false, palette.amber, "+", Vec2::new(20.0, 24.0));
        let _auto_arm_btn =
            live_toggle_button(ui, true, palette.amber, "⚡", Vec2::new(20.0, 24.0));
        let _capture_btn =
            live_toggle_button(ui, false, palette.amber, "Capture", Vec2::new(48.0, 24.0));
        let _sess_rec_btn = live_toggle_button(
            ui,
            false,
            palette.record_red,
            "[ • ]",
            Vec2::new(28.0, 24.0),
        );

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 7: Arrangement Loop Settings (Manual p. 36)
        // -------------------------------------------------------------
        let loop_active = project.transport.loop_enabled;
        let _punch_in = live_toggle_button(ui, false, palette.amber, ">|", Vec2::new(20.0, 24.0));

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

        let loop_btn =
            live_toggle_button(ui, loop_active, palette.amber, "⇄", Vec2::new(24.0, 24.0));
        if loop_btn.clicked() {
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

        let _punch_out = live_toggle_button(ui, false, palette.amber, "|<", Vec2::new(20.0, 24.0));

        live_divider(ui, 24.0);

        // -------------------------------------------------------------
        // SECTION 8: MIDI and CPU Settings (Manual p. 36)
        // -------------------------------------------------------------
        let _draw_btn = live_toggle_button(ui, false, palette.amber, "✎", Vec2::new(22.0, 24.0));
        let _kb_btn = live_toggle_button(ui, true, palette.amber, "⌨", Vec2::new(22.0, 24.0));
        let _key_map_btn =
            live_toggle_button(ui, false, palette.amber, "KEY", Vec2::new(28.0, 24.0));
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

            // CPU load bar
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

        // Push View Selectors to far right (Section 9)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // -------------------------------------------------------------
            // SECTION 9: View Selector (Manual p. 36)
            // -------------------------------------------------------------
            let is_arr = current_view == ViewMode::Arrangement;
            let is_sess = current_view == ViewMode::Session;

            let arr_btn = live_toggle_button(ui, is_arr, palette.amber, "☰", Vec2::new(26.0, 24.0));
            if arr_btn.clicked() {
                resp.view_mode_changed = Some(ViewMode::Arrangement);
            }

            let sess_btn =
                live_toggle_button(ui, is_sess, palette.amber, "|||", Vec2::new(26.0, 24.0));
            if sess_btn.clicked() {
                resp.view_mode_changed = Some(ViewMode::Session);
            }
        });
    });

    resp
}
