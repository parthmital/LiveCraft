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

        // Test ArrangementClip non-destructive editing (split, duplicate, trim)
        let arr_clip =
            ArrangementClip::new(100, track.clip_slots[0].clip.clone().unwrap(), 4.0, 4.0);

        // Split at beat 6.0 (2 beats into the 4-beat clip)
        let (first, second) = arr_clip.split_at(6.0, 101).expect("Split failed");
        assert_eq!(first.start_beat, 4.0);
        assert_eq!(first.length_beats, 2.0);
        assert_eq!(second.start_beat, 6.0);
        assert_eq!(second.length_beats, 2.0);

        if let ClipPayload::Midi(ref m1) = first.clip.payload {
            // Notes 60 (0..1) and 64 (1..2) should be in first
            assert_eq!(m1.notes.len(), 2);
            assert_eq!(m1.notes[0].pitch, 60);
            assert_eq!(m1.notes[1].pitch, 64);
        }
        if let ClipPayload::Midi(ref m2) = second.clip.payload {
            // Note 67 was originally at 2.0..4.0, which corresponds to 0.0..2.0 in second part
            assert_eq!(m2.notes.len(), 1);
            assert_eq!(m2.notes[0].pitch, 67);
            assert_eq!(m2.notes[0].start_beat, 0.0);
            assert_eq!(m2.notes[0].duration_beats, 2.0);
        }

        // Test duplicate
        let dup = arr_clip.duplicate(102, 8.0);
        assert_eq!(dup.id, 102);
        assert_eq!(dup.start_beat, 12.0);
        assert_eq!(dup.length_beats, 4.0);

        // Test trim_start and trim_end
        let mut trim_clip = arr_clip.clone();
        trim_clip.trim_start(1.0);
        assert_eq!(trim_clip.start_beat, 5.0);
        assert_eq!(trim_clip.length_beats, 3.0);

        trim_clip.trim_end(-1.0);
        assert_eq!(trim_clip.length_beats, 2.0);
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

        // Authentic Ableton Live 12 schema test snippet
        let authentic_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0" SchemaChangeCount="3" Creator="Ableton Live 12.0.1">
  <LiveSet>
    <Transport>
      <Tempo>
        <Manual Value="128.5" />
      </Tempo>
    </Transport>
    <Tracks>
      <MidiTrack Id="1">
        <Name>
          <EffectiveName Value="Synth Lead" />
        </Name>
        <DeviceChain>
          <Mixer>
            <Volume>
              <Manual Value="0.8" />
            </Volume>
            <Pan>
              <Manual Value="-0.25" />
            </Pan>
          </Mixer>
          <Devices>
            <Operator Id="10">
              <On>
                <Manual Value="true" />
              </On>
              <FilterCutoff>
                <Manual Value="2800" />
              </FilterCutoff>
            </Operator>
            <Eq8 Id="11">
              <On>
                <Manual Value="true" />
              </On>
            </Eq8>
            <Compressor2 Id="12">
              <On>
                <Manual Value="false" />
              </On>
              <Threshold>
                <Manual Value="-16.0" />
              </Threshold>
            </Compressor2>
          </Devices>
        </DeviceChain>
      </MidiTrack>
    </Tracks>
  </LiveSet>
</Ableton>"#;

        let parsed_ableton = AlsParser::parse_xml(authentic_xml, "AuthenticTest")
            .expect("Failed to parse authentic Ableton XML snippet");

        assert_eq!(parsed_ableton.transport.bpm, 128.5);
        assert_eq!(parsed_ableton.tracks.len(), 1);
        let track = &parsed_ableton.tracks[0];
        assert_eq!(track.name, "Synth Lead");
        assert!((track.volume_db - (20.0 * 0.8f32.log10())).abs() < 1e-3);
        assert!((track.pan - (-0.25)).abs() < 1e-4);

        assert_eq!(track.device_chain.len(), 3);
        assert_eq!(track.device_chain[0].device_type, DeviceType::Operator);
        assert_eq!(track.device_chain[0].id, 10);
        assert!(track.device_chain[0].is_enabled);
        assert_eq!(track.device_chain[0].get_param("filter_cutoff"), 2800.0);

        assert_eq!(track.device_chain[1].device_type, DeviceType::EqEight);
        assert_eq!(track.device_chain[1].id, 11);
        assert!(track.device_chain[1].is_enabled);

        assert_eq!(track.device_chain[2].device_type, DeviceType::Compressor);
        assert_eq!(track.device_chain[2].id, 12);
        assert!(!track.device_chain[2].is_enabled);
        assert_eq!(track.device_chain[2].get_param("threshold_db"), -16.0);
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
        assert!(!xml.contains("<Device "));
        assert!(xml.contains("<Operator Id=\"101\">"));
        assert!(xml.contains("<FilterCutoff>"));
        assert!(xml.contains("<Attack>"));

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

        // Verify gzip .als file save and load roundtrip
        let temp_dir = std::env::temp_dir();
        let als_path = temp_dir.join("livecraft_test_roundtrip.als");
        AlsParser::save_project_to_als(&proj, &als_path).expect("Failed to save .als");
        let loaded_proj = AlsParser::load_project(&als_path).expect("Failed to load .als");
        let _ = std::fs::remove_file(&als_path);
        assert_eq!(loaded_proj.scale.root_note, 2);
        let loaded_dev = loaded_proj.tracks[0]
            .device_chain
            .iter()
            .find(|d| d.name == "Lead Synth")
            .expect("Lead Synth device not found in loaded .als");
        assert_eq!(loaded_dev.get_param("filter_cutoff"), 3200.0);
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

        // Test AutomationLane and undo/redo of automation points
        let vol_lane = AutomationLane::new(1, AutomationTarget::TrackVolume);
        let pan_lane = AutomationLane::new(2, AutomationTarget::TrackPan);
        proj.tracks[0].automation_lanes.push(vol_lane);
        proj.tracks[0].automation_lanes.push(pan_lane);
        assert_eq!(proj.tracks[0].automation_lanes.len(), 2);

        // Initially empty lane evaluate_at returns None
        assert_eq!(proj.tracks[0].automation_lanes[0].evaluate_at(1.0), None);

        // Add first point at beat 0.0, val 0.0
        let p1 = AutomationPoint::new(0.0, 0.0, 0.0);
        let add_p1 = Box::new(AddAutomationPointAction::new(0, 0, p1));
        history.execute(add_p1, &mut proj);
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
        assert_eq!(
            proj.tracks[0].automation_lanes[0].evaluate_at(2.0),
            Some(0.0)
        );

        // Add second point at beat 4.0, val 1.0
        let p2 = AutomationPoint::new(4.0, 1.0, 0.0);
        let add_p2 = Box::new(AddAutomationPointAction::new(0, 0, p2));
        history.execute(add_p2, &mut proj);
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);

        // Evaluate at boundary points, before start, after end, and midpoint
        assert!(
            (proj.tracks[0].automation_lanes[0]
                .evaluate_at(-1.0)
                .unwrap()
                - 0.0)
                .abs()
                < 1e-5
        );
        assert!((proj.tracks[0].automation_lanes[0].evaluate_at(0.0).unwrap() - 0.0).abs() < 1e-5);
        assert!((proj.tracks[0].automation_lanes[0].evaluate_at(2.0).unwrap() - 0.5).abs() < 1e-5);
        assert!((proj.tracks[0].automation_lanes[0].evaluate_at(4.0).unwrap() - 1.0).abs() < 1e-5);
        assert!((proj.tracks[0].automation_lanes[0].evaluate_at(5.0).unwrap() - 1.0).abs() < 1e-5);

        // Undo adding second point
        assert!(history.undo(&mut proj));
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
        assert_eq!(
            proj.tracks[0].automation_lanes[0].evaluate_at(2.0),
            Some(0.0)
        );

        // Redo adding second point
        assert!(history.redo(&mut proj));
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);
        assert!((proj.tracks[0].automation_lanes[0].evaluate_at(2.0).unwrap() - 0.5).abs() < 1e-5);

        // Remove point at index 0
        let rem_p = Box::new(RemoveAutomationPointAction::new(0, 0, 0));
        history.execute(rem_p, &mut proj);
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
        assert_eq!(proj.tracks[0].automation_lanes[0].points[0].beat, 4.0);

        // Undo removal
        assert!(history.undo(&mut proj));
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);
        assert_eq!(proj.tracks[0].automation_lanes[0].points[0].beat, 0.0);

        // Redo removal
        assert!(history.redo(&mut proj));
        assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
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

        // Test Chop into 2 subdivisions
        let mut chop_data = MidiClipData {
            notes: vec![MidiNote::new(60, 0.0, 1.0, 100)],
        };
        chop_data.chop(2);
        assert_eq!(chop_data.notes.len(), 2);
        assert_eq!(chop_data.notes[0].duration_beats, 0.5);
        assert_eq!(chop_data.notes[1].start_beat, 0.5);

        // Test Span (legato)
        let mut span_data = MidiClipData {
            notes: vec![
                MidiNote::new(60, 0.0, 0.5, 100),
                MidiNote::new(62, 1.0, 0.5, 100),
            ],
        };
        span_data.span();
        assert_eq!(span_data.notes[0].duration_beats, 1.0);

        // Test Euclidean rhythm (3 pulses over 8 steps)
        let mut euc_data = MidiClipData { notes: Vec::new() };
        euc_data.generate_euclidean(3, 8, 36, 4.0, 100);
        assert_eq!(euc_data.notes.len(), 3);

        // Test Stacks (Triad: 0, 4, 7)
        let mut stack_data = MidiClipData { notes: Vec::new() };
        stack_data.generate_stacks(&[0, 4, 7], 60, 2.0, 110, None);
        assert_eq!(stack_data.notes.len(), 3);
        assert_eq!(stack_data.notes[0].pitch, 60);
        assert_eq!(stack_data.notes[1].pitch, 64);
        assert_eq!(stack_data.notes[2].pitch, 67);
    }

    #[test]
    fn test_tuning_system() {
        let default_tuning = TuningSystem::default();
        let f69 = default_tuning.note_to_freq(69);
        assert!((f69 - 440.0).abs() < 1e-4);
        let f81 = default_tuning.note_to_freq(81);
        assert!((f81 - 880.0).abs() < 1e-4);

        let scl_text = "! pentatonic.scl\nSlendro 5-TET\n5\n240.0\n480.0\n720.0\n960.0\n1200.0\n";
        let slendro = TuningSystem::from_scala(scl_text).expect("Failed to parse scala");
        assert_eq!(slendro.degrees_cents.len(), 6);
        assert!((slendro.degrees_cents[1] - 240.0).abs() < 1e-4);

        // Microtonal calculation verification
        let s69 = slendro.note_to_freq(69);
        assert!((s69 - 440.0).abs() < 1e-4);

        let s70 = slendro.note_to_freq(70);
        let expected_s70 = 440.0 * 2.0f64.powf(240.0 / 1200.0);
        assert!((s70 - expected_s70).abs() < 1e-4);
        // Ensure it differs from standard 12-TET note 70 (466.16 Hz)
        assert!((s70 - default_tuning.note_to_freq(70)).abs() > 30.0);

        // 5 notes up in 5-TET is an octave (880.0 Hz)
        let s74 = slendro.note_to_freq(74);
        assert!((s74 - 880.0).abs() < 1e-4);
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
