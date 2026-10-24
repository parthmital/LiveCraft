use crate::style::LivePalette;
use eframe::egui::{self, Color32, Ui};
use livecraft_core::model::{Device, DeviceType, Track, TrackType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCategory {
    Instruments,
    AudioEffects,
    Samples,
    Places,
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
            ui.set_width(170.0);

            // Search Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍").size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search...")
                        .desired_width(130.0),
                );
            });

            ui.separator();

            // Categories Section
            ui.label(
                egui::RichText::new("CATEGORIES")
                    .size(9.0)
                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
            );

            let mut category_btn = |cat: BrowserCategory, label: &str, icon: &str| {
                let is_sel = self.selected_category == cat;
                let text = format!("{} {}", icon, label);
                if ui.selectable_label(is_sel, text).clicked() {
                    self.selected_category = cat;
                }
            };

            category_btn(BrowserCategory::Instruments, "Instruments", "🎹");
            category_btn(BrowserCategory::AudioEffects, "Audio Effects", "🎛️");
            category_btn(BrowserCategory::Samples, "Samples", "🌊");
            category_btn(BrowserCategory::Places, "Places", "📁");

            ui.separator();

            // Content List based on selected category
            egui::ScrollArea::vertical().show(ui, |ui| match self.selected_category {
                BrowserCategory::Instruments => {
                    let instruments = [
                        (
                            "Operator",
                            DeviceType::Operator,
                            "4-Osc FM & Subtractive Synth",
                        ),
                        ("Simpler", DeviceType::Simpler, "Polyphonic Sample Player"),
                        (
                            "Drum Rack",
                            DeviceType::DrumRack,
                            "16-Pad Sample & Synth Drum Machine",
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

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(name)
                                        .strong()
                                        .size(11.0)
                                        .color(palette.amber),
                                );
                                if ui.button("+ Add").clicked() {
                                    let dev = Device::new(rand_id(), name, dtype);
                                    action.add_device = Some(dev);
                                }
                            });
                            ui.label(
                                egui::RichText::new(desc)
                                    .size(9.0)
                                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                            );
                        });
                        ui.add_space(2.0);
                    }
                }
                BrowserCategory::AudioEffects => {
                    let effects = [
                        ("EQ Eight", DeviceType::EqEight, "8-band parametric EQ"),
                        (
                            "Compressor",
                            DeviceType::Compressor,
                            "Dynamic range compressor",
                        ),
                        (
                            "Saturator",
                            DeviceType::Saturator,
                            "Harmonic drive & waveshaping",
                        ),
                        ("Delay", DeviceType::Delay, "Stereo ping-pong delay"),
                        ("Reverb", DeviceType::Reverb, "Algorithmic room reverb"),
                        ("Limiter", DeviceType::Limiter, "Peak brickwall limiter"),
                        ("Utility", DeviceType::Utility, "Gain, stereo width & pan"),
                        (
                            "Auto Filter",
                            DeviceType::AutoFilter,
                            "Multi-mode resonant filter with LFO",
                        ),
                        (
                            "Chorus-Ensemble",
                            DeviceType::Chorus,
                            "Stereo modulated delay ensemble",
                        ),
                        (
                            "Auto Pan-Tremolo",
                            DeviceType::AutoPan,
                            "LFO stereo panner & tremolo",
                        ),
                        (
                            "Gate",
                            DeviceType::Gate,
                            "Noise gate with hysteresis & hold",
                        ),
                        ("EQ Three", DeviceType::EqThree, "3-band DJ isolator EQ"),
                        ("Redux", DeviceType::Redux, "Sample rate & bit crusher"),
                    ];

                    for (name, dtype, desc) in effects {
                        if !self.search_query.is_empty()
                            && !name
                                .to_lowercase()
                                .contains(&self.search_query.to_lowercase())
                        {
                            continue;
                        }

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(name)
                                        .strong()
                                        .size(11.0)
                                        .color(palette.cyan),
                                );
                                if ui.button("+ Add").clicked() {
                                    let dev = Device::new(rand_id(), name, dtype);
                                    action.add_device = Some(dev);
                                }
                            });
                            ui.label(
                                egui::RichText::new(desc)
                                    .size(9.0)
                                    .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                            );
                        });
                        ui.add_space(2.0);
                    }
                }
                BrowserCategory::Samples => {
                    ui.label(egui::RichText::new("Core Sample Library").size(10.0));
                    ui.label(egui::RichText::new("• 808_Kick.wav").size(10.0));
                    ui.label(egui::RichText::new("• 909_Snare.wav").size(10.0));
                    ui.label(egui::RichText::new("• Closed_HiHat.wav").size(10.0));
                    ui.label(egui::RichText::new("• Open_HiHat.wav").size(10.0));
                }
                BrowserCategory::Places => {
                    ui.label(egui::RichText::new("Places:").size(10.0));
                    ui.label("• Core Library");
                    ui.label("• User Library");
                    ui.label("• Current Project");
                }
            });

            ui.add_space(8.0);
            ui.separator();

            // Track Addition shortcuts
            ui.horizontal(|ui| {
                if ui.button("+ MIDI").clicked() {
                    action.add_track = Some(TrackType::Midi);
                }
                if ui.button("+ Audio").clicked() {
                    action.add_track = Some(TrackType::Audio);
                }
            });
        });

        action
    }
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
