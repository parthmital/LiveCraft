use eframe::egui::{self, Color32, Rounding, Stroke, Vec2, Visuals};
use livecraft_core::theme::AbletonTheme;

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
}

impl Default for LivePalette {
    fn default() -> Self {
        let theme = AbletonTheme::default();
        let c = |key: &str| -> Color32 {
            let [r, g, b, a] = theme.parse_color(key);
            Color32::from_rgba_unmultiplied(r, g, b, a)
        };

        Self {
            desktop: c("Desktop"),
            surface_area: c("SurfaceArea"),
            surface_highlight: c("SurfaceHighlight"),
            detail_background: c("DetailViewBackground"),
            control_background: c("ControlBackground"),
            control_foreground: c("ControlForeground"),
            control_disabled: c("ControlDisabled"),
            meter_background: c("MeterBackground"),
            amber: c("ViewCheckControlEnabledOn"),
            record_red: c("ChosenRecord"),
            solo_blue: c("ChosenPreListen"),
            cyan: c("RangeDefault"),
            meter_green: c("VolumePeak"),
            meter_yellow: c("VolumeWarn"),
            meter_red: c("VolumeClip"),
            play_green: Color32::from_rgb(0x40, 0xff, 0x63),
        }
    }
}

pub fn apply_live_style(ctx: &egui::Context, palette: &LivePalette) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(palette.control_foreground);
    visuals.panel_fill = palette.surface_area;
    visuals.window_fill = palette.desktop;
    visuals.faint_bg_color = palette.desktop;
    visuals.extreme_bg_color = palette.control_background;

    // Ableton's crisp flat buttons
    visuals.widgets.noninteractive.bg_fill = palette.surface_area;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.surface_highlight);
    visuals.widgets.noninteractive.rounding = Rounding::same(2.0);

    visuals.widgets.inactive.bg_fill = palette.surface_highlight;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x4a, 0x4a, 0x4a));
    visuals.widgets.inactive.rounding = Rounding::same(2.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x52, 0x52, 0x52);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.amber);
    visuals.widgets.hovered.rounding = Rounding::same(2.0);

    visuals.widgets.active.bg_fill = palette.amber;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::BLACK);
    visuals.widgets.active.rounding = Rounding::same(2.0);

    visuals.selection.bg_fill = palette.amber;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.amber);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(4.0, 4.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    ctx.set_style(style);
}
