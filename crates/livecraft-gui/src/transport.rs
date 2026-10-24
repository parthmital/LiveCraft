use crate::style::LivePalette;
use eframe::egui::{self, Color32, Rounding, Ui, Vec2};
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
}

pub fn render_transport_bar(
    ui: &mut Ui,
    project: &mut Project,
    current_view: ViewMode,
    palette: &LivePalette,
    cpu_percent: f32,
) -> TransportBarResponse {
    let mut resp = TransportBarResponse {
        play_toggled: false,
        stop_clicked: false,
        record_toggled: false,
        view_mode_changed: None,
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

        // Tap tempo button
        let tap_btn = ui.add(
            egui::Button::new(
                egui::RichText::new("TAP")
                    .size(10.0)
                    .color(Color32::from_rgb(0xaa, 0xaa, 0xaa)),
            )
            .fill(palette.control_background)
            .rounding(Rounding::same(2.0)),
        );
        if tap_btn.clicked() {
            // Future: Tap tempo average
        }

        // Tempo BPM LCD display (draggable)
        let mut bpm = project.transport.bpm;
        let bpm_resp = ui.add(
            egui::DragValue::new(&mut bpm)
                .speed(0.2)
                .range(20.0..=999.0)
                .suffix(" BPM")
                .custom_formatter(|n, _| format!("{:.2}", n)),
        );
        if bpm_resp.changed() {
            project.transport.bpm = bpm;
        }

        ui.separator();

        // Time Signature LCD
        ui.label(
            egui::RichText::new(format!(
                "{}/{}",
                project.transport.time_signature.0, project.transport.time_signature.1
            ))
            .monospace()
            .size(12.0)
            .color(palette.control_foreground),
        );

        // Metronome button
        let metro_active = project.transport.metronome_enabled;
        let metro_color = if metro_active {
            palette.amber
        } else {
            palette.control_disabled
        };
        let metro_btn = ui.add(
            egui::Button::new(egui::RichText::new("●●").size(10.0).color(metro_color))
                .fill(palette.control_background)
                .rounding(Rounding::same(2.0)),
        );
        if metro_btn.clicked() {
            project.transport.metronome_enabled = !project.transport.metronome_enabled;
        }

        ui.separator();

        // Transport Controls: Play, Stop, Record
        let is_playing = project.transport.is_playing;
        let play_btn_color = if is_playing {
            palette.play_green
        } else {
            Color32::from_rgb(0x80, 0x80, 0x80)
        };
        let play_btn = ui.add(
            egui::Button::new(egui::RichText::new("▶").size(13.0).color(play_btn_color))
                .fill(if is_playing {
                    Color32::from_rgb(0x1a, 0x3d, 0x22)
                } else {
                    palette.control_background
                })
                .min_size(Vec2::new(28.0, 22.0))
                .rounding(Rounding::same(2.0)),
        );
        if play_btn.clicked() {
            project.transport.is_playing = !project.transport.is_playing;
            resp.play_toggled = true;
        }

        let stop_btn = ui.add(
            egui::Button::new(
                egui::RichText::new("■")
                    .size(11.0)
                    .color(Color32::from_rgb(0xbb, 0xbb, 0xbb)),
            )
            .fill(palette.control_background)
            .min_size(Vec2::new(24.0, 22.0))
            .rounding(Rounding::same(2.0)),
        );
        if stop_btn.clicked() {
            project.transport.is_playing = false;
            project.transport.playhead_beats = 0.0;
            resp.stop_clicked = true;
        }

        let is_recording = project.transport.is_recording;
        let rec_color = if is_recording {
            palette.record_red
        } else {
            Color32::from_rgb(0x80, 0x80, 0x80)
        };
        let rec_btn = ui.add(
            egui::Button::new(egui::RichText::new("●").size(12.0).color(rec_color))
                .fill(if is_recording {
                    Color32::from_rgb(0x40, 0x18, 0x18)
                } else {
                    palette.control_background
                })
                .min_size(Vec2::new(24.0, 22.0))
                .rounding(Rounding::same(2.0)),
        );
        if rec_btn.clicked() {
            project.transport.is_recording = !project.transport.is_recording;
            resp.record_toggled = true;
        }

        ui.separator();

        // Position Display (Bar . Beat . 16th)
        let beats = project.transport.playhead_beats;
        let bar = (beats / 4.0) as u32 + 1;
        let beat = (beats % 4.0) as u32 + 1;
        let sixteenth = ((beats * 4.0) % 4.0) as u32 + 1;

        ui.label(
            egui::RichText::new(format!("{:03}. {:01}. {:01}", bar, beat, sixteenth))
                .monospace()
                .size(13.0)
                .color(palette.amber),
        );

        ui.separator();

        // Loop button
        let loop_active = project.transport.loop_enabled;
        let loop_color = if loop_active {
            palette.amber
        } else {
            palette.control_disabled
        };
        let loop_btn = ui.add(
            egui::Button::new(egui::RichText::new("🔁").size(10.0).color(loop_color))
                .fill(palette.control_background)
                .rounding(Rounding::same(2.0)),
        );
        if loop_btn.clicked() {
            project.transport.loop_enabled = !project.transport.loop_enabled;
        }

        // View Mode Switcher: Session vs Arrangement
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // CPU Load Meter
            ui.label(
                egui::RichText::new(format!("{:.0}% CPU", cpu_percent))
                    .size(10.0)
                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
            );
            let cpu_rect = ui
                .allocate_exact_size(Vec2::new(24.0, 8.0), egui::Sense::hover())
                .0;
            let cpu_painter = ui.painter();
            cpu_painter.rect_filled(
                cpu_rect,
                Rounding::ZERO,
                Color32::from_rgb(0x18, 0x18, 0x18),
            );
            let fill_w = cpu_rect.width() * (cpu_percent / 100.0).clamp(0.0, 1.0);
            cpu_painter.rect_filled(
                egui::Rect::from_min_size(cpu_rect.min, Vec2::new(fill_w, cpu_rect.height())),
                Rounding::ZERO,
                palette.amber,
            );

            ui.separator();

            // Arrangement view button [=]
            let is_arr = current_view == ViewMode::Arrangement;
            let arr_btn = ui.add(
                egui::Button::new(egui::RichText::new("═").size(11.0).color(if is_arr {
                    Color32::BLACK
                } else {
                    palette.control_foreground
                }))
                .fill(if is_arr {
                    palette.amber
                } else {
                    palette.control_background
                })
                .rounding(Rounding::same(2.0)),
            );
            if arr_btn.clicked() {
                resp.view_mode_changed = Some(ViewMode::Arrangement);
            }

            // Session view button [|||]
            let is_sess = current_view == ViewMode::Session;
            let sess_btn = ui.add(
                egui::Button::new(egui::RichText::new("|||").size(10.0).color(if is_sess {
                    Color32::BLACK
                } else {
                    palette.control_foreground
                }))
                .fill(if is_sess {
                    palette.amber
                } else {
                    palette.control_background
                })
                .rounding(Rounding::same(2.0)),
            );
            if sess_btn.clicked() {
                resp.view_mode_changed = Some(ViewMode::Session);
            }
        });
    });

    resp
}
