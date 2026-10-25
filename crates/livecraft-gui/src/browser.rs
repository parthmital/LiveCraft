use crate::style::LivePalette;
use eframe::egui::{self, Color32, Pos2, Rounding, Sense, Ui, Vec2};
use livecraft_core::model::{Device, DeviceType, Track, TrackType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCategory {
    Collections(usize), // 0..7 colored dots
    Sounds,
    Drums,
    Instruments,
    AudioEffects,
    MidiEffects,
    Plugins,
    Clips,
    Samples,
    Packs,
    UserLibrary,
    CurrentProject,
}

pub struct BrowserAction {
    pub add_device: Option<Device>,
    pub add_track: Option<TrackType>,
}

pub struct LiveBrowser {
    pub selected_category: BrowserCategory,
    pub search_query: String,
}

impl Default for LiveBrowser {
    fn default() -> Self {
        Self {
            selected_category: BrowserCategory::Instruments,
            search_query: String::new(),
        }
    }
}

impl LiveBrowser {
    pub fn render(
        &mut self,
        ui: &mut Ui,
        _selected_track: Option<&mut Track>,
        palette: &LivePalette,
    ) -> BrowserAction {
        let mut action = BrowserAction {
            add_device: None,
            add_track: None,
        };

        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

            // 1. Search Bar (Manual p. 65)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                let w = ui.available_width()
                    - if self.search_query.is_empty() {
                        0.0
                    } else {
                        22.0
                    };
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search (Ctrl+F)")
                        .desired_width(w),
                );
                if !self.search_query.is_empty() && ui.small_button("x").clicked() {
                    self.search_query.clear();
                }
            });

            ui.add_space(3.0);

            // 2. Collections (Manual p. 81)
            ui.label(
                egui::RichText::new("COLLECTIONS")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let collections = [
                (Color32::from_rgb(0xff, 0x4f, 0x4f), "Favorites"),
                (Color32::from_rgb(0xff, 0x9f, 0x36), "Synths"),
                (Color32::from_rgb(0xff, 0xd8, 0x3d), "Drums"),
                (Color32::from_rgb(0x38, 0xd8, 0x55), "Bass"),
                (Color32::from_rgb(0x2f, 0x77, 0xee), "FX"),
                (Color32::from_rgb(0xaa, 0x55, 0xff), "Templates"),
                (Color32::from_rgb(0x88, 0x88, 0x88), "Other"),
            ];

            for (idx, (dot_color, label)) in collections.iter().enumerate() {
                let is_sel = self.selected_category == BrowserCategory::Collections(idx);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.button_hover);
                    }
                    // Color dot
                    painter.circle_filled(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        3.5,
                        *dot_color,
                    );
                    painter.text(
                        Pos2::new(rect.min.x + 20.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        *label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = BrowserCategory::Collections(idx);
                }
            }

            ui.add_space(4.0);

            // 3. Categories (Manual p. 64)
            ui.label(
                egui::RichText::new("CATEGORIES")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let categories = [
                (BrowserCategory::Sounds, "Sounds"),
                (BrowserCategory::Drums, "Drums"),
                (BrowserCategory::Instruments, "Instruments"),
                (BrowserCategory::AudioEffects, "Audio Effects"),
                (BrowserCategory::MidiEffects, "MIDI Effects"),
                (BrowserCategory::Plugins, "Plug-Ins"),
                (BrowserCategory::Clips, "Clips"),
                (BrowserCategory::Samples, "Samples"),
            ];

            for (cat, label) in categories {
                let is_sel = self.selected_category == cat;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.button_hover);
                    }
                    painter.text(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = cat;
                }
            }

            ui.add_space(4.0);

            // 4. Places (Manual p. 85)
            ui.label(
                egui::RichText::new("PLACES")
                    .size(9.0)
                    .strong()
                    .color(palette.control_disabled),
            );

            let places = [
                (BrowserCategory::Packs, "Packs"),
                (BrowserCategory::UserLibrary, "User Library"),
                (BrowserCategory::CurrentProject, "Current Project"),
            ];

            for (cat, label) in places {
                let is_sel = self.selected_category == cat;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_sel {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.surface_highlight);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, Rounding::same(2.0), palette.button_hover);
                    }
                    painter.text(
                        Pos2::new(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(10.5),
                        if is_sel {
                            if palette.is_dark() {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            }
                        } else {
                            palette.control_foreground
                        },
                    );
                }
                if resp.clicked() {
                    self.selected_category = cat;
                }
            }

            ui.add_space(4.0);
            ui.separator();

            // 5. Category Content List (scrollable)
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.selected_category {
                    BrowserCategory::Instruments => {
                        let instruments = [
                            (
                                "Operator",
                                DeviceType::Operator,
                                "4-Osc FM & Subtractive Synthesizer",
                            ),
                            (
                                "Simpler",
                                DeviceType::Simpler,
                                "Polyphonic Sample Instrument",
                            ),
                            (
                                "Drum Rack",
                                DeviceType::DrumRack,
                                "16-Pad Drum Sampler & Synth",
                            ),
                            (
                                "Analog",
                                DeviceType::Operator,
                                "Vintage Analog Modeling Synth",
                            ),
                            (
                                "Tension",
                                DeviceType::Operator,
                                "Physical Modeling String Synth",
                            ),
                            (
                                "Electric",
                                DeviceType::Operator,
                                "Classic Electric Piano Modeling",
                            ),
                        ];
                        for (name, dtype, desc) in instruments {
                            if !self.search_query.is_empty()
                                && !name
                                    .to_lowercase()
                                    .contains(&self.search_query.to_lowercase())
                            {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(false, name)
                                    .on_hover_text(desc)
                                    .clicked()
                                {
                                    action.add_device =
                                        Some(Device::new(1001, name, dtype.clone()));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(Device::new(1001, name, dtype.clone()));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::AudioEffects => {
                        let fx = [
                            ("Chorus-Ensemble", DeviceType::Chorus),
                            ("Compressor", DeviceType::Compressor),
                            ("Delay", DeviceType::Delay),
                            ("EQ Eight", DeviceType::EqEight),
                            ("Reverb", DeviceType::Reverb),
                            ("Saturator", DeviceType::Saturator),
                            ("Utility", DeviceType::Utility),
                            ("Auto Filter", DeviceType::AutoFilter),
                            ("Auto Pan", DeviceType::AutoPan),
                            ("Gate", DeviceType::Gate),
                            ("Redux", DeviceType::Redux),
                        ];
                        for (name, dtype) in fx {
                            if !self.search_query.is_empty()
                                && !name
                                    .to_lowercase()
                                    .contains(&self.search_query.to_lowercase())
                            {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, name).clicked() {
                                    action.add_device =
                                        Some(Device::new(1002, name, dtype.clone()));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(Device::new(1002, name, dtype.clone()));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::Drums => {
                        let kits = [
                            "808 Core Kit",
                            "909 Vintage Kit",
                            "Acoustic Studio Kit",
                            "Electronic Breakbeat Kit",
                            "Trap 808 Essentials",
                        ];
                        for kit in kits {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, kit).clicked() {
                                    action.add_device =
                                        Some(Device::new(1003, kit, DeviceType::DrumRack));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("+ Add").clicked() {
                                            action.add_device =
                                                Some(Device::new(1003, kit, DeviceType::DrumRack));
                                        }
                                    },
                                );
                            });
                        }
                    }
                    BrowserCategory::Samples => {
                        let samples = [
                            "808 Kick Drum.wav",
                            "Crisp Snare 01.wav",
                            "Hi-Hat Closed 16th.wav",
                            "Synth Lead C4.wav",
                            "Sub Bass 40Hz.wav",
                            "Amen Break 174bpm.wav",
                        ];
                        for s in samples {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(s)
                                        .size(10.0)
                                        .color(palette.control_foreground),
                                );
                            });
                        }
                    }
                    _ => {
                        let defaults = [
                            ("Operator Preset 1", DeviceType::Operator),
                            ("Ambient Reverb", DeviceType::Reverb),
                            ("Stereo Delay Echo", DeviceType::Delay),
                        ];
                        for (name, dtype) in defaults {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(false, name).clicked() {
                                    action.add_device = Some(Device::new(1004, name, dtype));
                                }
                            });
                        }
                    }
                });
        });

        action
    }
}
