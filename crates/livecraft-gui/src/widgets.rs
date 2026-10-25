use eframe::egui::{self, Color32, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};
use std::f32::consts::PI;

/// Authentic Ableton Live 12 rotary control knob.
pub fn live_knob(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    size: f32,
    label: &str,
    accent: Color32,
) -> Response {
    let desired_size = Vec2::new(size, size + 13.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

    if response.dragged() {
        let delta = -response.drag_delta().y * (max - min) / 140.0;
        *value = (*value + delta).clamp(min, max);
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = Pos2::new(rect.center().x, rect.min.y + size * 0.5);
        let radius = size * 0.42;

        // Background well
        painter.circle_filled(center, radius, Color32::from_rgb(0x18, 0x18, 0x18));
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(1.0_f32, Color32::from_rgb(0x3a, 0x3a, 0x3a)),
        );

        // Value arc track
        let norm = (*value - min) / (max - min).max(1e-5);
        let start_angle = 0.75 * PI;
        let arc_span = 1.5 * PI;
        let end_angle = start_angle + norm * arc_span;

        // Inactive background arc track
        let steps = 18;
        for i in 0..steps {
            let a1 = start_angle + (i as f32 / steps as f32) * arc_span;
            let a2 = start_angle + ((i + 1) as f32 / steps as f32) * arc_span;
            let p1 = Pos2::new(
                center.x + radius * 0.82 * a1.cos(),
                center.y + radius * 0.82 * a1.sin(),
            );
            let p2 = Pos2::new(
                center.x + radius * 0.82 * a2.cos(),
                center.y + radius * 0.82 * a2.sin(),
            );
            painter.line_segment(
                [p1, p2],
                Stroke::new(1.5_f32, Color32::from_rgb(0x2d, 0x2d, 0x2d)),
            );
        }

        // Active illuminated arc
        let active_steps = (steps as f32 * norm).ceil() as usize;
        for i in 0..active_steps {
            let a1 = start_angle + (i as f32 / steps as f32) * arc_span;
            let a2 = (start_angle + ((i + 1) as f32 / steps as f32) * arc_span).min(end_angle);
            let p1 = Pos2::new(
                center.x + radius * 0.82 * a1.cos(),
                center.y + radius * 0.82 * a1.sin(),
            );
            let p2 = Pos2::new(
                center.x + radius * 0.82 * a2.cos(),
                center.y + radius * 0.82 * a2.sin(),
            );
            painter.line_segment([p1, p2], Stroke::new(1.8_f32, accent));
        }

        // Center pointer line
        let p_x = center.x + radius * 0.75 * end_angle.cos();
        let p_y = center.y + radius * 0.75 * end_angle.sin();
        let needle_color = if response.hovered() || response.dragged() {
            Color32::WHITE
        } else {
            accent
        };
        painter.line_segment(
            [center, Pos2::new(p_x, p_y)],
            Stroke::new(2.0_f32, needle_color),
        );

        // Center hub dot
        painter.circle_filled(center, 2.0, Color32::from_rgb(0x10, 0x10, 0x10));

        // Label under knob
        let text_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.min.y + size),
            Vec2::new(rect.width(), 13.0),
        );
        painter.text(
            text_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(0xa0, 0xa0, 0xa0),
        );
    }

    response
}

/// Authentic Ableton Live 12 vertical volume fader.
pub fn live_fader(
    ui: &mut Ui,
    value_db: &mut f32,
    min_db: f32,
    max_db: f32,
    size: Vec2,
    accent: Color32,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());

    if response.dragged() {
        let delta = -response.drag_delta().y * (max_db - min_db) / (size.y - 14.0);
        *value_db = (*value_db + delta).clamp(min_db, max_db);
        response.mark_changed();
    }

    if response.double_clicked() {
        // Double click resets fader to 0.0 dB (Ableton standard)
        *value_db = 0.0;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Background trough well
        let trough_rect =
            Rect::from_center_size(rect.center(), Vec2::new(5.0, rect.height() - 4.0));
        painter.rect_filled(
            trough_rect,
            Rounding::same(1.0),
            Color32::from_rgb(0x14, 0x14, 0x14),
        );
        painter.rect_stroke(
            trough_rect,
            Rounding::same(1.0),
            Stroke::new(1.0_f32, Color32::from_rgb(0x35, 0x35, 0x35)),
        );

        // 0 dB center reference line tick
        let norm_zero = (0.0 - min_db) / (max_db - min_db);
        let zero_y = rect.max.y - 7.0 - norm_zero * (rect.height() - 14.0);
        painter.line_segment(
            [
                Pos2::new(rect.min.x + 1.0, zero_y),
                Pos2::new(rect.min.x + 5.0, zero_y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(0x88, 0x88, 0x88)),
        );
        painter.line_segment(
            [
                Pos2::new(rect.max.x - 5.0, zero_y),
                Pos2::new(rect.max.x - 1.0, zero_y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(0x88, 0x88, 0x88)),
        );

        // Fader handle position
        let norm = (*value_db - min_db) / (max_db - min_db).max(1e-5);
        let handle_y = rect.max.y - 7.0 - norm * (rect.height() - 14.0);
        let handle_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, handle_y),
            Vec2::new(size.x * 0.85, 12.0),
        );

        let cap_base = if response.hovered() || response.dragged() {
            Color32::from_rgb(0x75, 0x75, 0x75)
        } else {
            Color32::from_rgb(0x5a, 0x5a, 0x5a)
        };

        // Ableton metallic cap with 1px top highlight and black border
        painter.rect_filled(handle_rect, Rounding::same(2.0), cap_base);
        painter.rect_stroke(
            handle_rect,
            Rounding::same(2.0),
            Stroke::new(1.0_f32, Color32::from_rgb(0x18, 0x18, 0x18)),
        );

        // Horizontal center groove line
        let groove_color = if response.dragged() {
            accent
        } else {
            Color32::from_rgb(0x22, 0x22, 0x22)
        };
        painter.line_segment(
            [
                Pos2::new(handle_rect.min.x + 2.0, handle_rect.center().y),
                Pos2::new(handle_rect.max.x - 2.0, handle_rect.center().y),
            ],
            Stroke::new(1.5_f32, groove_color),
        );
    }

    response
}

/// Authentic Ableton Live 12 stereo peak VU meter with multi-segment color gradient.
pub fn live_stereo_meter(ui: &mut Ui, peak_l_db: f32, peak_r_db: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // Meter well background
        painter.rect_filled(
            rect,
            Rounding::same(1.0),
            Color32::from_rgb(0x12, 0x12, 0x12),
        );
        painter.rect_stroke(
            rect,
            Rounding::same(1.0),
            Stroke::new(1.0_f32, Color32::from_rgb(0x22, 0x22, 0x22)),
        );

        let clip_box_h = 4.0;
        let active_meter_h = rect.height() - clip_box_h - 3.0;
        let half_w = (rect.width() - 3.0) * 0.5;

        let draw_channel = |p_db: f32, x_offset: f32| {
            let clamped = p_db.clamp(-60.0, 6.0);
            let norm = ((clamped + 60.0) / 66.0).clamp(0.0, 1.0);
            let _meter_fill_h = active_meter_h * norm;

            let channel_x = rect.min.x + x_offset;
            let meter_bottom = rect.max.y - 1.0;

            // Draw segmented gradient levels
            let segments = 24;
            let seg_h = active_meter_h / segments as f32;
            let filled_segs = (segments as f32 * norm).round() as usize;

            for s in 0..filled_segs {
                let seg_norm = s as f32 / segments as f32;
                let seg_color = if seg_norm > 0.88 {
                    Color32::from_rgb(0xff, 0x45, 0x45) // Red clip
                } else if seg_norm > 0.70 {
                    Color32::from_rgb(0xff, 0xd0, 0x36) // Yellow warn
                } else {
                    Color32::from_rgb(0x28, 0xe0, 0x58) // Green peak
                };

                let y_top = meter_bottom - (s + 1) as f32 * seg_h;
                let seg_rect = Rect::from_min_size(
                    Pos2::new(channel_x, y_top + 0.5),
                    Vec2::new(half_w, seg_h - 1.0),
                );
                painter.rect_filled(seg_rect, Rounding::ZERO, seg_color);
            }

            // Clip indicator box at top of meter
            let clip_rect = Rect::from_min_size(
                Pos2::new(channel_x, rect.min.y + 1.0),
                Vec2::new(half_w, clip_box_h),
            );
            let clip_color = if p_db > 0.0 {
                Color32::from_rgb(0xff, 0x30, 0x30)
            } else {
                Color32::from_rgb(0x25, 0x25, 0x25)
            };
            painter.rect_filled(clip_rect, Rounding::same(1.0), clip_color);
        };

        draw_channel(peak_l_db, 1.0);
        draw_channel(peak_r_db, 2.0 + half_w);
    }
}

/// Authentic Ableton Live 12 dark LCD readout box with recessed frame.
pub fn live_lcd_box(
    ui: &mut Ui,
    text: &str,
    size: Vec2,
    accent: Color32,
    highlight: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = Color32::from_rgb(0x14, 0x14, 0x14);
        let border = if highlight || response.hovered() {
            accent
        } else {
            Color32::from_rgb(0x38, 0x38, 0x38)
        };

        painter.rect_filled(rect, Rounding::same(2.0), bg);
        painter.rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));

        let text_color = if highlight {
            accent
        } else if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xcc, 0xcc, 0xcc)
        };

        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::monospace(11.0),
            text_color,
        );
    }
    response
}

/// Authentic Ableton Live 12 toggle button with illumination state.
pub fn live_toggle_button(
    ui: &mut Ui,
    is_active: bool,
    active_color: Color32,
    text: &str,
    size: Vec2,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let (bg, border, fg) = if is_active {
            (active_color, active_color, Color32::BLACK)
        } else if response.hovered() {
            (
                Color32::from_rgb(0x42, 0x42, 0x42),
                Color32::from_rgb(0x55, 0x55, 0x55),
                Color32::from_rgb(0xee, 0xee, 0xee),
            )
        } else {
            (
                Color32::from_rgb(0x2d, 0x2d, 0x2d),
                Color32::from_rgb(0x38, 0x38, 0x38),
                Color32::from_rgb(0x99, 0x99, 0x99),
            )
        };

        painter.rect_filled(rect, Rounding::same(2.0), bg);
        painter.rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(10.5),
            fg,
        );
    }
    response
}

/// Subtle vertical divider between sections in Ableton Live bars.
pub fn live_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, height), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let x = rect.center().x;
        painter.line_segment(
            [
                Pos2::new(x, rect.min.y + 3.0),
                Pos2::new(x, rect.max.y - 3.0),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(0x3a, 0x3a, 0x3a)),
        );
    }
}
