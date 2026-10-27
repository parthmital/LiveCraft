use super::*;

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
fn test_parse_real_ableton_template() {
    let template_path =
        r"C:\ProgramData\Ableton\Live 12 Suite\Resources\Builtin\Templates\DefaultLiveSet.als";
    if std::path::Path::new(template_path).exists() {
        let proj = AlsParser::load_project(template_path)
            .expect("Failed to load official Ableton template");
        assert!(proj.tracks.len() >= 4);
    }
}

#[test]
fn test_als_decompression_rejects_oversized_output() {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&[b'a'; 4096]).unwrap();
    let gz = encoder.finish().unwrap();

    let err = AlsParser::decompress_reader(gz.as_slice(), 1024).unwrap_err();
    assert!(matches!(err, AlsError::TooLarge { limit: 1024 }));

    let xml = AlsParser::decompress_reader(gz.as_slice(), 4096).unwrap();
    assert_eq!(xml.len(), 4096);
}
