use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::f32::consts::PI;

/// Converts a dB value (-60.0..=+6.0) to normalized fader travel (0.0..=1.0).
/// 0.0 dB is positioned at exactly norm = 0.80 (20% from the top of the fader).
pub fn db_to_norm(db: f32) -> f32 {
    if db <= -60.0 {
        0.0
    } else if db >= 6.0 {
        1.0
    } else if db >= 0.0 {
        0.80 + (db / 6.0) * 0.20
    } else if db >= -12.0 {
        0.50 + ((db + 12.0) / 12.0) * 0.30
    } else if db >= -24.0 {
        0.30 + ((db + 24.0) / 12.0) * 0.20
    } else {
        ((db + 60.0) / 36.0).clamp(0.0, 1.0) * 0.30
    }
}

/// Converts a normalized fader travel (0.0..=1.0) back to dB (-60.0..=+6.0).
pub fn norm_to_db(norm: f32) -> f32 {
    let n = norm.clamp(0.0, 1.0);
    if n <= 0.01 {
        -60.0
    } else if n >= 0.9999 {
        6.0
    } else if (n - 0.80).abs() < 0.0001 {
        0.0
    } else if n >= 0.80 {
        (n - 0.80) / 0.20 * 6.0
    } else if n >= 0.50 {
        -12.0 + (n - 0.50) / 0.30 * 12.0
    } else if n >= 0.30 {
        -24.0 + (n - 0.30) / 0.20 * 12.0
    } else {
        -60.0 + (n / 0.30) * 36.0
    }
}

/// Authentic Ableton Live 12 rotary control knob.
/// Automatically handles unipolar (0..max) and bipolar (-max..+max) parameters.
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

    let is_bipolar = min < 0.0 && max > 0.0;

    if response.dragged() {
        let delta = -response.drag_delta().y * (max - min) / 140.0;
        *value = (*value + delta).clamp(min, max);
        response.mark_changed();
    }

    if response.double_clicked() {
        // Double-click resets to center (bipolar) or default
        *value = if is_bipolar { 0.0 } else { min };
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let is_dark = ui.visuals().dark_mode;
        let center = Pos2::new(rect.center().x, rect.min.y + size * 0.5);
        let radius = size * 0.42;

        // Background circular well
        let well_bg = if is_dark {
            Color32::from_rgb(0x18, 0x18, 0x18)
        } else {
            Color32::from_rgb(0xb2, 0xb2, 0xb0)
        };
        let well_border = if is_dark {
            Color32::from_rgb(0x38, 0x38, 0x38)
        } else {
            Color32::from_rgb(0x92, 0x92, 0x90)
        };
        painter.circle_filled(center, radius, well_bg);
        painter.circle_stroke(center, radius, Stroke::new(1.0_f32, well_border));

        let start_angle = 0.75 * PI;
        let arc_span = 1.5 * PI;
        let steps = 36;

        // Draw inactive track arc
        let track_color = if is_dark {
            Color32::from_rgb(0x2a, 0x2a, 0x2a)
        } else {
            Color32::from_rgb(0xc4, 0xc4, 0xc2)
        };
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
            painter.line_segment([p1, p2], Stroke::new(1.5_f32, track_color));
        }

        // Draw active illuminated arc
        if is_bipolar {
            // Bipolar arc originates from top-center 12 o'clock (1.5 * PI)
            let center_angle = 1.5 * PI;
            let norm_val = if *value >= 0.0 {
                (*value / max.max(1e-5)).clamp(0.0, 1.0)
            } else {
                (*value / min.abs().max(1e-5)).clamp(-1.0, 0.0)
            };
            let target_angle = center_angle + norm_val * (0.75 * PI);

            let arc_start = center_angle.min(target_angle);
            let arc_end = center_angle.max(target_angle);
            let sub_steps = 18;
            for i in 0..sub_steps {
                let a1 = arc_start + (i as f32 / sub_steps as f32) * (arc_end - arc_start);
                let a2 = arc_start + ((i + 1) as f32 / sub_steps as f32) * (arc_end - arc_start);
                let p1 = Pos2::new(
                    center.x + radius * 0.82 * a1.cos(),
                    center.y + radius * 0.82 * a1.sin(),
                );
                let p2 = Pos2::new(
                    center.x + radius * 0.82 * a2.cos(),
                    center.y + radius * 0.82 * a2.sin(),
                );
                painter.line_segment([p1, p2], Stroke::new(2.0_f32, accent));
            }

            // Top-center 12 o'clock tick marker
            let top_tick_inner = Pos2::new(center.x, center.y - radius * 0.65);
            let top_tick_outer = Pos2::new(center.x, center.y - radius * 0.95);
            painter.line_segment(
                [top_tick_inner, top_tick_outer],
                Stroke::new(
                    1.0_f32,
                    if is_dark {
                        Color32::from_rgb(0x70, 0x70, 0x70)
                    } else {
                        Color32::from_rgb(0x40, 0x40, 0x40)
                    },
                ),
            );

            // Pointer needle
            let needle_x = center.x + radius * 0.75 * target_angle.cos();
            let needle_y = center.y + radius * 0.75 * target_angle.sin();
            let needle_color = if response.hovered() || response.dragged() {
                Color32::WHITE
            } else {
                accent
            };
            painter.line_segment(
                [center, Pos2::new(needle_x, needle_y)],
                Stroke::new(2.0_f32, needle_color),
            );
        } else {
            // Unipolar arc originates from bottom-left (start_angle)
            let norm = ((*value - min) / (max - min).max(1e-5)).clamp(0.0, 1.0);
            let end_angle = start_angle + norm * arc_span;
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
                painter.line_segment([p1, p2], Stroke::new(2.0_f32, accent));
            }

            // Pointer needle
            let needle_x = center.x + radius * 0.75 * end_angle.cos();
            let needle_y = center.y + radius * 0.75 * end_angle.sin();
            let needle_color = if response.hovered() || response.dragged() {
                Color32::WHITE
            } else {
                accent
            };
            painter.line_segment(
                [center, Pos2::new(needle_x, needle_y)],
                Stroke::new(2.0_f32, needle_color),
            );
        }

        // Center hub dot
        painter.circle_filled(
            center,
            2.0,
            if is_dark {
                Color32::from_rgb(0x10, 0x10, 0x10)
            } else {
                Color32::from_rgb(0x80, 0x80, 0x80)
            },
        );

        // Label under knob
        let text_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.min.y + size),
            Vec2::new(rect.width(), 13.0),
        );
        let label_color = if is_dark {
            Color32::from_rgb(0xb0, 0xb0, 0xb0)
        } else {
            Color32::from_rgb(0x18, 0x18, 0x18)
        };
        painter.text(
            text_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            label_color,
        );
    }

    response
}

/// Authentic Ableton Live 12 vertical volume fader with tapered response.
pub fn live_fader(
    ui: &mut Ui,
    value_db: &mut f32,
    _min_db: f32,
    _max_db: f32,
    size: Vec2,
    accent: Color32,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());

    if response.dragged() {
        let current_norm = db_to_norm(*value_db);
        let delta_norm = -response.drag_delta().y / (size.y - 14.0);
        let new_norm = (current_norm + delta_norm).clamp(0.0, 1.0);
        *value_db = norm_to_db(new_norm);
        response.mark_changed();
    }

    if response.double_clicked() {
        // Double-click resets fader to 0.0 dB (Ableton standard)
        *value_db = 0.0;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let is_dark = ui.visuals().dark_mode;

        // Background trough well
        let trough_rect =
            Rect::from_center_size(rect.center(), Vec2::new(4.0, rect.height() - 4.0));
        let trough_color = if is_dark {
            Color32::from_rgb(0x14, 0x14, 0x14)
        } else {
            Color32::from_rgb(0x9a, 0x9a, 0x98)
        };
        let trough_border = if is_dark {
            Color32::from_rgb(0x35, 0x35, 0x35)
        } else {
            Color32::from_rgb(0x82, 0x82, 0x80)
        };
        painter.rect_filled(trough_rect, CornerRadius::same(1), trough_color);
        painter.rect_stroke(
            trough_rect,
            CornerRadius::same(1),
            Stroke::new(1.0_f32, trough_border),
            eframe::egui::StrokeKind::Middle,
        );

        // 0 dB reference line tick across the trough
        let norm_zero = db_to_norm(0.0);
        let zero_y = rect.max.y - 7.0 - norm_zero * (rect.height() - 14.0);
        let tick_color = if is_dark {
            Color32::from_rgb(0x88, 0x88, 0x88)
        } else {
            Color32::from_rgb(0x40, 0x40, 0x40)
        };
        painter.line_segment(
            [
                Pos2::new(rect.min.x + 1.0, zero_y),
                Pos2::new(rect.min.x + 5.0, zero_y),
            ],
            Stroke::new(1.0_f32, tick_color),
        );
        painter.line_segment(
            [
                Pos2::new(rect.max.x - 5.0, zero_y),
                Pos2::new(rect.max.x - 1.0, zero_y),
            ],
            Stroke::new(1.0_f32, tick_color),
        );

        // Fader handle position
        let norm = db_to_norm(*value_db);
        let handle_y = rect.max.y - 7.0 - norm * (rect.height() - 14.0);
        let handle_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, handle_y),
            Vec2::new(size.x * 0.85, 12.0),
        );

        let cap_base = if response.hovered() || response.dragged() {
            if is_dark {
                Color32::from_rgb(0x75, 0x75, 0x75)
            } else {
                Color32::from_rgb(0xee, 0xee, 0xec)
            }
        } else {
            if is_dark {
                Color32::from_rgb(0x5a, 0x5a, 0x5a)
            } else {
                Color32::from_rgb(0xd8, 0xd8, 0xd6)
            }
        };

        // Ableton metallic cap with 1px border
        let cap_border = if is_dark {
            Color32::from_rgb(0x18, 0x18, 0x18)
        } else {
            Color32::from_rgb(0x60, 0x60, 0x60)
        };
        painter.rect_filled(handle_rect, CornerRadius::same(2), cap_base);
        painter.rect_stroke(
            handle_rect,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, cap_border),
            eframe::egui::StrokeKind::Middle,
        );

        // Horizontal center groove line
        let groove_color = if response.dragged() {
            accent
        } else if is_dark {
            Color32::from_rgb(0x22, 0x22, 0x22)
        } else {
            Color32::from_rgb(0x30, 0x30, 0x30)
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
        let is_dark = ui.visuals().dark_mode;

        // Meter well background
        let well_bg = if is_dark {
            Color32::from_rgb(0x12, 0x12, 0x12)
        } else {
            Color32::from_rgb(0x1a, 0x1a, 0x1a)
        };
        let well_border = if is_dark {
            Color32::from_rgb(0x22, 0x22, 0x22)
        } else {
            Color32::from_rgb(0x55, 0x55, 0x55)
        };
        painter.rect_filled(rect, CornerRadius::same(1), well_bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(1),
            Stroke::new(1.0_f32, well_border),
            eframe::egui::StrokeKind::Middle,
        );

        let clip_box_h = 4.0;
        let active_meter_h = rect.height() - clip_box_h - 3.0;
        let half_w = (rect.width() - 3.0) * 0.5;

        let draw_channel = |p_db: f32, x_offset: f32| {
            let norm = db_to_norm(p_db);
            let channel_x = rect.min.x + x_offset;
            let meter_bottom = rect.max.y - 1.0;

            // Draw segmented gradient levels
            let segments = 24;
            let seg_h = active_meter_h / segments as f32;
            let filled_segs = (segments as f32 * norm).round() as usize;

            for s in 0..filled_segs {
                let seg_norm = s as f32 / segments as f32;
                // db_to_norm(0.0) is 0.80, db_to_norm(-12.0) is 0.50
                let seg_color = if seg_norm >= 0.80 {
                    Color32::from_rgb(0xff, 0x45, 0x45) // Red peak clip
                } else if seg_norm >= 0.50 {
                    Color32::from_rgb(0xff, 0xd0, 0x36) // Yellow warn
                } else {
                    Color32::from_rgb(0x28, 0xe0, 0x58) // Green level
                };

                let y_top = meter_bottom - (s + 1) as f32 * seg_h;
                let seg_rect = Rect::from_min_size(
                    Pos2::new(channel_x, y_top + 0.5),
                    Vec2::new(half_w, seg_h - 1.0),
                );
                painter.rect_filled(seg_rect, CornerRadius::ZERO, seg_color);
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
            painter.rect_filled(clip_rect, CornerRadius::same(1), clip_color);
        };

        draw_channel(peak_l_db, 1.0);
        draw_channel(peak_r_db, 2.0 + half_w);
    }
}

/// Authentic Ableton Live 12 LCD readout box with recessed frame.
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
        let is_dark = ui.visuals().dark_mode;

        let bg = if is_dark {
            Color32::from_rgb(0x14, 0x14, 0x14)
        } else {
            Color32::from_rgb(0x20, 0x20, 0x20)
        };
        let border = if highlight || response.hovered() {
            accent
        } else if is_dark {
            Color32::from_rgb(0x38, 0x38, 0x38)
        } else {
            Color32::from_rgb(0x55, 0x55, 0x55)
        };

        painter.rect_filled(rect, CornerRadius::same(2), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, border),
            eframe::egui::StrokeKind::Middle,
        );

        let text_color = if highlight {
            accent
        } else if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xdd, 0xdd, 0xdd)
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

/// Authentic Ableton Live 12 toggle button with illumination state and full theme support.
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
        let is_dark = ui.visuals().dark_mode;

        let (bg, border, fg) = if is_active {
            let text_col =
                if active_color.r() > 180 && active_color.g() > 180 && active_color.b() > 180 {
                    Color32::BLACK
                } else if active_color.r() < 100 && active_color.g() < 100 && active_color.b() > 160
                {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                };
            (active_color, active_color, text_col)
        } else if response.hovered() {
            if is_dark {
                (
                    Color32::from_rgb(0x40, 0x40, 0x40),
                    Color32::from_rgb(0x55, 0x55, 0x55),
                    Color32::from_rgb(0xee, 0xee, 0xee),
                )
            } else {
                (
                    Color32::from_rgb(0xdb, 0xdb, 0xd9),
                    Color32::from_rgb(0xb0, 0xb0, 0xae),
                    Color32::from_rgb(0x05, 0x05, 0x05),
                )
            }
        } else {
            if is_dark {
                (
                    Color32::from_rgb(0x2d, 0x2d, 0x2d),
                    Color32::from_rgb(0x38, 0x38, 0x38),
                    Color32::from_rgb(0xa0, 0xa0, 0xa0),
                )
            } else {
                (
                    Color32::from_rgb(0xcc, 0xcc, 0xca),
                    Color32::from_rgb(0x9c, 0x9c, 0x9a),
                    Color32::from_rgb(0x18, 0x18, 0x18),
                )
            }
        };

        painter.rect_filled(rect, CornerRadius::same(2), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, border),
            eframe::egui::StrokeKind::Middle,
        );
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
        let is_dark = ui.visuals().dark_mode;
        let x = rect.center().x;
        let line_color = if is_dark {
            Color32::from_rgb(0x3a, 0x3a, 0x3a)
        } else {
            Color32::from_rgb(0x9a, 0x9a, 0x98)
        };
        painter.line_segment(
            [
                Pos2::new(x, rect.min.y + 3.0),
                Pos2::new(x, rect.max.y - 3.0),
            ],
            Stroke::new(1.0_f32, line_color),
        );
    }
}

/// Dedicated segmented capsule toggle for Session (`|||`) and Arrangement (`☰`) views.
pub fn live_view_mode_toggle(ui: &mut Ui, is_session: bool, accent: Color32) -> (bool, bool) {
    let total_size = Vec2::new(52.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(total_size, Sense::hover());

    let mut session_clicked = false;
    let mut arrange_clicked = false;

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let is_dark = ui.visuals().dark_mode;

        // Container frame
        let frame_bg = if is_dark {
            Color32::from_rgb(0x18, 0x18, 0x18)
        } else {
            Color32::from_rgb(0x9a, 0x9a, 0x98)
        };
        let frame_stroke = if is_dark {
            Color32::from_rgb(0x35, 0x35, 0x35)
        } else {
            Color32::from_rgb(0x80, 0x80, 0x7e)
        };
        painter.rect_filled(rect, CornerRadius::same(2), frame_bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, frame_stroke),
            eframe::egui::StrokeKind::Middle,
        );

        let half_w = rect.width() * 0.5;
        let sess_rect = Rect::from_min_size(rect.min, Vec2::new(half_w, rect.height()));
        let arr_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + half_w, rect.min.y),
            Vec2::new(half_w, rect.height()),
        );

        let sess_resp = ui.interact(sess_rect, ui.scope_id().with("sess_toggle"), Sense::click());
        let arr_resp = ui.interact(arr_rect, ui.scope_id().with("arr_toggle"), Sense::click());

        if sess_resp.clicked() {
            session_clicked = true;
        }
        if arr_resp.clicked() {
            arrange_clicked = true;
        }

        // Session half (left): 3 vertical bars |||
        let sess_active_bg = if is_session {
            if is_dark {
                Color32::from_rgb(0x2e, 0x2e, 0x2e)
            } else {
                Color32::from_rgb(0xbf, 0xbf, 0xbd)
            }
        } else if sess_resp.hovered() {
            if is_dark {
                Color32::from_rgb(0x24, 0x24, 0x24)
            } else {
                Color32::from_rgb(0xaa, 0xaa, 0xa8)
            }
        } else {
            frame_bg
        };
        painter.rect_filled(sess_rect.shrink(1.0), CornerRadius::same(1), sess_active_bg);

        let sess_icon_col = if is_session {
            accent
        } else if is_dark {
            Color32::from_rgb(0x75, 0x75, 0x75)
        } else {
            Color32::from_rgb(0x40, 0x40, 0x40)
        };
        let c_s = sess_rect.center();
        let bar_h = 10.0;
        painter.line_segment(
            [
                Pos2::new(c_s.x - 4.0, c_s.y - bar_h * 0.5),
                Pos2::new(c_s.x - 4.0, c_s.y + bar_h * 0.5),
            ],
            Stroke::new(1.8_f32, sess_icon_col),
        );
        painter.line_segment(
            [
                Pos2::new(c_s.x, c_s.y - bar_h * 0.5),
                Pos2::new(c_s.x, c_s.y + bar_h * 0.5),
            ],
            Stroke::new(1.8_f32, sess_icon_col),
        );
        painter.line_segment(
            [
                Pos2::new(c_s.x + 4.0, c_s.y - bar_h * 0.5),
                Pos2::new(c_s.x + 4.0, c_s.y + bar_h * 0.5),
            ],
            Stroke::new(1.8_f32, sess_icon_col),
        );

        // Arrangement half (right): 3 horizontal bars ☰
        let arr_active_bg = if !is_session {
            if is_dark {
                Color32::from_rgb(0x2e, 0x2e, 0x2e)
            } else {
                Color32::from_rgb(0xbf, 0xbf, 0xbd)
            }
        } else if arr_resp.hovered() {
            if is_dark {
                Color32::from_rgb(0x24, 0x24, 0x24)
            } else {
                Color32::from_rgb(0xaa, 0xaa, 0xa8)
            }
        } else {
            frame_bg
        };
        painter.rect_filled(arr_rect.shrink(1.0), CornerRadius::same(1), arr_active_bg);

        let arr_icon_col = if !is_session {
            accent
        } else if is_dark {
            Color32::from_rgb(0x75, 0x75, 0x75)
        } else {
            Color32::from_rgb(0x40, 0x40, 0x40)
        };
        let c_a = arr_rect.center();
        let bar_w = 11.0;
        painter.line_segment(
            [
                Pos2::new(c_a.x - bar_w * 0.5, c_a.y - 4.0),
                Pos2::new(c_a.x + bar_w * 0.5, c_a.y - 4.0),
            ],
            Stroke::new(1.8_f32, arr_icon_col),
        );
        painter.line_segment(
            [
                Pos2::new(c_a.x - bar_w * 0.5, c_a.y),
                Pos2::new(c_a.x + bar_w * 0.5, c_a.y),
            ],
            Stroke::new(1.8_f32, arr_icon_col),
        );
        painter.line_segment(
            [
                Pos2::new(c_a.x - bar_w * 0.5, c_a.y + 4.0),
                Pos2::new(c_a.x + bar_w * 0.5, c_a.y + 4.0),
            ],
            Stroke::new(1.8_f32, arr_icon_col),
        );
    }

    (session_clicked, arrange_clicked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_norm_roundtrip() {
        let test_points = [-60.0, -48.0, -24.0, -18.0, -12.0, -6.0, 0.0, 3.0, 6.0];
        for &db in &test_points {
            let norm = db_to_norm(db);
            assert!((0.0..=1.0).contains(&norm), "norm out of range for db {db}");
            let roundtrip = norm_to_db(norm);
            assert!(
                (roundtrip - db).abs() < 0.15,
                "Roundtrip mismatch: db={db}, norm={norm}, roundtrip={roundtrip}"
            );
        }
    }

    #[test]
    fn test_db_norm_boundaries() {
        assert_eq!(db_to_norm(0.0), 0.80);
        assert_eq!(norm_to_db(0.80), 0.0);
        assert_eq!(db_to_norm(6.0), 1.0);
        assert_eq!(norm_to_db(1.0), 6.0);
        assert_eq!(db_to_norm(-60.0), 0.0);
        assert_eq!(norm_to_db(0.0), -60.0);
        assert_eq!(db_to_norm(-100.0), 0.0);
        assert_eq!(norm_to_db(1.5), 6.0);
    }
}
