pub mod als;
pub mod command;
pub mod model;
pub mod theme;

pub use als::*;
pub use command::*;
pub use model::*;
pub use theme::*;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let proj = Project::new_default("Test Project");
        assert_eq!(proj.tracks.len(), 4);
        assert_eq!(proj.return_tracks.len(), 2);
        assert_eq!(proj.scenes.len(), 8);
        assert_eq!(proj.transport.bpm, 120.0);
    }

    #[test]
    fn test_theme_colors() {
        let theme = AbletonTheme::default();
        let amber = theme.parse_color("ViewCheckControlEnabledOn");
        assert_eq!(amber[0], 0xff);
        assert_eq!(amber[1], 0xad);
        assert_eq!(amber[2], 0x56);
    }

    #[test]
    fn test_theme_hex_variants() {
        // 6-digit hex
        assert_eq!(theme::parse_hex_color("#ff0000"), [255, 0, 0, 255]);
        // 8-digit hex with alpha
        assert_eq!(theme::parse_hex_color("#00ff0080"), [0, 255, 0, 128]);
        // Invalid fallback
        assert_eq!(theme::parse_hex_color("invalid"), [255, 255, 255, 255]);
    }

    #[test]
    fn test_device_defaults() {
        let eq = Device::new(1, "EQ", DeviceType::EqEight);
        assert_eq!(eq.get_param("band_1_enabled"), 1.0);
        assert!(eq.get_param("band_8_freq") > 1000.0);

        let comp = Device::new(2, "Comp", DeviceType::Compressor);
        assert_eq!(comp.get_param("threshold_db"), -18.0);
        assert_eq!(comp.get_param("ratio"), 4.0);

        let delay = Device::new(3, "Delay", DeviceType::Delay);
        assert_eq!(delay.get_param("ping_pong"), 1.0);

        let op = Device::new(4, "Operator", DeviceType::Operator);
        assert_eq!(op.get_param("osc1_level"), 1.0);
        assert_eq!(op.get_param("filter_cutoff"), 2400.0);
    }

    #[test]
    fn test_clip_slot_and_payload_manipulation() {
        let mut track = Track::new(1, "Synth Lead", TrackType::Midi, 4);
        assert_eq!(track.clip_slots.len(), 4);

        let mut clip = Clip::new_midi(10, "Melody", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100));
            mdata.notes.push(MidiNote::new(64, 1.0, 1.0, 100));
            mdata.notes.push(MidiNote::new(67, 2.0, 2.0, 110));
        }

        track.clip_slots[0].clip = Some(clip);
        assert!(track.clip_slots[0].clip.is_some());

        let retrieved = track.clip_slots[0].clip.as_ref().unwrap();
        assert_eq!(retrieved.name, "Melody");
        if let ClipPayload::Midi(ref m) = retrieved.payload {
            assert_eq!(m.notes.len(), 3);
            assert_eq!(m.notes[2].pitch, 67);
        }
    }

    #[test]
    fn test_als_xml_export_and_import() {
        let proj = Project::new_default("ExportTest");
        let xml = AlsParser::export_to_xml(&proj).expect("Failed to export XML");
        assert!(xml.contains("<Ableton MajorVersion=\"5\""));
        assert!(xml.contains("1-MIDI"));

        let imported = AlsParser::parse_xml(&xml, "Imported").expect("Failed to parse XML");
        assert_eq!(imported.tracks.len(), 4);
        assert_eq!(imported.transport.bpm, 120.0);
    }

    #[test]
    fn test_als_full_roundtrip_with_devices_and_clips() {
        let mut proj = Project::new_default("FullRoundtrip");
        proj.scale = ScaleConfig::new(2, ScaleType::Dorian); // D Dorian
        proj.scale.is_active = true;

        // Add Operator device with custom cutoff
        let mut op = Device::new(101, "Lead Synth", DeviceType::Operator);
        op.set_param("filter_cutoff", 3200.0);
        op.set_param("attack_ms", 12.0);
        proj.tracks[0].device_chain.push(op);

        // Add clip with notes
        let mut clip = Clip::new_midi(201, "Intro Hook", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(62, 0.0, 1.0, 110)); // D4
            mdata.notes.push(MidiNote::new(65, 1.0, 1.0, 105)); // F4
            mdata.notes.push(MidiNote::new(69, 2.0, 2.0, 120)); // A4
        }
        proj.tracks[0].clip_slots[0].clip = Some(clip);

        let xml = AlsParser::export_to_xml(&proj).expect("Export failed");
        assert!(xml.contains("Lead Synth"));
        assert!(xml.contains("Intro Hook"));
        assert!(xml.contains("Dorian"));

        let imported = AlsParser::parse_xml(&xml, "FullRoundtrip").expect("Import failed");

        // Verify Scale
        assert_eq!(imported.scale.root_note, 2);
        assert_eq!(imported.scale.scale_type, ScaleType::Dorian);
        assert!(imported.scale.is_active);

        // Verify Device & Parameters
        let track0 = &imported.tracks[0];
        let dev = track0
            .device_chain
            .iter()
            .find(|d| d.name == "Lead Synth")
            .expect("Device not found after import");
        assert_eq!(dev.get_param("filter_cutoff"), 3200.0);
        assert_eq!(dev.get_param("attack_ms"), 12.0);

        // Verify Clip & Notes
        let clip_slot = &track0.clip_slots[0];
        let imported_clip = clip_slot.clip.as_ref().expect("Clip missing");
        assert_eq!(imported_clip.name, "Intro Hook");
        if let ClipPayload::Midi(ref mdata) = imported_clip.payload {
            assert_eq!(mdata.notes.len(), 3);
            assert_eq!(mdata.notes[0].pitch, 62);
            assert_eq!(mdata.notes[1].pitch, 65);
            assert_eq!(mdata.notes[2].pitch, 69);
            assert_eq!(mdata.notes[2].velocity, 120);
        } else {
            panic!("Expected Midi payload");
        }
    }

    #[test]
    fn test_command_undo_redo() {
        let mut proj = Project::new_default("UndoTest");
        let mut history = CommandHistory::default();

        assert_eq!(proj.transport.bpm, 120.0);

        let action = Box::new(SetTempoAction {
            old_tempo: 120.0,
            new_tempo: 140.0,
        });
        history.execute(action, &mut proj);
        assert_eq!(proj.transport.bpm, 140.0);

        assert!(history.undo(&mut proj));
        assert_eq!(proj.transport.bpm, 120.0);

        assert!(history.redo(&mut proj));
        assert_eq!(proj.transport.bpm, 140.0);
    }

    #[test]
    fn test_command_history_capacity() {
        let mut proj = Project::new_default("HistoryCapTest");
        let mut history = CommandHistory::with_capacity(5);

        for i in 1..=10 {
            let action = Box::new(SetTempoAction {
                old_tempo: 120.0,
                new_tempo: 120.0 + i as f32,
            });
            history.execute(action, &mut proj);
        }

        assert_eq!(proj.transport.bpm, 130.0);
        // Can only undo 5 times because max_history is 5
        let mut undos = 0;
        while history.undo(&mut proj) {
            undos += 1;
        }
        assert_eq!(undos, 5);
        assert_eq!(proj.transport.bpm, 125.0);
    }

    #[test]
    fn test_scale_system_and_snapping() {
        // C Major: C(60), D(62), E(64), F(65), G(67), A(69), B(71)
        let c_maj = ScaleConfig::new(0, ScaleType::Major);
        assert!(c_maj.is_in_scale(60)); // C4
        assert!(!c_maj.is_in_scale(61)); // C#4
        assert!(c_maj.is_in_scale(62)); // D4
        assert_eq!(c_maj.snap_to_scale(61), 60); // snaps to C or D

        // A Minor: A(57), B(59), C(60), D(62), E(64), F(65), G(67)
        let a_min = ScaleConfig::new(9, ScaleType::Minor);
        assert!(a_min.is_in_scale(60)); // C
        assert!(a_min.is_in_scale(57)); // A
        assert!(!a_min.is_in_scale(58)); // A#
    }

    #[test]
    fn test_midi_transformations() {
        let mut data = MidiClipData {
            notes: vec![
                MidiNote::new(60, 0.24, 0.98, 100),
                MidiNote::new(64, 0.26, 1.05, 100),
                MidiNote::new(67, 1.10, 0.45, 100),
            ],
        };

        // Quantize to 16th notes (0.25)
        data.quantize(0.25);
        assert_eq!(data.notes[0].start_beat, 0.25);
        assert_eq!(data.notes[1].start_beat, 0.25);
        assert_eq!(data.notes[2].start_beat, 1.0);

        // Transpose up 2 semitones
        data.transpose(2, None);
        assert_eq!(data.notes[0].pitch, 62);
        assert_eq!(data.notes[1].pitch, 66);
        assert_eq!(data.notes[2].pitch, 69);

        // Strum chord at 0.25 beats
        data.strum(0.1, true);
        assert!(data.notes[1].start_beat > data.notes[0].start_beat);

        // Velocity ramp from 40 to 120
        data.velocity_ramp(40, 120);
        assert_eq!(data.notes[0].velocity, 40);
        assert_eq!(data.notes[2].velocity, 120);
    }

    #[test]
    fn test_parse_real_ableton_template() {
        let template_path =
            r"C:\ProgramData\Ableton\Live 12 Suite\Resources\Builtin\Templates\DefaultLiveSet.als";
        if std::path::Path::new(template_path).exists() {
            let proj = AlsParser::load_project(template_path)
                .expect("Failed to load official Ableton template");
            assert!(proj.tracks.len() >= 4);
        }
    }
}
