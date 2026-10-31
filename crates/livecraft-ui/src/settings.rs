//! The Settings window with its Audio and MIDI tabs (M06 item 9, manual 2.3, pages 31 to 34).
//!
//! The window is functional: it lists the audio hosts, devices, sample rates, and buffer sizes the
//! machine offers, shows the latency of the running output, and switches the MIDI ports. Visual parity
//! with Live comes in M40. The choices are worked out by the pure functions below, which the tests
//! cover; the window only draws them and hands changes to the session.

use eframe::egui::{self, Color32};
use livecraft_session::{
    AudioPreferences, DeviceInfo, DeviceList, HostInfo, MidiPortPreference, MidiPorts,
    MidiPreferences, Session,
};

/// The buffer sizes offered, in frames (TARGET-ARCHITECTURE: 32 to 2048).
const BUFFER_SIZES: [u32; 7] = [32, 64, 128, 256, 512, 1024, 2048];

/// The tabs of the Settings window that M06 implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Audio,
    Midi,
}

/// The Settings window. The device and port lists are read when the window opens and when Rescan is
/// pressed, since listing devices is slow.
#[derive(Default)]
pub struct SettingsWindow {
    pub open: bool,
    pub tab: SettingsTab,
    devices: Option<DeviceList>,
    ports: Option<MidiPorts>,
}

/// What the Audio tab offers for the current preferences. `None` in a preference is the system
/// default, shown first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioChoices {
    pub driver_types: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub sample_rates: Vec<u32>,
    pub buffer_sizes: Vec<u32>,
}

/// One row of the MIDI Ports table: a port with its switches, and whether it is connected now.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiRow {
    pub port: MidiPortPreference,
    pub connected: bool,
}

/// The host the preferences name, or the default host, or the first.
fn chosen_host<'a>(devices: &'a DeviceList, audio: &AudioPreferences) -> Option<&'a HostInfo> {
    let named = audio
        .driver_type
        .as_ref()
        .and_then(|name| devices.hosts.iter().find(|host| &host.name == name));
    named
        .or_else(|| devices.hosts.iter().find(|host| host.is_default))
        .or_else(|| devices.hosts.first())
}

/// The device the preferences name among `devices`, or the default device, or the first.
fn chosen_device<'a>(devices: &'a [DeviceInfo], name: Option<&String>) -> Option<&'a DeviceInfo> {
    name.and_then(|name| devices.iter().find(|device| &device.name == name))
        .or_else(|| devices.iter().find(|device| device.is_default))
        .or_else(|| devices.first())
}

/// The choices of the Audio tab: the hosts, the devices of the chosen host, and the sample rates and
/// buffer sizes of the chosen output device. A device that reports no buffer range offers every size.
pub fn audio_choices(devices: &DeviceList, audio: &AudioPreferences) -> AudioChoices {
    let host = chosen_host(devices, audio);
    let names = |list: &[DeviceInfo]| list.iter().map(|device| device.name.clone()).collect();
    let output = host.and_then(|host| chosen_device(&host.outputs, audio.output_device.as_ref()));
    let buffer_sizes = BUFFER_SIZES
        .into_iter()
        .filter(|size| {
            output
                .and_then(|device| device.buffer_frames)
                .is_none_or(|(low, high)| (low..=high).contains(size))
        })
        .collect();
    AudioChoices {
        driver_types: devices.hosts.iter().map(|host| host.name.clone()).collect(),
        inputs: host.map_or_else(Vec::new, |host| names(&host.inputs)),
        outputs: host.map_or_else(Vec::new, |host| names(&host.outputs)),
        sample_rates: output.map_or_else(Vec::new, |device| device.sample_rates.clone()),
        buffer_sizes,
    }
}

/// The rows of the MIDI Ports table for one direction: every connected port with its saved switches,
/// then the saved ports that are not connected. A connected port with no saved row takes
/// `default_track` for its Track switch, which matches what the session opens without a preference.
pub fn midi_rows(
    available: &[String],
    saved: &[MidiPortPreference],
    default_track: bool,
) -> Vec<MidiRow> {
    let connected = available.iter().map(|name| MidiRow {
        port: saved
            .iter()
            .find(|port| &port.name == name)
            .cloned()
            .unwrap_or_else(|| MidiPortPreference {
                name: name.clone(),
                track: default_track,
                ..MidiPortPreference::default()
            }),
        connected: true,
    });
    let missing = saved
        .iter()
        .filter(|port| !available.contains(&port.name))
        .map(|port| MidiRow {
            port: port.clone(),
            connected: false,
        });
    connected.chain(missing).collect()
}

/// A latency in milliseconds, as the Audio tab shows it.
pub fn latency_text(frames: u32, sample_rate: u32) -> String {
    if sample_rate == 0 {
        return "-".to_string();
    }
    format!(
        "{:.2} ms",
        f64::from(frames) * 1000.0 / f64::from(sample_rate)
    )
}

impl SettingsWindow {
    /// Opens the window on its last tab and reads the device lists.
    pub fn show_window(&mut self) {
        self.open = true;
        self.rescan();
    }

    fn rescan(&mut self) {
        self.devices = Some(Session::audio_devices());
        self.ports = Some(Session::midi_ports());
    }

    /// Draws the window when it is open. A change goes to the session at once, and an error comes back
    /// as the status message.
    pub fn ui(&mut self, ctx: &egui::Context, session: &mut Session) -> Option<String> {
        if !self.open {
            return None;
        }
        if self.devices.is_none() || self.ports.is_none() {
            self.rescan();
        }
        let mut open = self.open;
        let mut message = None;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, SettingsTab::Audio, "Audio");
                    ui.selectable_value(&mut self.tab, SettingsTab::Midi, "Link, Tempo & MIDI");
                    if ui.button("Rescan").clicked() {
                        self.rescan();
                    }
                });
                ui.separator();
                message = match self.tab {
                    SettingsTab::Audio => self.audio_tab(ui, session),
                    SettingsTab::Midi => self.midi_tab(ui, session),
                };
            });
        self.open = open;
        message
    }

    fn audio_tab(&self, ui: &mut egui::Ui, session: &mut Session) -> Option<String> {
        let devices = self.devices.as_ref()?;
        let current = session.preferences().audio.clone();
        let choices = audio_choices(devices, &current);
        let mut next = current.clone();
        egui::Grid::new("audio_settings")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Driver Type");
                text_combo(
                    ui,
                    "driver_type",
                    &mut next.driver_type,
                    &choices.driver_types,
                );
                ui.end_row();
                ui.label("Audio Input Device");
                text_combo(ui, "input_device", &mut next.input_device, &choices.inputs);
                ui.end_row();
                ui.label("Audio Output Device");
                text_combo(
                    ui,
                    "output_device",
                    &mut next.output_device,
                    &choices.outputs,
                );
                ui.end_row();
                ui.label("Sample Rate");
                number_combo(
                    ui,
                    "sample_rate",
                    &mut next.sample_rate,
                    &choices.sample_rates,
                    "Hz",
                );
                ui.end_row();
                ui.label("Buffer Size");
                number_combo(
                    ui,
                    "buffer_size",
                    &mut next.buffer_size,
                    &choices.buffer_sizes,
                    "Samples",
                );
                ui.end_row();
                let (latency, rate) = session.latency().unwrap_or_default();
                ui.label("Input Latency");
                ui.label(latency_text(latency.input_frames, rate));
                ui.end_row();
                ui.label("Output Latency");
                ui.label(latency_text(latency.output_frames, rate));
                ui.end_row();
                ui.label("Overall Latency");
                ui.label(latency_text(
                    latency.input_frames + latency.output_frames,
                    rate,
                ));
                ui.end_row();
            });
        if next.driver_type != current.driver_type {
            // A device of another host does not exist in the new one.
            next.input_device = None;
            next.output_device = None;
        }
        if next == current {
            return None;
        }
        Some(match session.set_audio_preferences(next) {
            Ok(()) => "Audio settings changed".to_string(),
            Err(error) => format!("Audio settings not changed: {error}"),
        })
    }

    fn midi_tab(&self, ui: &mut egui::Ui, session: &mut Session) -> Option<String> {
        let ports = self.ports.as_ref()?;
        let current = session.preferences().midi.clone();
        let mut inputs = midi_rows(&ports.inputs, &current.inputs, current.inputs.is_empty());
        let mut outputs = midi_rows(&ports.outputs, &current.outputs, false);
        let mut changed = false;
        ui.label("MIDI Ports");
        egui::Grid::new("midi_ports")
            .num_columns(4)
            .spacing([12.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("");
                ui.label("Track");
                ui.label("Sync");
                ui.label("Remote");
                ui.end_row();
                for (direction, rows) in [("In", &mut inputs), ("Out", &mut outputs)] {
                    for row in rows.iter_mut() {
                        let name = format!("{direction}: {}", row.port.name);
                        if row.connected {
                            ui.label(name);
                        } else {
                            ui.label(
                                egui::RichText::new(format!("{name} (not connected)"))
                                    .color(Color32::GRAY),
                            );
                        }
                        changed |= ui.checkbox(&mut row.port.track, "").changed();
                        changed |= ui.checkbox(&mut row.port.sync, "").changed();
                        changed |= ui.checkbox(&mut row.port.remote, "").changed();
                        ui.end_row();
                    }
                }
            });
        if ports.inputs.is_empty() && ports.outputs.is_empty() {
            ui.label("No MIDI ports are connected.");
        }
        if !changed {
            return None;
        }
        let rows = |rows: Vec<MidiRow>| rows.into_iter().map(|row| row.port).collect();
        let next = MidiPreferences {
            inputs: rows(inputs),
            outputs: rows(outputs),
        };
        Some(match session.set_midi_preferences(next) {
            Ok(()) => "MIDI settings changed".to_string(),
            Err(error) => format!("MIDI settings not changed: {error}"),
        })
    }
}

/// A combo box over `options` with "Default" first, which is `None`.
fn text_combo(ui: &mut egui::Ui, id: &str, value: &mut Option<String>, options: &[String]) {
    let shown = value.clone().unwrap_or_else(|| "Default".to_string());
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(260.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "Default");
            for option in options {
                ui.selectable_value(value, Some(option.clone()), option);
            }
        });
}

/// A combo box over the numbers in `options`, with "Default" first, which is `None`.
fn number_combo(ui: &mut egui::Ui, id: &str, value: &mut Option<u32>, options: &[u32], unit: &str) {
    let shown = value.map_or_else(
        || "Default".to_string(),
        |number| format!("{number} {unit}"),
    );
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(260.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "Default");
            for &option in options {
                ui.selectable_value(value, Some(option), format!("{option} {unit}"));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(
        name: &str,
        is_default: bool,
        rates: &[u32],
        buffer: Option<(u32, u32)>,
    ) -> DeviceInfo {
        DeviceInfo {
            name: name.to_string(),
            is_default,
            channels: 2,
            sample_rates: rates.to_vec(),
            buffer_frames: buffer,
        }
    }

    fn devices() -> DeviceList {
        DeviceList {
            hosts: vec![
                HostInfo {
                    name: "WASAPI".to_string(),
                    is_default: true,
                    outputs: vec![
                        device("Speakers", true, &[44_100, 48_000], Some((128, 1024))),
                        device("Headphones", false, &[48_000, 96_000], None),
                    ],
                    inputs: vec![device("Microphone", true, &[48_000], None)],
                },
                HostInfo {
                    name: "ASIO".to_string(),
                    is_default: false,
                    outputs: vec![device("Interface", true, &[96_000], Some((32, 64)))],
                    inputs: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn the_default_host_and_output_give_the_choices_without_preferences() {
        let choices = audio_choices(&devices(), &AudioPreferences::default());
        assert_eq!(choices.driver_types, vec!["WASAPI", "ASIO"]);
        assert_eq!(choices.outputs, vec!["Speakers", "Headphones"]);
        assert_eq!(choices.inputs, vec!["Microphone"]);
        assert_eq!(choices.sample_rates, vec![44_100, 48_000]);
        assert_eq!(choices.buffer_sizes, vec![128, 256, 512, 1024]);
    }

    #[test]
    fn the_named_host_and_output_give_their_own_rates_and_buffer_sizes() {
        let asio = AudioPreferences {
            driver_type: Some("ASIO".to_string()),
            ..AudioPreferences::default()
        };
        let choices = audio_choices(&devices(), &asio);
        assert_eq!(choices.outputs, vec!["Interface"]);
        assert!(choices.inputs.is_empty());
        assert_eq!(choices.sample_rates, vec![96_000]);
        assert_eq!(choices.buffer_sizes, vec![32, 64]);

        let headphones = AudioPreferences {
            output_device: Some("Headphones".to_string()),
            ..AudioPreferences::default()
        };
        let choices = audio_choices(&devices(), &headphones);
        assert_eq!(choices.sample_rates, vec![48_000, 96_000]);
        assert_eq!(choices.buffer_sizes, BUFFER_SIZES.to_vec());
    }

    #[test]
    fn no_hosts_give_no_choices_but_every_buffer_size() {
        let choices = audio_choices(
            &DeviceList { hosts: Vec::new() },
            &AudioPreferences::default(),
        );
        assert!(choices.driver_types.is_empty() && choices.outputs.is_empty());
        assert!(choices.sample_rates.is_empty());
        assert_eq!(choices.buffer_sizes, BUFFER_SIZES.to_vec());
    }

    #[test]
    fn midi_rows_keep_saved_switches_and_list_saved_ports_that_are_not_connected() {
        let saved = vec![
            MidiPortPreference {
                name: "Pad".to_string(),
                track: false,
                sync: true,
                remote: false,
            },
            MidiPortPreference {
                name: "Old Keyboard".to_string(),
                track: true,
                ..MidiPortPreference::default()
            },
        ];
        let available = ["Keyboard".to_string(), "Pad".to_string()];
        let rows = midi_rows(&available, &saved, false);
        let summary: Vec<(&str, bool, bool, bool)> = rows
            .iter()
            .map(|row| {
                (
                    row.port.name.as_str(),
                    row.port.track,
                    row.port.sync,
                    row.connected,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Keyboard", false, false, true),
                ("Pad", false, true, true),
                ("Old Keyboard", true, false, false),
            ]
        );
    }

    #[test]
    fn inputs_without_preferences_show_track_on_as_the_session_opens_them() {
        let available = ["Keyboard".to_string()];
        let rows = midi_rows(&available, &[], true);
        assert!(rows[0].port.track && rows[0].connected);
    }

    #[test]
    fn latency_is_shown_in_milliseconds() {
        assert_eq!(latency_text(441, 44_100), "10.00 ms");
        assert_eq!(latency_text(256, 48_000), "5.33 ms");
        assert_eq!(latency_text(0, 0), "-");
    }
}
