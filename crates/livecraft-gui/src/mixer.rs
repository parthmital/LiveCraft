use crate::style::LivePalette;
use crate::widgets::{
    db_to_norm, live_fader, live_knob, live_stereo_meter, live_toggle_button, norm_to_db,
};
use eframe::egui::{self, Color32, Pos2, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::{MonitoringMode, Track};
use livecraft_core::theme::parse_hex_color;
use livecraft_dsp::graph::TrackMeter;

/// Renders the mixer channel strip for a standard Audio or MIDI track.
pub fn render_track_mixer_strip(
    ui: &mut Ui,
    trk: &mut Track,
    trk_idx: usize,
    meter: &TrackMeter,
    track_width: f32,
    palette: &LivePalette,
) {
    ui.vertical(|ui| {
        ui.set_width(track_width);
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);
        let is_dark = palette.is_dark();

        // 1. In / Auto / Off Monitoring (Ableton Live signature)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            let btn_w = ((track_width - 4.0) / 3.0).floor();
            let in_active = trk.monitoring == MonitoringMode::In;
            let auto_active = trk.monitoring == MonitoringMode::Auto;
            let off_active = trk.monitoring == MonitoringMode::Off;

            let in_btn =
                live_toggle_button(ui, in_active, palette.amber, "In", Vec2::new(btn_w, 16.0));
            if in_btn.clicked() {
                trk.monitoring = MonitoringMode::In;
            }

            let auto_btn = live_toggle_button(
                ui,
                auto_active,
                palette.amber,
                "Auto",
                Vec2::new(btn_w, 16.0),
            );
            if auto_btn.clicked() {
                trk.monitoring = MonitoringMode::Auto;
            }

            let off_btn =
                live_toggle_button(ui, off_active, palette.amber, "Off", Vec2::new(btn_w, 16.0));
            if off_btn.clicked() {
                trk.monitoring = MonitoringMode::Off;
            }
        });

        // 2. Sends A and B
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            let knob_size = 24.0;
            if let Some(send_a) = trk.sends.first_mut() {
                live_knob(ui, send_a, 0.0, 1.0, knob_size, "A", palette.amber);
            }
            if let Some(send_b) = trk.sends.get_mut(1) {
                live_knob(ui, send_b, 0.0, 1.0, knob_size, "B", palette.cyan);
            }
        });

        // 3. Pan knob with center / L / R readout
        let pan_label = if trk.pan.abs() < 0.05 {
            "C".to_string()
        } else if trk.pan < 0.0 {
            format!("{}L", (-trk.pan * 50.0).round() as i32)
        } else {
            format!("{}R", (trk.pan * 50.0).round() as i32)
        };
        live_knob(ui, &mut trk.pan, -1.0, 1.0, 24.0, &pan_label, palette.amber);

        // 4. Volume Fader & Stereo VU meter side by side with dB markings
        let fader_h = 100.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

            // Fader
            live_fader(
                ui,
                &mut trk.volume_db,
                -60.0,
                6.0,
                Vec2::new(28.0, fader_h),
                palette.amber,
            );

            // Stereo VU Meter
            live_stereo_meter(
                ui,
                meter.peak_db_l(),
                meter.peak_db_r(),
                Vec2::new(14.0, fader_h),
            );

            // Vertical dB scale ticks perfectly aligned with db_to_norm
            let (scale_rect, _) = ui.allocate_exact_size(Vec2::new(24.0, fader_h), Sense::hover());
            if ui.is_rect_visible(scale_rect) {
                let painter = ui.painter();
                let labels: [(&str, f32); 6] = [
                    ("+6", 6.0),
                    ("0", 0.0),
                    ("-6", -6.0),
                    ("-12", -12.0),
                    ("-24", -24.0),
                    ("-inf", -60.0),
                ];
                let text_col = if is_dark {
                    Color32::from_rgb(0x80, 0x80, 0x80)
                } else {
                    Color32::from_rgb(0x40, 0x40, 0x40)
                };
                for (txt, db_val) in labels {
                    let norm = db_to_norm(db_val);
                    let y = scale_rect.max.y - 7.0 - norm * (scale_rect.height() - 14.0);
                    painter.text(
                        Pos2::new(scale_rect.min.x + 2.0, y),
                        egui::Align2::LEFT_CENTER,
                        txt,
                        egui::FontId::monospace(8.0),
                        text_col,
                    );
                }
            }
        });

        // 5. Volume dB numerical display box (interactive click & drag, double-click resets to 0.0)
        let (val_rect, val_resp) =
            ui.allocate_exact_size(Vec2::new(track_width - 6.0, 16.0), Sense::click_and_drag());
        if val_resp.dragged() {
            let current_norm = db_to_norm(trk.volume_db);
            let delta_norm = -val_resp.drag_delta().y / 100.0;
            trk.volume_db = norm_to_db((current_norm + delta_norm).clamp(0.0, 1.0));
        }
        if val_resp.double_clicked() {
            trk.volume_db = 0.0;
        }

        if ui.is_rect_visible(val_rect) {
            let painter = ui.painter();
            painter.rect_filled(val_rect, Rounding::same(1.0), palette.lcd_bg);
            let border_color = if val_resp.hovered() {
                palette.amber
            } else {
                Color32::from_rgb(0x35, 0x35, 0x35)
            };
            painter.rect_stroke(
                val_rect,
                Rounding::same(1.0),
                Stroke::new(1.0_f32, border_color),
            );

            let vol_txt = if trk.volume_db <= -59.5 {
                "-inf dB".to_string()
            } else {
                format!("{:.1} dB", trk.volume_db)
            };
            painter.text(
                val_rect.center(),
                egui::Align2::CENTER_CENTER,
                vol_txt,
                egui::FontId::monospace(9.5),
                palette.lcd_text,
            );
        }

        // 6. Action buttons row: Track Activator (Mute), Solo (S), Arm (●)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            let btn_w = ((track_width - 12.0) / 3.0).floor();

            // Track Activator (Amber number)
            let active = !trk.is_muted;
            let (act_rect, act_resp) =
                ui.allocate_exact_size(Vec2::new(btn_w, 20.0), Sense::click());
            if ui.is_rect_visible(act_rect) {
                let painter = ui.painter();
                let bg = if active {
                    palette.amber
                } else if act_resp.hovered() {
                    palette.button_hover
                } else {
                    palette.button_idle
                };
                painter.rect_filled(act_rect, Rounding::same(2.0), bg);
                painter.rect_stroke(
                    act_rect,
                    Rounding::same(2.0),
                    Stroke::new(
                        1.0_f32,
                        if active {
                            palette.amber
                        } else {
                            palette.panel_border
                        },
                    ),
                );
                let num_color = if active {
                    Color32::BLACK
                } else {
                    palette.control_disabled
                };
                painter.text(
                    act_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", trk_idx + 1),
                    egui::FontId::proportional(11.0),
                    num_color,
                );
            }
            if act_resp.clicked() {
                trk.is_muted = !trk.is_muted;
            }

            // Solo button (S)
            let solo = trk.is_soloed;
            let (solo_rect, solo_resp) =
                ui.allocate_exact_size(Vec2::new(btn_w, 20.0), Sense::click());
            if ui.is_rect_visible(solo_rect) {
                let painter = ui.painter();
                let bg = if solo {
                    palette.solo_blue
                } else if solo_resp.hovered() {
                    palette.button_hover
                } else {
                    palette.button_idle
                };
                painter.rect_filled(solo_rect, Rounding::same(2.0), bg);
                painter.rect_stroke(
                    solo_rect,
                    Rounding::same(2.0),
                    Stroke::new(
                        1.0_f32,
                        if solo {
                            palette.solo_blue
                        } else {
                            palette.panel_border
                        },
                    ),
                );
                let s_color = if solo {
                    Color32::WHITE
                } else {
                    palette.control_disabled
                };
                painter.text(
                    solo_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "S",
                    egui::FontId::proportional(11.0),
                    s_color,
                );
            }
            if solo_resp.clicked() {
                trk.is_soloed = !trk.is_soloed;
            }

            // Arm button (●)
            let armed = trk.is_armed;
            let (arm_rect, arm_resp) =
                ui.allocate_exact_size(Vec2::new(btn_w, 20.0), Sense::click());
            if ui.is_rect_visible(arm_rect) {
                let painter = ui.painter();
                let bg = if armed {
                    palette.record_red
                } else if arm_resp.hovered() {
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
                        if armed {
                            palette.record_red
                        } else {
                            palette.panel_border
                        },
                    ),
                );
                let arm_color = if armed {
                    Color32::WHITE
                } else {
                    palette.control_disabled
                };
                painter.circle_filled(arm_rect.center(), 4.5, arm_color);
            }
            if arm_resp.clicked() {
                trk.is_armed = !trk.is_armed;
            }
        });

        // 7. Bottom color accent stripe
        let (stripe_rect, _) =
            ui.allocate_exact_size(Vec2::new(track_width - 4.0, 4.0), Sense::hover());
        if ui.is_rect_visible(stripe_rect) {
            let [r, g, b, _] = parse_hex_color(&trk.color);
            ui.painter()
                .rect_filled(stripe_rect, Rounding::same(1.0), Color32::from_rgb(r, g, b));
        }
    });
}

/// Renders the mixer channel strip for a Return Track (A-Reverb, B-Delay).
pub fn render_return_mixer_strip(
    ui: &mut Ui,
    ret: &mut Track,
    ret_idx: usize,
    track_width: f32,
    palette: &LivePalette,
) {
    ui.vertical(|ui| {
        ui.set_width(track_width);
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);
        let is_dark = palette.is_dark();

        // Blank space matching the In/Auto/Off monitoring row (16.0)
        ui.allocate_exact_size(Vec2::new(track_width, 16.0), Sense::hover());

        // Blank space matching the Sends row (live_knob size 24.0 is 37.0 tall)
        ui.allocate_exact_size(Vec2::new(track_width, 37.0), Sense::hover());

        // Pan knob
        let pan_label = if ret.pan.abs() < 0.05 {
            "C".to_string()
        } else if ret.pan < 0.0 {
            format!("{}L", (-ret.pan * 50.0).round() as i32)
        } else {
            format!("{}R", (ret.pan * 50.0).round() as i32)
        };
        live_knob(ui, &mut ret.pan, -1.0, 1.0, 24.0, &pan_label, palette.amber);

        // Volume fader & dB scale ticks
        let fader_h = 100.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            live_fader(
                ui,
                &mut ret.volume_db,
                -60.0,
                6.0,
                Vec2::new(28.0, fader_h),
                palette.amber,
            );

            // Meter quiet
            live_stereo_meter(ui, -60.0, -60.0, Vec2::new(14.0, fader_h));

            // dB scale ticks
            let (scale_rect, _) = ui.allocate_exact_size(Vec2::new(20.0, fader_h), Sense::hover());
            if ui.is_rect_visible(scale_rect) {
                let painter = ui.painter();
                let labels: [(&str, f32); 4] =
                    [("+6", 6.0), ("0", 0.0), ("-12", -12.0), ("-inf", -60.0)];
                let text_col = if is_dark {
                    Color32::from_rgb(0x80, 0x80, 0x80)
                } else {
                    Color32::from_rgb(0x40, 0x40, 0x40)
                };
                for (txt, db_val) in labels {
                    let norm = db_to_norm(db_val);
                    let y = scale_rect.max.y - 7.0 - norm * (scale_rect.height() - 14.0);
                    painter.text(
                        Pos2::new(scale_rect.min.x + 2.0, y),
                        egui::Align2::LEFT_CENTER,
                        txt,
                        egui::FontId::monospace(8.0),
                        text_col,
                    );
                }
            }
        });

        // dB readout (interactive click & drag)
        let (val_rect, val_resp) =
            ui.allocate_exact_size(Vec2::new(track_width - 6.0, 16.0), Sense::click_and_drag());
        if val_resp.dragged() {
            let current_norm = db_to_norm(ret.volume_db);
            let delta_norm = -val_resp.drag_delta().y / 100.0;
            ret.volume_db = norm_to_db((current_norm + delta_norm).clamp(0.0, 1.0));
        }
        if val_resp.double_clicked() {
            ret.volume_db = 0.0;
        }

        if ui.is_rect_visible(val_rect) {
            let painter = ui.painter();
            painter.rect_filled(val_rect, Rounding::same(1.0), palette.lcd_bg);
            let border_color = if val_resp.hovered() {
                palette.amber
            } else {
                Color32::from_rgb(0x35, 0x35, 0x35)
            };
            painter.rect_stroke(
                val_rect,
                Rounding::same(1.0),
                Stroke::new(1.0_f32, border_color),
            );

            let vol_txt = if ret.volume_db <= -59.5 {
                "-inf dB".to_string()
            } else {
                format!("{:.1} dB", ret.volume_db)
            };
            painter.text(
                val_rect.center(),
                egui::Align2::CENTER_CENTER,
                vol_txt,
                egui::FontId::monospace(9.5),
                palette.lcd_text,
            );
        }

        // Activator & Solo row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            let btn_w = ((track_width - 8.0) / 2.0).floor();
            let letter = if ret_idx == 0 { "A" } else { "B" };

            let active = !ret.is_muted;
            let act_btn =
                live_toggle_button(ui, active, palette.amber, letter, Vec2::new(btn_w, 20.0));
            if act_btn.clicked() {
                ret.is_muted = !ret.is_muted;
            }

            let solo_btn = live_toggle_button(
                ui,
                ret.is_soloed,
                palette.solo_blue,
                "S",
                Vec2::new(btn_w, 20.0),
            );
            if solo_btn.clicked() {
                ret.is_soloed = !ret.is_soloed;
            }
        });

        // Bottom color accent stripe
        let (stripe_rect, _) =
            ui.allocate_exact_size(Vec2::new(track_width - 4.0, 4.0), Sense::hover());
        if ui.is_rect_visible(stripe_rect) {
            let [r, g, b, _] = parse_hex_color(&ret.color);
            ui.painter()
                .rect_filled(stripe_rect, Rounding::same(1.0), Color32::from_rgb(r, g, b));
        }
    });
}

/// Renders the Master / Main track mixer strip.
pub fn render_master_mixer_strip(
    ui: &mut Ui,
    master: &mut Track,
    master_meter: &TrackMeter,
    track_width: f32,
    palette: &LivePalette,
) {
    ui.vertical(|ui| {
        ui.set_width(track_width);
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);
        let is_dark = palette.is_dark();

        // Space matching In/Auto/Off
        ui.allocate_exact_size(Vec2::new(track_width, 16.0), Sense::hover());

        // Master Cue knob
        let mut cue_dummy = 0.8_f32;
        live_knob(ui, &mut cue_dummy, 0.0, 1.0, 24.0, "Cue", palette.cyan);

        // Master Pan
        let pan_label = if master.pan.abs() < 0.05 {
            "C".to_string()
        } else if master.pan < 0.0 {
            format!("{}L", (-master.pan * 50.0).round() as i32)
        } else {
            format!("{}R", (master.pan * 50.0).round() as i32)
        };
        live_knob(
            ui,
            &mut master.pan,
            -1.0,
            1.0,
            24.0,
            &pan_label,
            palette.amber,
        );

        // Master Volume Fader & Stereo Master Meters
        let fader_h = 100.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            live_fader(
                ui,
                &mut master.volume_db,
                -60.0,
                6.0,
                Vec2::new(28.0, fader_h),
                palette.amber,
            );
            live_stereo_meter(
                ui,
                master_meter.peak_db_l(),
                master_meter.peak_db_r(),
                Vec2::new(16.0, fader_h),
            );

            // dB scale ticks aligned with db_to_norm
            let (scale_rect, _) = ui.allocate_exact_size(Vec2::new(20.0, fader_h), Sense::hover());
            if ui.is_rect_visible(scale_rect) {
                let painter = ui.painter();
                let labels: [(&str, f32); 6] = [
                    ("+6", 6.0),
                    ("0", 0.0),
                    ("-6", -6.0),
                    ("-12", -12.0),
                    ("-24", -24.0),
                    ("-inf", -60.0),
                ];
                let text_col = if is_dark {
                    Color32::from_rgb(0x80, 0x80, 0x80)
                } else {
                    Color32::from_rgb(0x40, 0x40, 0x40)
                };
                for (txt, db_val) in labels {
                    let norm = db_to_norm(db_val);
                    let y = scale_rect.max.y - 7.0 - norm * (scale_rect.height() - 14.0);
                    painter.text(
                        Pos2::new(scale_rect.min.x + 2.0, y),
                        egui::Align2::LEFT_CENTER,
                        txt,
                        egui::FontId::monospace(8.0),
                        text_col,
                    );
                }
            }
        });

        // Master Volume dB numerical box (interactive click & drag)
        let (val_rect, val_resp) =
            ui.allocate_exact_size(Vec2::new(track_width - 6.0, 16.0), Sense::click_and_drag());
        if val_resp.dragged() {
            let current_norm = db_to_norm(master.volume_db);
            let delta_norm = -val_resp.drag_delta().y / 100.0;
            master.volume_db = norm_to_db((current_norm + delta_norm).clamp(0.0, 1.0));
        }
        if val_resp.double_clicked() {
            master.volume_db = 0.0;
        }

        if ui.is_rect_visible(val_rect) {
            let painter = ui.painter();
            painter.rect_filled(val_rect, Rounding::same(1.0), palette.lcd_bg);
            let border_color = if val_resp.hovered() {
                palette.amber
            } else {
                Color32::from_rgb(0x35, 0x35, 0x35)
            };
            painter.rect_stroke(
                val_rect,
                Rounding::same(1.0),
                Stroke::new(1.0_f32, border_color),
            );

            let vol_txt = if master.volume_db <= -59.5 {
                "-inf dB".to_string()
            } else {
                format!("{:.1} dB", master.volume_db)
            };
            painter.text(
                val_rect.center(),
                egui::Align2::CENTER_CENTER,
                vol_txt,
                egui::FontId::monospace(9.5),
                palette.lcd_text,
            );
        }

        // Crossfader assign / Master Cue selector row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            let solo_btn = live_toggle_button(
                ui,
                false,
                palette.solo_blue,
                "Solo",
                Vec2::new(track_width - 6.0, 20.0),
            );
            if solo_btn.clicked() {
                // Toggle Master solo defeat / cue
            }
        });

        // Bottom master stripe
        let (stripe_rect, _) =
            ui.allocate_exact_size(Vec2::new(track_width - 4.0, 4.0), Sense::hover());
        if ui.is_rect_visible(stripe_rect) {
            ui.painter().rect_filled(
                stripe_rect,
                Rounding::same(1.0),
                Color32::from_rgb(0x80, 0xe0, 0x90),
            );
        }
    });
}
