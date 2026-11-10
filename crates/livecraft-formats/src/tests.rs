use crate::als::*;
use crate::theme::*;
use livecraft_model::ids::*;
use livecraft_model::model::*;
use std::path::PathBuf;

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
    assert_eq!(crate::theme::parse_hex_color("#ff0000"), [255, 0, 0, 255]);
    // 8-digit hex with alpha
    assert_eq!(crate::theme::parse_hex_color("#00ff0080"), [0, 255, 0, 128]);
    // Invalid fallback
    assert_eq!(
        crate::theme::parse_hex_color("invalid"),
        [255, 255, 255, 255]
    );
}

#[test]
fn test_als_xml_export_and_import() {
    let proj = Project::new_default("ExportTest");
    let xml = AlsParser::export_to_xml(&proj).expect("Failed to export XML");
    assert!(xml.contains("<Ableton MajorVersion=\"5\""));
    assert!(xml.contains("1-MIDI"));

    let imported = AlsParser::parse_xml(&xml, "Imported").expect("Failed to parse XML");
    assert_eq!(imported.tracks.len(), 4);
    assert_eq!(imported.transport.bpm(), 120.0);

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

    assert_eq!(parsed_ableton.transport.bpm(), 128.5);
    assert_eq!(parsed_ableton.tracks.len(), 1);
    let track = &parsed_ableton.tracks[0];
    assert_eq!(track.name, "Synth Lead");
    assert!((track.volume_db - (20.0 * 0.8f32.log10())).abs() < 1e-3);
    assert!((track.pan - (-0.25)).abs() < 1e-4);

    assert_eq!(track.device_chain.len(), 3);
    assert_eq!(track.device_chain[0].device_type, DeviceType::Operator);
    assert_eq!(track.device_chain[0].id, DeviceId(10));
    assert!(track.device_chain[0].is_enabled);
    assert_eq!(track.device_chain[0].get_param("filter_cutoff"), 2800.0);

    assert_eq!(track.device_chain[1].device_type, DeviceType::EqEight);
    assert_eq!(track.device_chain[1].id, DeviceId(11));
    assert!(track.device_chain[1].is_enabled);

    assert_eq!(track.device_chain[2].device_type, DeviceType::Compressor);
    assert_eq!(track.device_chain[2].id, DeviceId(12));
    assert!(!track.device_chain[2].is_enabled);
    assert_eq!(track.device_chain[2].get_param("threshold_db"), -16.0);
}

#[test]
fn test_als_full_roundtrip_with_devices_and_clips() {
    let mut proj = Project::new_default("FullRoundtrip");
    proj.scale = ScaleConfig::new(2, ScaleType::Dorian); // D Dorian
    proj.scale.is_active = true;

    // Add Operator device with custom cutoff
    let mut op = Device::new(DeviceId(101), "Lead Synth", DeviceType::Operator);
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
    assert!(xml.contains("<Root Value=\"2\" />"));
    assert!(xml.contains("<Name Value=\"2\" />"));
    assert!(xml.contains("<InKey Value=\"true\" />"));
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
        assert_eq!(mdata.notes[2].velocity, 120.0);
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

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_formats_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn test_als_roundtrip_audio_clip_return_tracks_and_sends() {
    let dir = temp_test_dir("als_m08_roundtrip");
    let mut proj = Project::new_default("OracleM08");

    // Add a 3rd return track and resize all tracks' sends to 3
    let mut ret3 = Track::new(TrackId(8), "C-Chorus", TrackType::Return, 0);
    ret3.volume_db = -3.0;
    ret3.device_chain
        .push(Device::new(DeviceId(203), "Chorus", DeviceType::Chorus));
    proj.return_tracks.push(ret3);
    for trk in proj.all_tracks_mut() {
        trk.sends.resize(3, 0.0);
    }

    // Set non-default sends on tracks
    proj.tracks[0].sends = vec![0.501_187_2, 0.25, 0.75];
    proj.tracks[2].sends = vec![0.0, 0.8, 0.125];
    proj.return_tracks[0].volume_db = -6.0;
    proj.return_tracks[0].sends = vec![0.0, 0.3, 0.0];

    // Add an AudioClip in slot 0 and another in non-zero slot 2 with non-default length
    let audio_clip_0 = Clip::new_audio(501, "Kick Hit", "samples/kick_01.wav", 4.0);
    let audio_clip_2 = Clip::new_audio(502, "Pad Stem", "samples/pad & bell.wav", 12.0);
    proj.tracks[2].clip_slots[0].clip = Some(audio_clip_0);
    proj.tracks[3].clip_slots[2].clip = Some(audio_clip_2);

    let als_path = dir.join("oracle_m08.als");
    AlsParser::save_project_to_als(&proj, &als_path).expect("save .als");
    let xml = AlsParser::decompress_to_xml(&als_path).expect("decompress .als");
    assert!(xml.contains("<ReturnTrack Id=\"5\">"));
    assert!(xml.contains("<ReturnTrack Id=\"8\">"));
    assert!(xml.contains("<TrackSendHolder Id=\"2\">"));
    assert!(
        xml.contains("<CurrentEnd Value=\"12\"/>") || xml.contains("<CurrentEnd Value=\"12\" />")
    );

    let imported = AlsParser::load_project(&als_path).expect("load .als");
    assert!(
        imported.validate().is_empty(),
        "unexpected validation errors: {:?}",
        imported.validate()
    );

    // Return tracks round-tripped
    assert_eq!(imported.return_tracks.len(), 3);
    assert_eq!(imported.return_tracks[0].id, TrackId(5));
    assert_eq!(imported.return_tracks[0].name, "A-Reverb");
    assert!((imported.return_tracks[0].volume_db - (-6.0)).abs() < 1e-3);
    assert_eq!(imported.return_tracks[0].sends, vec![0.0, 0.3, 0.0]);
    assert_eq!(imported.return_tracks[1].id, TrackId(6));
    assert_eq!(imported.return_tracks[1].name, "B-Delay");
    assert_eq!(imported.return_tracks[2].id, TrackId(8));
    assert_eq!(imported.return_tracks[2].name, "C-Chorus");
    assert_eq!(imported.return_tracks[2].device_chain.len(), 1);
    assert_eq!(
        imported.return_tracks[2].device_chain[0].device_type,
        DeviceType::Chorus
    );

    // Sends round-tripped
    assert_eq!(imported.tracks[0].sends.len(), 3);
    assert!((imported.tracks[0].sends[0] - 0.501_187_2).abs() < 1e-6);
    assert!((imported.tracks[0].sends[1] - 0.25).abs() < 1e-6);
    assert!((imported.tracks[0].sends[2] - 0.75).abs() < 1e-6);
    assert_eq!(imported.tracks[2].sends, vec![0.0, 0.8, 0.125]);

    // AudioClip FileRef/Path, slot index, and length round-tripped
    let c0 = imported.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("slot 0 audio clip");
    assert_eq!(c0.name, "Kick Hit");
    assert_eq!(c0.length_beats, 4.0);
    match &c0.payload {
        ClipPayload::Audio(a) => assert_eq!(a.file_path, "samples/kick_01.wav"),
        other => panic!("expected audio clip, got {other:?}"),
    }

    assert!(imported.tracks[3].clip_slots[0].clip.is_none());
    assert!(imported.tracks[3].clip_slots[1].clip.is_none());
    let c2 = imported.tracks[3].clip_slots[2]
        .clip
        .as_ref()
        .expect("slot 2 audio clip");
    assert_eq!(c2.name, "Pad Stem");
    assert_eq!(c2.length_beats, 12.0);
    assert_eq!(c2.loop_end_beat, 12.0);
    match &c2.payload {
        ClipPayload::Audio(a) => assert_eq!(a.file_path, "samples/pad & bell.wav"),
        other => panic!("expected audio clip, got {other:?}"),
    }

    // Authentic Ableton-style AudioClip (SampleRef/FileRef/Path + Loop/LoopEnd) and TrackSendHolder
    let ableton_audio_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0" SchemaChangeCount="3" Creator="Ableton Live 12.0.1">
  <LiveSet>
    <Tracks>
      <AudioTrack Id="14">
        <Name><EffectiveName Value="Vocals"/></Name>
        <DeviceChain>
          <Mixer>
            <Volume><Manual Value="1.0"/></Volume>
            <Pan><Manual Value="0"/></Pan>
            <Sends>
              <TrackSendHolder Id="0">
                <Send><Manual Value="0.50118723"/></Send>
              </TrackSendHolder>
            </Sends>
          </Mixer>
          <MainSequencer>
            <ClipSlotList>
              <ClipSlot Id="0">
                <ClipSlot>
                  <Value>
                    <AudioClip Id="0">
                      <Name Value="Vox Lead"/>
                      <Loop>
                        <LoopStart Value="0"/>
                        <LoopEnd Value="16"/>
                      </Loop>
                      <SampleRef>
                        <FileRef>
                          <RelativePath Value="Samples/Vox.wav"/>
                          <Path Value="D:/Audio/Vox.wav"/>
                        </FileRef>
                      </SampleRef>
                    </AudioClip>
                  </Value>
                </ClipSlot>
              </ClipSlot>
            </ClipSlotList>
          </MainSequencer>
        </DeviceChain>
      </AudioTrack>
      <ReturnTrack Id="2">
        <Name><EffectiveName Value="A-Verb"/></Name>
        <DeviceChain>
          <Mixer>
            <Volume><Manual Value="1.0"/></Volume>
            <Pan><Manual Value="0"/></Pan>
            <Sends>
              <TrackSendHolder Id="0">
                <Send><Manual Value="0"/></Send>
              </TrackSendHolder>
            </Sends>
          </Mixer>
        </DeviceChain>
      </ReturnTrack>
    </Tracks>
  </LiveSet>
</Ableton>"#;
    let parsed =
        AlsParser::parse_xml(ableton_audio_xml, "AbletonAudio").expect("parse ableton xml");
    assert_eq!(parsed.return_tracks.len(), 1);
    assert_eq!(parsed.return_tracks[0].name, "A-Verb");
    assert_eq!(parsed.tracks[0].sends.len(), 1);
    assert!((parsed.tracks[0].sends[0] - 0.501_187_2).abs() < 1e-6);
    let vox = parsed.tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("vox audio clip");
    assert_eq!(vox.name, "Vox Lead");
    assert_eq!(vox.length_beats, 16.0);
    match &vox.payload {
        ClipPayload::Audio(a) => assert_eq!(a.file_path, "D:/Audio/Vox.wav"),
        other => panic!("expected audio clip, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
