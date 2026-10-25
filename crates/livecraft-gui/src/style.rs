use eframe::egui::{self, Color32, Margin, Rounding, Stroke, Vec2, Visuals};

#[derive(Debug, Clone)]
pub struct LivePalette {
    pub desktop: Color32,
    pub surface_area: Color32,
    pub surface_highlight: Color32,
    pub detail_background: Color32,
    pub control_background: Color32,
    pub control_foreground: Color32,
    pub control_disabled: Color32,
    pub meter_background: Color32,
    pub amber: Color32,
    pub record_red: Color32,
    pub solo_blue: Color32,
    pub cyan: Color32,
    pub meter_green: Color32,
    pub meter_yellow: Color32,
    pub meter_red: Color32,
    pub play_green: Color32,
    pub panel_border: Color32,
    pub lcd_bg: Color32,
    pub lcd_text: Color32,
    pub button_idle: Color32,
    pub button_hover: Color32,
    pub grid_line: Color32,
    pub grid_subdivision: Color32,
}

impl Default for LivePalette {
    fn default() -> Self {
        Self::dark()
    }
}

impl LivePalette {
    /// Authentic Ableton Live 12 Dark Neutral theme
    pub fn dark() -> Self {
        Self {
            desktop: Color32::from_rgb(0x1a, 0x1a, 0x1a),
            surface_area: Color32::from_rgb(0x28, 0x28, 0x28),
            surface_highlight: Color32::from_rgb(0x38, 0x38, 0x38),
            detail_background: Color32::from_rgb(0x2e, 0x2e, 0x2e),
            control_background: Color32::from_rgb(0x1a, 0x1a, 0x1a),
            control_foreground: Color32::from_rgb(0xc8, 0xc8, 0xc8),
            control_disabled: Color32::from_rgb(0x5a, 0x5a, 0x5a),
            meter_background: Color32::from_rgb(0x10, 0x10, 0x10),
            amber: Color32::from_rgb(0xff, 0xad, 0x56), // Ableton Live Signature Amber
            record_red: Color32::from_rgb(0xff, 0x4f, 0x4f), // Ableton Record Red
            solo_blue: Color32::from_rgb(0x2f, 0x77, 0xee), // Ableton Solo Blue
            cyan: Color32::from_rgb(0x00, 0xca, 0xde),  // Ableton Cyan
            meter_green: Color32::from_rgb(0x28, 0xe0, 0x58), // VolumePeak
            meter_yellow: Color32::from_rgb(0xff, 0xd0, 0x36), // VolumeWarn
            meter_red: Color32::from_rgb(0xff, 0x45, 0x45), // VolumeClip
            play_green: Color32::from_rgb(0x2c, 0xe5, 0x62), // Live Play Green
            panel_border: Color32::from_rgb(0x35, 0x35, 0x35),
            lcd_bg: Color32::from_rgb(0x14, 0x14, 0x14),
            lcd_text: Color32::from_rgb(0xdd, 0xdd, 0xdd),
            button_idle: Color32::from_rgb(0x32, 0x32, 0x32),
            button_hover: Color32::from_rgb(0x42, 0x42, 0x42),
            grid_line: Color32::from_rgb(0x33, 0x33, 0x33),
            grid_subdivision: Color32::from_rgb(0x26, 0x26, 0x26),
        }
    }

    /// Authentic Ableton Live 12 Light theme (matches the reference manual printed screenshots)
    pub fn light() -> Self {
        Self {
            desktop: Color32::from_rgb(0x9e, 0x9e, 0x9c),
            surface_area: Color32::from_rgb(0xbf, 0xbf, 0xbd),
            surface_highlight: Color32::from_rgb(0xd4, 0xd4, 0xd2),
            detail_background: Color32::from_rgb(0xc8, 0xc8, 0xc6),
            control_background: Color32::from_rgb(0xa8, 0xa8, 0xa6),
            control_foreground: Color32::from_rgb(0x18, 0x18, 0x18),
            control_disabled: Color32::from_rgb(0x6e, 0x6e, 0x6c),
            meter_background: Color32::from_rgb(0x18, 0x18, 0x18),
            amber: Color32::from_rgb(0xff, 0x98, 0x00),
            record_red: Color32::from_rgb(0xff, 0x3b, 0x3b),
            solo_blue: Color32::from_rgb(0x26, 0x68, 0xde),
            cyan: Color32::from_rgb(0x00, 0xb0, 0xc8),
            meter_green: Color32::from_rgb(0x20, 0xd0, 0x50),
            meter_yellow: Color32::from_rgb(0xf0, 0xc0, 0x20),
            meter_red: Color32::from_rgb(0xff, 0x38, 0x38),
            play_green: Color32::from_rgb(0x1e, 0xd4, 0x50),
            panel_border: Color32::from_rgb(0x8e, 0x8e, 0x8c),
            lcd_bg: Color32::from_rgb(0x22, 0x22, 0x22),
            lcd_text: Color32::from_rgb(0xee, 0xee, 0xee),
            button_idle: Color32::from_rgb(0xcc, 0xcc, 0xca),
            button_hover: Color32::from_rgb(0xdb, 0xdb, 0xd9),
            grid_line: Color32::from_rgb(0xa6, 0xa6, 0xa4),
            grid_subdivision: Color32::from_rgb(0xb2, 0xb2, 0xb0),
        }
    }
}

pub fn apply_live_style(ctx: &egui::Context, palette: &LivePalette) {
    let mut visuals = if palette.control_foreground.r() < 100 {
        Visuals::light()
    } else {
        Visuals::dark()
    };

    visuals.override_text_color = Some(palette.control_foreground);
    visuals.panel_fill = palette.surface_area;
    visuals.window_fill = palette.desktop;
    visuals.faint_bg_color = palette.desktop;
    visuals.extreme_bg_color = palette.lcd_bg;

    // Ableton Live 12's signature crisp flat widgets
    visuals.widgets.noninteractive.bg_fill = palette.surface_area;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.noninteractive.rounding = Rounding::same(2.0);

    visuals.widgets.inactive.bg_fill = palette.button_idle;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.inactive.rounding = Rounding::same(2.0);

    visuals.widgets.hovered.bg_fill = palette.button_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.amber);
    visuals.widgets.hovered.rounding = Rounding::same(2.0);

    visuals.widgets.active.bg_fill = palette.amber;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::BLACK);
    visuals.widgets.active.rounding = Rounding::same(2.0);

    visuals.widgets.open.bg_fill = palette.surface_highlight;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.amber);
    visuals.widgets.open.rounding = Rounding::same(2.0);

    visuals.selection.bg_fill = palette.amber;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.amber);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(3.0, 3.0);
    style.spacing.button_padding = Vec2::new(5.0, 2.0);
    style.spacing.window_margin = Margin::same(2.0);
    ctx.set_style(style);
}
