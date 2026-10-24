use eframe::egui::{self, Color32, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};
use std::f32::consts::PI;

pub fn live_knob(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    size: f32,
    label: &str,
    accent: Color32,
) -> Response {
    let desired_size = Vec2::new(size, size + 14.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

    if response.dragged() {
        let delta = -response.drag_delta().y * (max - min) / 150.0;
        *value = (*value + delta).clamp(min, max);
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = Pos2::new(rect.center().x, rect.min.y + size * 0.5);
        let radius = size * 0.42;

        // Background circle
        painter.circle_filled(center, radius, Color32::from_rgb(0x1a, 0x1a, 0x1a));
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x40, 0x40)),
        );

        // Value arc (from 135 deg to 405 deg)
        let norm = (*value - min) / (max - min).max(1e-5);
        let start_angle = 0.75 * PI;
        let end_angle = start_angle + norm * 1.5 * PI;

        // Draw pointer line
        let p_x = center.x + radius * 0.75 * end_angle.cos();
        let p_y = center.y + radius * 0.75 * end_angle.sin();
        painter.line_segment([center, Pos2::new(p_x, p_y)], Stroke::new(2.0_f32, accent));

        // Label under knob
        let text_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.min.y + size),
            Vec2::new(rect.width(), 14.0),
        );
        painter.text(
            text_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(0xaa, 0xaa, 0xaa),
        );
    }

    response
}

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
        let delta = -response.drag_delta().y * (max_db - min_db) / (size.y - 10.0);
        *value_db = (*value_db + delta).clamp(min_db, max_db);
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Trough
        let trough_rect =
            Rect::from_center_size(rect.center(), Vec2::new(4.0, rect.height() - 8.0));
        painter.rect_filled(
            trough_rect,
            Rounding::same(1.0),
            Color32::from_rgb(0x18, 0x18, 0x18),
        );
        painter.rect_stroke(
            trough_rect,
            Rounding::same(1.0),
            Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x38, 0x38)),
        );

        // Handle
        let norm = (*value_db - min_db) / (max_db - min_db).max(1e-5);
        let handle_y = rect.max.y - 6.0 - norm * (rect.height() - 16.0);
        let handle_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, handle_y),
            Vec2::new(size.x * 0.75, 10.0),
        );

        let handle_color = if response.hovered() || response.dragged() {
            accent
        } else {
            Color32::from_rgb(0x60, 0x60, 0x60)
        };

        painter.rect_filled(handle_rect, Rounding::same(2.0), handle_color);
        painter.rect_stroke(
            handle_rect,
            Rounding::same(2.0),
            Stroke::new(1.0_f32, Color32::BLACK),
        );
        // Notch in center of handle
        painter.line_segment(
            [
                Pos2::new(handle_rect.min.x + 2.0, handle_rect.center().y),
                Pos2::new(handle_rect.max.x - 2.0, handle_rect.center().y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(0x20, 0x20, 0x20)),
        );
    }

    response
}

pub fn live_stereo_meter(ui: &mut Ui, peak_l_db: f32, peak_r_db: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            Rounding::same(1.0),
            Color32::from_rgb(0x10, 0x10, 0x10),
        );

        let half_w = (rect.width() - 3.0) * 0.5;

        let draw_channel = |p_db: f32, x_offset: f32| {
            let clamped = p_db.clamp(-60.0, 0.0);
            let norm = (clamped + 60.0) / 60.0;
            let meter_h = (rect.height() - 2.0) * norm;

            let col_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + x_offset, rect.max.y - 1.0 - meter_h),
                Vec2::new(half_w, meter_h),
            );

            let color = if p_db > -3.0 {
                Color32::from_rgb(0xff, 0x48, 0x48) // Red clip
            } else if p_db > -12.0 {
                Color32::from_rgb(0xff, 0xd0, 0x3c) // Yellow warn
            } else {
                Color32::from_rgb(0x40, 0xff, 0x63) // Green peak
            };

            painter.rect_filled(col_rect, Rounding::ZERO, color);
        };

        draw_channel(peak_l_db, 1.0);
        draw_channel(peak_r_db, 2.0 + half_w);
    }
}
