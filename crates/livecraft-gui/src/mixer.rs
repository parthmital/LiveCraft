use crate::style::LivePalette;
use crate::widgets::{live_fader, live_knob, live_stereo_meter, live_toggle_button};
use eframe::egui::{self, Color32, Pos2, Rounding, Sense, Stroke, Ui, Vec2};
use livecraft_core::model::Project;
use livecraft_dsp::graph::TrackMeter;

pub fn render_mixer_section(
    ui: &mut Ui,
    project: &mut Project,
    track_meters: &[TrackMeter],
    master_meter: &TrackMeter,
    palette: &LivePalette,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        let track_width = 96.0;

        // -------------------------------------------------------------
        // Player Tracks Mixer Strips
        // -------------------------------------------------------------
        for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
            let meter = track_meters.get(trk_idx).cloned().unwrap_or_default();

            ui.vertical(|ui| {
                ui.set_width(track_width);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                // 1. In / Auto / Off Monitoring (Ableton Live signature)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);
                    let _in_btn =
                        live_toggle_button(ui, false, palette.amber, "In", Vec2::new(30.0, 16.0));
                    let _auto_btn =
                        live_toggle_button(ui, true, palette.amber, "Auto", Vec2::new(32.0, 16.0));
                    let _off_btn =
                        live_toggle_button(ui, false, palette.amber, "Off", Vec2::new(30.0, 16.0));
                });

                // 2. Sends (A and B)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                    if let Some(send_a) = trk.sends.first_mut() {
                        live_knob(ui, send_a, 0.0, 1.0, 26.0, "A", palette.amber);
                    }
                    if let Some(send_b) = trk.sends.get_mut(1) {
                        live_knob(ui, send_b, 0.0, 1.0, 26.0, "B", palette.cyan);
                    }
                });

                // 3. Pan knob with center readout
                let pan_label = if trk.pan.abs() < 0.05 {
                    "C".to_string()
                } else if trk.pan < 0.0 {
                    format!("{}L", (-trk.pan * 50.0).round() as i32)
                } else {
                    format!("{}R", (trk.pan * 50.0).round() as i32)
                };
                live_knob(ui, &mut trk.pan, -1.0, 1.0, 26.0, &pan_label, palette.amber);

                // 4. Volume Fader & Stereo VU meter side by side with dB markings
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                    // Fader
                    live_fader(
                        ui,
                        &mut trk.volume_db,
                        -60.0,
                        6.0,
                        Vec2::new(30.0, 105.0),
                        palette.amber,
                    );

                    // Stereo VU Meter
                    live_stereo_meter(
                        ui,
                        meter.peak_db_l(),
                        meter.peak_db_r(),
                        Vec2::new(16.0, 105.0),
                    );

                    // Vertical dB scale ticks
                    let (scale_rect, _) =
                        ui.allocate_exact_size(Vec2::new(20.0, 105.0), Sense::hover());
                    if ui.is_rect_visible(scale_rect) {
                        let painter = ui.painter();
                        let labels = [
                            ("+6", 0.0),
                            ("0", 0.15),
                            ("-6", 0.30),
                            ("-12", 0.45),
                            ("-24", 0.65),
                            ("-inf", 0.95),
                        ];
                        for (txt, norm_y) in labels {
                            let y = scale_rect.min.y + norm_y * scale_rect.height();
                            painter.text(
                                Pos2::new(scale_rect.min.x, y),
                                egui::Align2::LEFT_CENTER,
                                txt,
                                egui::FontId::monospace(8.0),
                                Color32::from_rgb(0x70, 0x70, 0x70),
                            );
                        }
                    }
                });

                // 5. Volume dB numerical display box
                let (val_rect, _) =
                    ui.allocate_exact_size(Vec2::new(track_width - 8.0, 16.0), Sense::click());
                if ui.is_rect_visible(val_rect) {
                    let painter = ui.painter();
                    painter.rect_filled(val_rect, Rounding::same(1.0), palette.lcd_bg);
                    painter.rect_stroke(
                        val_rect,
                        Rounding::same(1.0),
                        Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
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
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                    // Track Activator (Amber number)
                    let active = !trk.is_muted;
                    let (act_rect, act_resp) =
                        ui.allocate_exact_size(Vec2::new(28.0, 20.0), Sense::click());
                    if ui.is_rect_visible(act_rect) {
                        let painter = ui.painter();
                        let bg = if active {
                            palette.amber
                        } else {
                            Color32::from_rgb(0x28, 0x28, 0x28)
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
                                    Color32::from_rgb(0x38, 0x38, 0x38)
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
                        ui.allocate_exact_size(Vec2::new(28.0, 20.0), Sense::click());
                    if ui.is_rect_visible(solo_rect) {
                        let painter = ui.painter();
                        let bg = if solo {
                            palette.solo_blue
                        } else {
                            Color32::from_rgb(0x28, 0x28, 0x28)
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
                                    Color32::from_rgb(0x38, 0x38, 0x38)
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
                        ui.allocate_exact_size(Vec2::new(28.0, 20.0), Sense::click());
                    if ui.is_rect_visible(arm_rect) {
                        let painter = ui.painter();
                        let bg = if armed {
                            palette.record_red
                        } else {
                            Color32::from_rgb(0x28, 0x28, 0x28)
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
                                    Color32::from_rgb(0x38, 0x38, 0x38)
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
            });
        }

        // -------------------------------------------------------------
        // Return Tracks Strips (A and B)
        // -------------------------------------------------------------
        for ret in project.return_tracks.iter_mut() {
            ui.vertical(|ui| {
                ui.set_width(track_width * 0.75);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                // Space for In/Auto/Off
                ui.allocate_exact_size(Vec2::new(track_width * 0.75, 16.0), Sense::hover());

                // Pan knob
                live_knob(ui, &mut ret.pan, -1.0, 1.0, 26.0, "Pan", palette.amber);

                // Volume fader
                live_fader(
                    ui,
                    &mut ret.volume_db,
                    -60.0,
                    6.0,
                    Vec2::new(26.0, 105.0),
                    palette.amber,
                );

                // dB readout
                ui.label(
                    egui::RichText::new(format!("{:.1} dB", ret.volume_db))
                        .size(9.0)
                        .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                );

                // Activator & Solo
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                    let active = !ret.is_muted;
                    let act_btn = live_toggle_button(
                        ui,
                        active,
                        palette.amber,
                        &ret.name[..1],
                        Vec2::new(24.0, 20.0),
                    );
                    if act_btn.clicked() {
                        ret.is_muted = !ret.is_muted;
                    }
                    let solo_btn = live_toggle_button(
                        ui,
                        ret.is_soloed,
                        palette.solo_blue,
                        "S",
                        Vec2::new(24.0, 20.0),
                    );
                    if solo_btn.clicked() {
                        ret.is_soloed = !ret.is_soloed;
                    }
                });
            });
        }

        // -------------------------------------------------------------
        // Master Track Mixer Strip
        // -------------------------------------------------------------
        ui.vertical(|ui| {
            ui.set_width(72.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

            // Master Pan
            live_knob(
                ui,
                &mut project.master_track.pan,
                -1.0,
                1.0,
                26.0,
                "C",
                palette.amber,
            );

            // Master Volume Fader & VU Meter
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                live_fader(
                    ui,
                    &mut project.master_track.volume_db,
                    -60.0,
                    6.0,
                    Vec2::new(28.0, 105.0),
                    palette.amber,
                );
                live_stereo_meter(
                    ui,
                    master_meter.peak_db_l(),
                    master_meter.peak_db_r(),
                    Vec2::new(16.0, 105.0),
                );
            });

            // Master Volume dB box
            let (mval_rect, _) = ui.allocate_exact_size(Vec2::new(68.0, 16.0), Sense::click());
            if ui.is_rect_visible(mval_rect) {
                let painter = ui.painter();
                painter.rect_filled(mval_rect, Rounding::same(1.0), palette.lcd_bg);
                painter.rect_stroke(
                    mval_rect,
                    Rounding::same(1.0),
                    Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
                );
                painter.text(
                    mval_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.1} dB", project.master_track.volume_db),
                    egui::FontId::monospace(9.5),
                    palette.lcd_text,
                );
            }

            // Master Cue / Solo Volume knob
            ui.label(
                egui::RichText::new("Main Out")
                    .size(9.0)
                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
            );
        });
    });
}
