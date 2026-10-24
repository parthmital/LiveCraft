use crate::style::LivePalette;
use crate::widgets::{live_fader, live_knob, live_stereo_meter};
use eframe::egui::{self, Color32, Rounding, Ui, Vec2};
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
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
        let track_width = 88.0;

        // Mixer strips for player tracks
        for (trk_idx, trk) in project.tracks.iter_mut().enumerate() {
            let meter = track_meters.get(trk_idx).cloned().unwrap_or_default();

            ui.vertical(|ui| {
                ui.set_width(track_width);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                // Send A and Send B
                ui.horizontal(|ui| {
                    if let Some(send_a) = trk.sends.first_mut() {
                        live_knob(ui, send_a, 0.0, 1.0, 26.0, "A", palette.amber);
                    }
                    if let Some(send_b) = trk.sends.get_mut(1) {
                        live_knob(ui, send_b, 0.0, 1.0, 26.0, "B", palette.cyan);
                    }
                });

                // Pan knob
                live_knob(ui, &mut trk.pan, -1.0, 1.0, 26.0, "Pan", palette.amber);

                // Volume Fader & Stereo VU meter side by side
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    live_fader(
                        ui,
                        &mut trk.volume_db,
                        -60.0,
                        6.0,
                        Vec2::new(26.0, 110.0),
                        palette.amber,
                    );
                    live_stereo_meter(
                        ui,
                        meter.peak_db_l(),
                        meter.peak_db_r(),
                        Vec2::new(14.0, 110.0),
                    );
                });

                // Track volume dB read-out
                ui.label(
                    egui::RichText::new(format!("{:.1} dB", trk.volume_db))
                        .size(9.0)
                        .color(Color32::from_rgb(0x99, 0x99, 0x99)),
                );

                // Buttons row: Track Activator (Mute), Solo (S), Arm (●)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                    // Track Activator (Yellow number)
                    let active = !trk.is_muted;
                    let act_color = if active {
                        palette.amber
                    } else {
                        palette.control_disabled
                    };
                    let act_fill = if active {
                        Color32::from_rgb(0x5a, 0x3d, 0x18)
                    } else {
                        palette.control_background
                    };
                    let act_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!("{}", trk_idx + 1))
                                .size(10.0)
                                .color(act_color),
                        )
                        .fill(act_fill)
                        .min_size(Vec2::new(22.0, 18.0))
                        .rounding(Rounding::same(2.0)),
                    );
                    if act_btn.clicked() {
                        trk.is_muted = !trk.is_muted;
                    }

                    // Solo button (S)
                    let solo = trk.is_soloed;
                    let solo_color = if solo {
                        Color32::WHITE
                    } else {
                        palette.control_disabled
                    };
                    let solo_fill = if solo {
                        palette.solo_blue
                    } else {
                        palette.control_background
                    };
                    let solo_btn = ui.add(
                        egui::Button::new(egui::RichText::new("S").size(10.0).color(solo_color))
                            .fill(solo_fill)
                            .min_size(Vec2::new(20.0, 18.0))
                            .rounding(Rounding::same(2.0)),
                    );
                    if solo_btn.clicked() {
                        trk.is_soloed = !trk.is_soloed;
                    }

                    // Arm button (●)
                    let armed = trk.is_armed;
                    let arm_color = if armed {
                        Color32::WHITE
                    } else {
                        palette.control_disabled
                    };
                    let arm_fill = if armed {
                        palette.record_red
                    } else {
                        palette.control_background
                    };
                    let arm_btn = ui.add(
                        egui::Button::new(egui::RichText::new("●").size(10.0).color(arm_color))
                            .fill(arm_fill)
                            .min_size(Vec2::new(20.0, 18.0))
                            .rounding(Rounding::same(2.0)),
                    );
                    if arm_btn.clicked() {
                        trk.is_armed = !trk.is_armed;
                    }
                });
            });
        }

        // Return tracks strips (A-Reverb, B-Delay)
        for (ret_idx, ret) in project.return_tracks.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.set_width(track_width * 0.75);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                ui.add_space(34.0); // No sends on return
                live_knob(ui, &mut ret.pan, -1.0, 1.0, 24.0, "Pan", palette.amber);

                ui.horizontal(|ui| {
                    live_fader(
                        ui,
                        &mut ret.volume_db,
                        -60.0,
                        6.0,
                        Vec2::new(24.0, 110.0),
                        palette.amber,
                    );
                });

                ui.label(
                    egui::RichText::new(format!("{:.1} dB", ret.volume_db))
                        .size(9.0)
                        .color(Color32::from_rgb(0x99, 0x99, 0x99)),
                );

                // Letter button (A / B)
                let letter = (b'A' + ret_idx as u8) as char;
                let active = !ret.is_muted;
                let btn = ui.add(
                    egui::Button::new(egui::RichText::new(format!("{}", letter)).size(10.0).color(
                        if active {
                            palette.amber
                        } else {
                            palette.control_disabled
                        },
                    ))
                    .fill(if active {
                        Color32::from_rgb(0x5a, 0x3d, 0x18)
                    } else {
                        palette.control_background
                    })
                    .min_size(Vec2::new(24.0, 18.0))
                    .rounding(Rounding::same(2.0)),
                );
                if btn.clicked() {
                    ret.is_muted = !ret.is_muted;
                }
            });
        }

        // Master strip
        ui.vertical(|ui| {
            ui.set_width(65.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

            ui.add_space(34.0);
            live_knob(
                ui,
                &mut project.master_track.pan,
                -1.0,
                1.0,
                24.0,
                "Pan",
                palette.amber,
            );

            ui.horizontal(|ui| {
                live_fader(
                    ui,
                    &mut project.master_track.volume_db,
                    -60.0,
                    6.0,
                    Vec2::new(24.0, 110.0),
                    palette.amber,
                );
                live_stereo_meter(
                    ui,
                    master_meter.peak_db_l(),
                    master_meter.peak_db_r(),
                    Vec2::new(14.0, 110.0),
                );
            });

            ui.label(
                egui::RichText::new(format!("{:.1} dB", project.master_track.volume_db))
                    .size(9.0)
                    .color(palette.amber),
            );

            ui.label(
                egui::RichText::new("MAIN")
                    .size(10.0)
                    .color(palette.control_foreground),
            );
        });
    });
}
