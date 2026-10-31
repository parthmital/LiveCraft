use crate::Edits;
use crate::style::LivePalette;
use crate::widgets::live_knob;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Stroke, Ui, Vec2};
use livecraft_model::ids::TrackId;
use livecraft_model::model::{Device, DeviceType, Track};
use livecraft_session::commands::device::{RemoveDevice, SetDeviceEnabled, SetDeviceParam};

pub fn render_device_chain(
    ui: &mut Ui,
    selected_track: &Track,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

        if selected_track.device_chain.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(
                    egui::RichText::new("Drop Audio Effects or Instruments here from the Browser")
                        .size(12.0)
                        .color(Color32::from_rgb(0x77, 0x77, 0x77)),
                );
            });
            return;
        }

        let mut remove_idx = None;

        for (dev_idx, dev) in selected_track.device_chain.iter().enumerate() {
            ui.vertical(|ui| {
                let box_width = match dev.device_type {
                    DeviceType::EqEight => 280.0,
                    DeviceType::Operator => 350.0,
                    DeviceType::Saturator => 220.0,
                    DeviceType::DrumRack => 280.0,
                    DeviceType::AutoFilter => 230.0,
                    DeviceType::Chorus => 200.0,
                    DeviceType::Delay => 210.0,
                    DeviceType::Reverb => 210.0,
                    DeviceType::Compressor => 210.0,
                    _ => 180.0,
                };
                ui.set_width(box_width);

                // Device Box Header
                ui.horizontal(|ui| {
                    // Power on/off switch
                    let power_color = if dev.is_enabled {
                        palette.amber
                    } else {
                        palette.control_disabled
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("●").size(10.0).color(power_color),
                            )
                            .fill(palette.control_background)
                            .corner_radius(CornerRadius::same(2)),
                        )
                        .clicked()
                    {
                        edits.push(Box::new(SetDeviceEnabled::new(
                            selected_track.id,
                            dev.id,
                            !dev.is_enabled,
                        )));
                    }

                    // Device Name
                    ui.label(
                        egui::RichText::new(&dev.name)
                            .strong()
                            .size(11.0)
                            .color(palette.control_foreground),
                    );

                    // Close/Remove button on right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("×")
                                        .size(11.0)
                                        .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                                )
                                .fill(palette.control_background)
                                .corner_radius(CornerRadius::same(2)),
                            )
                            .clicked()
                        {
                            remove_idx = Some(dev_idx);
                        }
                    });
                });

                // Device Box Body
                let (body_rect, _) =
                    ui.allocate_exact_size(Vec2::new(box_width, 120.0), egui::Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(body_rect, CornerRadius::same(3), palette.control_background);
                painter.rect_stroke(
                    body_rect,
                    CornerRadius::same(3),
                    Stroke::new(1.0_f32, palette.panel_border),
                    eframe::egui::StrokeKind::Middle,
                );

                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(body_rect.shrink(4.0)),
                    |child_ui| match dev.device_type {
                        DeviceType::EqEight => {
                            render_eq_eight_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Compressor => {
                            render_compressor_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Delay => {
                            render_delay_controls(child_ui, selected_track.id, dev, palette, edits);
                        }
                        DeviceType::Reverb => {
                            render_reverb_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Operator => {
                            render_operator_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Utility => {
                            render_utility_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Saturator => {
                            render_saturator_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::AutoFilter => {
                            render_autofilter_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Chorus => {
                            render_chorus_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::DrumRack => {
                            render_drumrack_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::AutoPan => {
                            render_autopan_controls(
                                child_ui,
                                selected_track.id,
                                dev,
                                palette,
                                edits,
                            );
                        }
                        DeviceType::Gate => {
                            render_gate_controls(child_ui, selected_track.id, dev, palette, edits);
                        }
                        DeviceType::EqThree => {
                            render_eq3_controls(child_ui, selected_track.id, dev, palette, edits);
                        }
                        DeviceType::Redux => {
                            render_redux_controls(child_ui, selected_track.id, dev, palette, edits);
                        }
                        _ => {
                            child_ui.label(format!("Device: {}", dev.name));
                        }
                    },
                );
            });
        }
        // Drop Audio Effects Here placeholder
        let drop_w = 130.0;
        let (drop_rect, _) = ui.allocate_exact_size(Vec2::new(drop_w, 142.0), egui::Sense::hover());
        if ui.is_rect_visible(drop_rect) {
            let painter = ui.painter();
            painter.rect_filled(
                drop_rect,
                CornerRadius::same(3),
                Color32::from_rgb(0x1e, 0x1e, 0x1e),
            );
            painter.rect_stroke(
                drop_rect,
                CornerRadius::same(3),
                Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
                eframe::egui::StrokeKind::Middle,
            );
            painter.text(
                drop_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop Audio Effects\n       Here",
                egui::FontId::proportional(10.5),
                palette.control_disabled,
            );
        }

        if let Some(idx) = remove_idx
            && let Some(device) = selected_track.device_chain.get(idx)
        {
            edits.push(Box::new(RemoveDevice::new(selected_track.id, device.id)));
        }
    });
}

fn render_eq_eight_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        // Mini EQ curve visualizer
        let (curve_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(
            curve_rect,
            CornerRadius::same(2),
            Color32::from_rgb(0x10, 0x10, 0x10),
        );
        painter.rect_stroke(
            curve_rect,
            CornerRadius::same(2),
            Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
            eframe::egui::StrokeKind::Middle,
        );

        // Center 0 dB line
        let center_y = curve_rect.center().y;
        painter.line_segment(
            [
                Pos2::new(curve_rect.min.x, center_y),
                Pos2::new(curve_rect.max.x, center_y),
            ],
            Stroke::new(0.5_f32, Color32::from_rgb(0x28, 0x28, 0x28)),
        );

        // Draw EQ curve points
        let mut prev_pt = Pos2::new(curve_rect.min.x, center_y);
        for x_step in 0..=30 {
            let norm_x = x_step as f32 / 30.0;
            let screen_x = curve_rect.min.x + norm_x * curve_rect.width();

            // Approximate curve from bands
            let mut total_gain = 0.0;
            for i in 1..=4 {
                let gain = dev.get_param(&format!("band_{}_gain", i));
                let band_norm_x = (i as f32 - 0.5) / 4.0;
                let dist = (norm_x - band_norm_x).abs();
                total_gain += gain * (-dist * 6.0).exp();
            }

            let screen_y = center_y - (total_gain / 15.0) * (curve_rect.height() * 0.45);
            let pt = Pos2::new(
                screen_x,
                screen_y.clamp(curve_rect.min.y + 2.0, curve_rect.max.y - 2.0),
            );
            painter.line_segment([prev_pt, pt], Stroke::new(1.5_f32, palette.amber));
            prev_pt = pt;
        }

        ui.add_space(4.0);

        // 4 Main Bands Knobs
        ui.horizontal(|ui| {
            for b in 1..=4 {
                let mut gain = dev.get_param(&format!("band_{}_gain", b));
                if live_knob(
                    ui,
                    &mut gain,
                    -15.0,
                    15.0,
                    24.0,
                    &format!("B{}", b),
                    palette.amber,
                )
                .changed()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        format!("band_{}_gain", b),
                        gain,
                    )));
                }
            }
        });
    });
}

fn render_compressor_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        let mut thresh = dev.get_param("threshold_db");
        if live_knob(ui, &mut thresh, -60.0, 0.0, 26.0, "Thresh", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "threshold_db",
                thresh,
            )));
        }

        let mut ratio = dev.get_param("ratio");
        if live_knob(ui, &mut ratio, 1.0, 20.0, 26.0, "Ratio", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "ratio", ratio)));
        }

        let mut attack = dev.get_param("attack_ms");
        if live_knob(ui, &mut attack, 0.1, 100.0, 26.0, "Attack", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "attack_ms",
                attack,
            )));
        }

        let mut makeup = dev.get_param("makeup_gain_db");
        if live_knob(ui, &mut makeup, 0.0, 24.0, 26.0, "Gain", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "makeup_gain_db",
                makeup,
            )));
        }
    });
}

fn render_delay_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        let mut time = dev.get_param("time_ms");
        if live_knob(ui, &mut time, 10.0, 1000.0, 26.0, "Time", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track, dev.id, "time_ms", time,
            )));
        }

        let mut fb = dev.get_param("feedback");
        if live_knob(ui, &mut fb, 0.0, 0.95, 26.0, "Feedback", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "feedback", fb)));
        }

        let mut dry_wet = dev.get_param("dry_wet");
        if live_knob(ui, &mut dry_wet, 0.0, 1.0, 26.0, "Dry/Wet", palette.cyan).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track, dev.id, "dry_wet", dry_wet,
            )));
        }
    });
}

fn render_reverb_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        let mut decay = dev.get_param("decay_time_sec");
        if live_knob(ui, &mut decay, 0.2, 10.0, 26.0, "Decay", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "decay_time_sec",
                decay,
            )));
        }

        let mut size = dev.get_param("room_size");
        if live_knob(ui, &mut size, 0.0, 1.0, 26.0, "Size", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "room_size",
                size,
            )));
        }

        let mut dry_wet = dev.get_param("dry_wet");
        if live_knob(ui, &mut dry_wet, 0.0, 1.0, 26.0, "Dry/Wet", palette.cyan).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track, dev.id, "dry_wet", dry_wet,
            )));
        }
    });
}

fn render_operator_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

        // Sub-panel 1: Oscillators (A and B)
        ui.vertical(|ui| {
            ui.set_width(120.0);
            ui.label(
                egui::RichText::new("Osc A")
                    .strong()
                    .size(10.0)
                    .color(palette.amber),
            );

            let osc1_wave = dev.get_param("osc1_waveform");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);
                let btn_w = 26.0;
                if ui
                    .add_sized(
                        Vec2::new(btn_w, 16.0),
                        egui::Button::selectable(osc1_wave == 0.0, "Sin"),
                    )
                    .clicked()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_waveform",
                        0.0,
                    )));
                }
                if ui
                    .add_sized(
                        Vec2::new(btn_w, 16.0),
                        egui::Button::selectable(osc1_wave == 1.0, "Saw"),
                    )
                    .clicked()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_waveform",
                        1.0,
                    )));
                }
                if ui
                    .add_sized(
                        Vec2::new(btn_w, 16.0),
                        egui::Button::selectable(osc1_wave == 2.0, "Sqr"),
                    )
                    .clicked()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_waveform",
                        2.0,
                    )));
                }
                if ui
                    .add_sized(
                        Vec2::new(btn_w, 16.0),
                        egui::Button::selectable(osc1_wave == 3.0, "Noi"),
                    )
                    .clicked()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_waveform",
                        3.0,
                    )));
                }
            });

            ui.add_space(2.0);

            ui.horizontal(|ui| {
                let mut lvl = dev.get_param("osc1_level");
                if live_knob(ui, &mut lvl, 0.0, 1.0, 24.0, "Level", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_level",
                        lvl,
                    )));
                }

                let mut coarse = dev.get_param("osc1_coarse");
                if coarse < 0.5 {
                    coarse = 1.0;
                }
                if live_knob(ui, &mut coarse, 1.0, 16.0, 24.0, "Coarse", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "osc1_coarse",
                        coarse.round(),
                    )));
                }
            });
        });

        ui.separator();

        // Sub-panel 2: Filter Section
        ui.vertical(|ui| {
            ui.set_width(100.0);
            ui.label(
                egui::RichText::new("Filter")
                    .strong()
                    .size(10.0)
                    .color(palette.cyan),
            );

            ui.horizontal(|ui| {
                let mut cutoff = dev.get_param("filter_cutoff");
                if live_knob(ui, &mut cutoff, 100.0, 18000.0, 24.0, "Freq", palette.cyan).changed()
                {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "filter_cutoff",
                        cutoff,
                    )));
                }

                let mut reso = dev.get_param("filter_resonance");
                if live_knob(ui, &mut reso, 0.1, 8.0, 24.0, "Reso", palette.cyan).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "filter_resonance",
                        reso,
                    )));
                }
            });

            let cutoff_val = dev.get_param("filter_cutoff");
            ui.label(
                egui::RichText::new(if cutoff_val > 1000.0 {
                    format!("{:.1} kHz", cutoff_val / 1000.0)
                } else {
                    format!("{:.0} Hz", cutoff_val)
                })
                .size(9.0)
                .color(palette.control_foreground),
            );
        });

        ui.separator();

        // Sub-panel 3: Envelopes ADSR
        ui.vertical(|ui| {
            ui.set_width(100.0);
            ui.label(
                egui::RichText::new("Envelope")
                    .strong()
                    .size(10.0)
                    .color(palette.control_foreground),
            );

            ui.horizontal(|ui| {
                let mut atk = dev.get_param("attack_ms");
                if live_knob(ui, &mut atk, 0.1, 500.0, 20.0, "A", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "attack_ms",
                        atk,
                    )));
                }

                let mut dec = dev.get_param("decay_ms");
                if dec < 1.0 {
                    dec = 120.0;
                }
                if live_knob(ui, &mut dec, 10.0, 1000.0, 20.0, "D", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track, dev.id, "decay_ms", dec,
                    )));
                }

                let mut sus = dev.get_param("sustain");
                if sus < 0.01 && sus > -0.01 {
                    sus = 0.7;
                }
                if live_knob(ui, &mut sus, 0.0, 1.0, 20.0, "S", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(track, dev.id, "sustain", sus)));
                }

                let mut rel = dev.get_param("release_ms");
                if live_knob(ui, &mut rel, 10.0, 2000.0, 20.0, "R", palette.amber).changed() {
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "release_ms",
                        rel,
                    )));
                }
            });

            // Mini visual envelope curve
            let (env_rect, _) = ui.allocate_exact_size(Vec2::new(96.0, 26.0), egui::Sense::hover());
            if ui.is_rect_visible(env_rect) {
                let painter = ui.painter();
                painter.rect_filled(
                    env_rect,
                    CornerRadius::same(1),
                    Color32::from_rgb(0x12, 0x12, 0x12),
                );
                painter.rect_stroke(
                    env_rect,
                    CornerRadius::same(1),
                    Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
                    eframe::egui::StrokeKind::Middle,
                );

                let p0 = Pos2::new(env_rect.min.x + 2.0, env_rect.max.y - 2.0);
                let p1 = Pos2::new(env_rect.min.x + 18.0, env_rect.min.y + 4.0);
                let p2 = Pos2::new(env_rect.min.x + 45.0, env_rect.min.y + 12.0);
                let p3 = Pos2::new(env_rect.min.x + 72.0, env_rect.min.y + 12.0);
                let p4 = Pos2::new(env_rect.max.x - 2.0, env_rect.max.y - 2.0);

                painter.line_segment([p0, p1], Stroke::new(1.2_f32, palette.amber));
                painter.line_segment([p1, p2], Stroke::new(1.2_f32, palette.amber));
                painter.line_segment([p2, p3], Stroke::new(1.2_f32, palette.amber));
                painter.line_segment([p3, p4], Stroke::new(1.2_f32, palette.amber));
            }
        });
    });
}

fn render_utility_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        let mut gain = dev.get_param("gain_db");
        if live_knob(ui, &mut gain, -35.0, 35.0, 26.0, "Gain", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track, dev.id, "gain_db", gain,
            )));
        }

        let mut width = dev.get_param("width");
        if live_knob(ui, &mut width, 0.0, 2.0, 26.0, "Width", palette.cyan).changed() {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "width", width)));
        }

        let mut pan = dev.get_param("pan");
        if live_knob(ui, &mut pan, -1.0, 1.0, 26.0, "Pan", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "pan", pan)));
        }
    });
}

fn render_saturator_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        let (curve_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(
            curve_rect,
            CornerRadius::same(2),
            Color32::from_rgb(0x10, 0x10, 0x10),
        );
        painter.rect_stroke(
            curve_rect,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, Color32::from_rgb(0x28, 0x28, 0x28)),
            eframe::egui::StrokeKind::Middle,
        );

        let c_y = curve_rect.center().y;
        let c_x = curve_rect.center().x;
        painter.line_segment(
            [
                Pos2::new(curve_rect.min.x, c_y),
                Pos2::new(curve_rect.max.x, c_y),
            ],
            Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
        );
        painter.line_segment(
            [
                Pos2::new(c_x, curve_rect.min.y),
                Pos2::new(c_x, curve_rect.max.y),
            ],
            Stroke::new(0.5_f32, Color32::from_rgb(0x30, 0x30, 0x30)),
        );

        let drive_db = dev.get_param("drive_db");
        let drive_lin = 10.0f32.powf(drive_db / 20.0);
        let curve_type = dev.get_param("curve_type") as usize;

        let num_points = 32;
        let mut points = Vec::with_capacity(num_points);
        for p in 0..num_points {
            let norm_x = (p as f32 / (num_points - 1) as f32) * 2.0 - 1.0;
            let driven = norm_x * drive_lin;
            let sat_y = match curve_type {
                1 => driven.clamp(-1.0, 1.0),
                2 => (driven / (1.0 + driven.abs()) + 0.08 * (driven * driven).min(1.0))
                    .clamp(-1.0, 1.0),
                3 => {
                    let mut v = driven;
                    while v > 1.0 || v < -1.0 {
                        if v > 1.0 {
                            v = 2.0 - v;
                        } else if v < -1.0 {
                            v = -2.0 - v;
                        }
                    }
                    v
                }
                _ => (driven.clamp(-2.0, 2.0) * std::f32::consts::FRAC_PI_2).sin(),
            };
            let px = curve_rect.min.x + (p as f32 / (num_points - 1) as f32) * curve_rect.width();
            let py = c_y - sat_y * (curve_rect.height() * 0.42);
            points.push(Pos2::new(px, py));
        }

        for i in 0..points.len() - 1 {
            painter.line_segment(
                [points[i], points[i + 1]],
                Stroke::new(1.5_f32, palette.amber),
            );
        }

        ui.add_space(3.0);

        ui.horizontal(|ui| {
            let mut drive = dev.get_param("drive_db");
            if live_knob(ui, &mut drive, 0.0, 36.0, 24.0, "Drive", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "drive_db", drive,
                )));
            }

            let mut out_db = dev.get_param("output_db");
            if live_knob(ui, &mut out_db, -24.0, 0.0, 24.0, "Out", palette.cyan).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "output_db",
                    out_db,
                )));
            }

            let mut dry_wet = dev.get_param("dry_wet");
            if live_knob(
                ui,
                &mut dry_wet,
                0.0,
                1.0,
                24.0,
                "Dry/Wet",
                palette.play_green,
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "dry_wet", dry_wet,
                )));
            }
        });

        ui.horizontal(|ui| {
            let names = ["Soft", "Hard", "Warm", "Fold"];
            let mut cur = dev.get_param("curve_type") as usize;
            for (idx, &name) in names.iter().enumerate() {
                if ui.selectable_label(cur == idx, name).clicked() {
                    cur = idx;
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "curve_type",
                        cur as f32,
                    )));
                }
            }
        });
    });
}

fn render_autofilter_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let modes = ["LP", "HP", "BP", "Notch"];
            let mut cur_mode = dev.get_param("filter_type").round() as usize;
            for (idx, &name) in modes.iter().enumerate() {
                if ui.selectable_label(cur_mode == idx, name).clicked() {
                    cur_mode = idx;
                    edits.push(Box::new(SetDeviceParam::new(
                        track,
                        dev.id,
                        "filter_type",
                        cur_mode as f32,
                    )));
                }
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let mut cutoff = dev.get_param("cutoff");
            if live_knob(ui, &mut cutoff, 20.0, 18000.0, 24.0, "Freq", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "cutoff", cutoff,
                )));
            }

            let mut res = dev.get_param("resonance");
            if live_knob(ui, &mut res, 0.1, 10.0, 24.0, "Res", palette.cyan).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "resonance",
                    res,
                )));
            }

            let mut lfo_rate = dev.get_param("lfo_rate");
            if live_knob(
                ui,
                &mut lfo_rate,
                0.1,
                20.0,
                24.0,
                "Rate",
                palette.play_green,
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "lfo_rate", lfo_rate,
                )));
            }

            let mut lfo_depth = dev.get_param("lfo_depth");
            if live_knob(
                ui,
                &mut lfo_depth,
                0.0,
                1.0,
                24.0,
                "Depth",
                Color32::from_rgb(0xff, 0x77, 0xee),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "lfo_depth",
                    lfo_depth,
                )));
            }
        });
    });
}

fn render_chorus_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let mut rate = dev.get_param("rate_hz");
            if live_knob(ui, &mut rate, 0.1, 5.0, 24.0, "Rate", palette.cyan).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "rate_hz", rate,
                )));
            }

            let mut depth = dev.get_param("depth");
            if live_knob(ui, &mut depth, 0.0, 1.0, 24.0, "Depth", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(track, dev.id, "depth", depth)));
            }

            let mut fb = dev.get_param("feedback");
            if live_knob(
                ui,
                &mut fb,
                0.0,
                0.7,
                24.0,
                "Feedb",
                Color32::from_rgb(0xff, 0x88, 0x44),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(track, dev.id, "feedback", fb)));
            }

            let mut dw = dev.get_param("dry_wet");
            if live_knob(ui, &mut dw, 0.0, 1.0, 24.0, "Dry/Wet", palette.play_green).changed() {
                edits.push(Box::new(SetDeviceParam::new(track, dev.id, "dry_wet", dw)));
            }
        });
    });
}

fn render_drumrack_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        let pad_names = [
            ("Kick", 36),
            ("Rim", 37),
            ("Snr 1", 38),
            ("Clap", 39),
            ("Snr 2", 40),
            ("LoTom", 41),
            ("ClsHH", 42),
            ("MdTom", 43),
            ("PedHH", 44),
            ("HiTom", 45),
            ("OpnHH", 46),
            ("FlTom", 47),
            ("Crash", 48),
            ("Ride", 49),
            ("Bell", 50),
            ("Perc", 51),
        ];

        egui::Grid::new("drum_rack_grid")
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for (i, (name, _note)) in pad_names.iter().enumerate() {
                    let btn = egui::Button::new(
                        egui::RichText::new(*name)
                            .size(9.0)
                            .color(palette.control_foreground),
                    )
                    .min_size(egui::Vec2::new(48.0, 22.0))
                    .fill(Color32::from_rgb(0x28, 0x28, 0x28));

                    if ui.add(btn).clicked() {
                        // Pad trigger handled live
                    }

                    if (i + 1) % 4 == 0 {
                        ui.end_row();
                    }
                }
            });

        ui.horizontal(|ui| {
            let mut gain = dev.get_param("gain_db");
            if live_knob(ui, &mut gain, -24.0, 6.0, 20.0, "Gain", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "gain_db", gain,
                )));
            }
        });
    });
}

fn render_autopan_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let mut amount = dev.get_param("amount");
            if live_knob(ui, &mut amount, 0.0, 1.0, 24.0, "Amount", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "amount", amount,
                )));
            }

            let mut rate = dev.get_param("rate_hz");
            if live_knob(
                ui,
                &mut rate,
                0.05,
                20.0,
                24.0,
                "Rate",
                Color32::from_rgb(0x40, 0xd2, 0xff),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "rate_hz", rate,
                )));
            }

            let mut phase = dev.get_param("phase_deg");
            if live_knob(
                ui,
                &mut phase,
                0.0,
                360.0,
                24.0,
                "Phase",
                Color32::from_rgb(0xff, 0x76, 0x4c),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "phase_deg",
                    phase,
                )));
            }
        });

        ui.horizontal(|ui| {
            let mode = dev.get_param("mode");
            let is_trem = mode >= 0.5;
            if ui.selectable_label(!is_trem, "Pan").clicked() {
                edits.push(Box::new(SetDeviceParam::new(track, dev.id, "mode", 0.0)));
            }
            if ui.selectable_label(is_trem, "Trem").clicked() {
                edits.push(Box::new(SetDeviceParam::new(track, dev.id, "mode", 1.0)));
            }
        });
    });
}

fn render_gate_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let mut thresh = dev.get_param("threshold_db");
            if live_knob(ui, &mut thresh, -60.0, 0.0, 24.0, "Thresh", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "threshold_db",
                    thresh,
                )));
            }

            let mut ret = dev.get_param("return_db");
            if live_knob(
                ui,
                &mut ret,
                0.0,
                12.0,
                24.0,
                "Return",
                Color32::from_rgb(0xff, 0xaa, 0x44),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "return_db",
                    ret,
                )));
            }

            let mut floor = dev.get_param("floor_db");
            if live_knob(
                ui,
                &mut floor,
                -80.0,
                0.0,
                24.0,
                "Floor",
                Color32::from_rgb(0x88, 0x88, 0x88),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track, dev.id, "floor_db", floor,
                )));
            }
        });

        ui.horizontal(|ui| {
            let mut att = dev.get_param("attack_ms");
            if live_knob(
                ui,
                &mut att,
                0.1,
                50.0,
                20.0,
                "Att",
                palette.control_foreground,
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "attack_ms",
                    att,
                )));
            }

            let mut rel = dev.get_param("release_ms");
            if live_knob(
                ui,
                &mut rel,
                5.0,
                500.0,
                20.0,
                "Rel",
                palette.control_foreground,
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "release_ms",
                    rel,
                )));
            }
        });
    });
}

fn render_eq3_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        // Low band
        ui.vertical(|ui| {
            let mut low = dev.get_param("low_gain_db");
            if live_knob(ui, &mut low, -24.0, 6.0, 24.0, "Low", palette.amber).changed() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "low_gain_db",
                    low,
                )));
            }
            let low_on = dev.get_param("low_on") > 0.5;
            if ui.selectable_label(low_on, "On").clicked() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "low_on",
                    if low_on { 0.0 } else { 1.0 },
                )));
            }
        });

        // Mid band
        ui.vertical(|ui| {
            let mut mid = dev.get_param("mid_gain_db");
            if live_knob(
                ui,
                &mut mid,
                -24.0,
                6.0,
                24.0,
                "Mid",
                Color32::from_rgb(0x40, 0xd2, 0xff),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "mid_gain_db",
                    mid,
                )));
            }
            let mid_on = dev.get_param("mid_on") > 0.5;
            if ui.selectable_label(mid_on, "On").clicked() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "mid_on",
                    if mid_on { 0.0 } else { 1.0 },
                )));
            }
        });

        // High band
        ui.vertical(|ui| {
            let mut high = dev.get_param("high_gain_db");
            if live_knob(
                ui,
                &mut high,
                -24.0,
                6.0,
                24.0,
                "High",
                Color32::from_rgb(0xff, 0x4c, 0x99),
            )
            .changed()
            {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "high_gain_db",
                    high,
                )));
            }
            let high_on = dev.get_param("high_on") > 0.5;
            if ui.selectable_label(high_on, "On").clicked() {
                edits.push(Box::new(SetDeviceParam::new(
                    track,
                    dev.id,
                    "high_on",
                    if high_on { 0.0 } else { 1.0 },
                )));
            }
        });
    });
}

fn render_redux_controls(
    ui: &mut Ui,
    track: TrackId,
    dev: &Device,
    palette: &LivePalette,
    edits: &mut Edits,
) {
    ui.horizontal(|ui| {
        let mut rate = dev.get_param("rate_factor");
        if live_knob(ui, &mut rate, 1.0, 32.0, 24.0, "Down", palette.amber).changed() {
            edits.push(Box::new(SetDeviceParam::new(
                track,
                dev.id,
                "rate_factor",
                rate,
            )));
        }

        let mut bits = dev.get_param("bits");
        if live_knob(
            ui,
            &mut bits,
            1.0,
            16.0,
            24.0,
            "Bits",
            Color32::from_rgb(0xff, 0x55, 0x59),
        )
        .changed()
        {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "bits", bits)));
        }

        let mut dw = dev.get_param("dry_wet");
        if live_knob(ui, &mut dw, 0.0, 1.0, 24.0, "Dry/Wet", palette.play_green).changed() {
            edits.push(Box::new(SetDeviceParam::new(track, dev.id, "dry_wet", dw)));
        }
    });
}
