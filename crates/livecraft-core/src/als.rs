use crate::model::*;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AlsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Decompression failed: {0}")]
    Decompress(String),
    #[error("Invalid Ableton file format: {0}")]
    InvalidFormat(String),
}

pub fn map_ableton_tag_to_device_type(tag: &str) -> Option<DeviceType> {
    match tag {
        "Operator" => Some(DeviceType::Operator),
        "OriginalSimpler" | "Simpler" => Some(DeviceType::Simpler),
        "Eq8" | "EqEight" => Some(DeviceType::EqEight),
        "Compressor2" | "Compressor" => Some(DeviceType::Compressor),
        "StereoGain" | "Utility" => Some(DeviceType::Utility),
        "Reverb" => Some(DeviceType::Reverb),
        "Delay" => Some(DeviceType::Delay),
        "Saturator" => Some(DeviceType::Saturator),
        "DrumGroupDevice" | "DrumRack" => Some(DeviceType::DrumRack),
        "AutoFilter" => Some(DeviceType::AutoFilter),
        "FilterEQ3" | "EqThree" => Some(DeviceType::EqThree),
        "Chorus" => Some(DeviceType::Chorus),
        "AutoPan" => Some(DeviceType::AutoPan),
        "Gate" => Some(DeviceType::Gate),
        "Redux2" | "Redux" => Some(DeviceType::Redux),
        "Limiter" => Some(DeviceType::Limiter),
        _ => None,
    }
}

pub fn default_device_name_for_type(dtype: &DeviceType) -> &'static str {
    match dtype {
        DeviceType::Operator => "Operator",
        DeviceType::Simpler => "Simpler",
        DeviceType::EqEight => "EQ Eight",
        DeviceType::Compressor => "Compressor",
        DeviceType::Utility => "Utility",
        DeviceType::Reverb => "Reverb",
        DeviceType::Delay => "Delay",
        DeviceType::Saturator => "Saturator",
        DeviceType::DrumRack => "Drum Rack",
        DeviceType::AutoFilter => "Auto Filter",
        DeviceType::EqThree => "EQ Three",
        DeviceType::Chorus => "Chorus",
        DeviceType::AutoPan => "Auto Pan",
        DeviceType::Gate => "Gate",
        DeviceType::Redux => "Redux",
        DeviceType::Limiter => "Limiter",
        DeviceType::Custom(_) => "Custom Device",
    }
}

fn handle_value_element(
    parent_tag: &str,
    val_str: &str,
    in_tempo: bool,
    bpm: &mut f32,
    current_device: &mut Option<Device>,
    current_track: &mut Option<Track>,
    master_track: &mut Track,
) {
    if in_tempo {
        if let Ok(val) = val_str.parse::<f32>() {
            *bpm = val;
        }
    } else if parent_tag == "On" {
        if let Some(dev) = current_device {
            dev.is_enabled = val_str == "true" || val_str == "1";
        }
    } else if parent_tag == "Volume" {
        if let Ok(val) = val_str.parse::<f32>() {
            let vol_db = if val <= 0.0001 {
                -60.0
            } else {
                20.0 * val.log10()
            };
            if let Some(trk) = current_track {
                trk.volume_db = vol_db;
            } else {
                master_track.volume_db = vol_db;
            }
        }
    } else if parent_tag == "Pan" {
        if let Ok(val) = val_str.parse::<f32>() {
            let pan = val.clamp(-1.0, 1.0);
            if let Some(trk) = current_track {
                trk.pan = pan;
            } else {
                master_track.pan = pan;
            }
        }
    } else if let Some(dev) = current_device
        && let Ok(val) = val_str.parse::<f32>()
    {
        let param_name = match parent_tag {
            "FilterCutoff" | "Cutoff" => Some("filter_cutoff"),
            "FilterResonance" | "Resonance" => Some("filter_resonance"),
            "Attack" => Some("attack_ms"),
            "Decay" => Some("decay_ms"),
            "Sustain" => Some("sustain"),
            "Release" => Some("release_ms"),
            "Drive" => Some("drive_db"),
            "Feedback" => Some("feedback"),
            "DryWet" | "Dry/Wet" => Some("dry_wet"),
            "Time" => Some("time_ms"),
            "Threshold" => Some("threshold_db"),
            "Ratio" => Some("ratio"),
            "RoomSize" => Some("room_size"),
            "Damping" => Some("damping"),
            "StereoWidth" => Some("stereo_width"),
            "Ceiling" => Some("ceiling_db"),
            "Gain" => Some("gain_db"),
            _ => None,
        };
        if let Some(pname) = param_name {
            dev.set_param(pname, val);
        }
    }
}

pub struct AlsParser;

impl AlsParser {
    /// Decompress a .als file into a UTF-8 XML string
    pub fn decompress_to_xml<P: AsRef<Path>>(path: P) -> Result<String, AlsError> {
        let file = File::open(path)?;
        let mut decoder = GzDecoder::new(file);
        let mut xml = String::new();
        decoder.read_to_string(&mut xml).map_err(|e| {
            AlsError::Decompress(format!("Failed to decompress gzip stream: {}", e))
        })?;
        Ok(xml)
    }

    /// Parse a .als file directly into a LiveCraft `Project`
    pub fn load_project<P: AsRef<Path>>(path: P) -> Result<Project, AlsError> {
        let path_ref = path.as_ref();
        let file_stem = path_ref
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        let xml = Self::decompress_to_xml(path_ref)?;
        Self::parse_xml(&xml, file_stem)
    }

    /// Parse XML string into a LiveCraft `Project`
    pub fn parse_xml(xml: &str, project_name: &str) -> Result<Project, AlsError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut creator = "Ableton Live".to_string();
        let mut schema_version = "Unknown".to_string();
        let mut bpm = 120.0f32;
        let mut tracks: Vec<Track> = Vec::new();
        let mut return_tracks: Vec<Track> = Vec::new();
        let mut master_track = Track::new(999, "Master", TrackType::Master, 0);
        let mut scenes: Vec<Scene> = Vec::new();
        let mut scale = ScaleConfig::default();

        let mut buf = Vec::new();
        let mut current_track: Option<Track> = None;
        let mut current_device: Option<Device> = None;
        let mut current_device_tag: Option<String> = None;
        let mut current_tag_stack: Vec<String> = Vec::new();
        let mut in_tempo = false;
        let mut current_midi_key: u8 = 60;
        let mut current_clip: Option<Clip> = None;
        let mut current_clip_slot_idx: usize = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_tag_stack.push(tag_name.clone());

                    match tag_name.as_str() {
                        "Ableton" => {
                            for attr in e.attributes().flatten() {
                                let key = String::from_utf8_lossy(attr.key.as_ref());
                                let val = String::from_utf8_lossy(&attr.value);
                                if key == "Creator" {
                                    creator = val.to_string();
                                } else if key == "MinorVersion" {
                                    schema_version = format!("Live {}", val);
                                }
                            }
                        }
                        "MidiTrack" => {
                            let track_id = tracks.len() as u64 + 1;
                            current_track = Some(Track::new(
                                track_id,
                                format!("{}-MIDI", track_id),
                                TrackType::Midi,
                                8,
                            ));
                        }
                        "AudioTrack" => {
                            let track_id = tracks.len() as u64 + 1;
                            current_track = Some(Track::new(
                                track_id,
                                format!("{}-Audio", track_id),
                                TrackType::Audio,
                                8,
                            ));
                        }
                        "ReturnTrack" => {
                            let track_id = return_tracks.len() as u64 + 1;
                            let letter = (b'A' + return_tracks.len() as u8) as char;
                            current_track = Some(Track::new(
                                track_id + 100,
                                format!("{}-Return", letter),
                                TrackType::Return,
                                8,
                            ));
                        }
                        "Tempo" => {
                            in_tempo = true;
                        }
                        "ScaleInformation" => {
                            scale.is_active = true;
                        }
                        "Name" | "UserName" | "EffectiveName" => {
                            if current_tag_stack.len() >= 2
                                && current_tag_stack[current_tag_stack.len() - 2]
                                    == "ScaleInformation"
                            {
                                for attr in e.attributes().flatten() {
                                    if attr.key.as_ref() == b"Value" {
                                        let s = String::from_utf8_lossy(&attr.value);
                                        for st in ScaleType::ALL {
                                            if st.name().eq_ignore_ascii_case(&s) {
                                                scale.scale_type = *st;
                                                scale.is_active = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else {
                                for attr in e.attributes().flatten() {
                                    if attr.key.as_ref() == b"Value" {
                                        let val = String::from_utf8_lossy(&attr.value).to_string();
                                        if !val.is_empty() {
                                            if let Some(ref mut dev) = current_device {
                                                dev.name = val;
                                            } else if let Some(ref mut clip) = current_clip {
                                                clip.name = val;
                                            } else if let Some(ref mut trk) = current_track {
                                                trk.name = val;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "MidiClip" => {
                            let clip_id = 1000 + current_clip_slot_idx as u64;
                            current_clip = Some(Clip::new_midi(clip_id, "Clip", 4.0));
                        }
                        "AudioClip" => {
                            let clip_id = 2000 + current_clip_slot_idx as u64;
                            current_clip = Some(Clip::new_audio(clip_id, "Audio", "", 4.0));
                        }
                        "Operator" | "OriginalSimpler" | "Simpler" | "Eq8" | "EqEight"
                        | "Compressor2" | "Compressor" | "StereoGain" | "Utility" | "Reverb"
                        | "Delay" | "Saturator" | "DrumGroupDevice" | "DrumRack" | "AutoFilter"
                        | "FilterEQ3" | "EqThree" | "Chorus" | "AutoPan" | "Gate" | "Redux2"
                        | "Redux" | "Limiter" => {
                            let mut dev_id = 0u64;
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"Id"
                                    && let Ok(s) = std::str::from_utf8(&attr.value)
                                {
                                    dev_id = s.parse().unwrap_or(0);
                                }
                            }
                            if dev_id == 0 {
                                dev_id = (tracks.len() as u64 * 100) + 1;
                            }
                            if let Some(dtype) = map_ableton_tag_to_device_type(&tag_name) {
                                let dev_name = default_device_name_for_type(&dtype);
                                current_device = Some(Device::new(dev_id, dev_name, dtype));
                                current_device_tag = Some(tag_name.clone());
                            }
                        }
                        "Manual" | "Value" => {
                            let parent_tag = if current_tag_stack.len() >= 2 {
                                current_tag_stack[current_tag_stack.len() - 2].as_str()
                            } else {
                                ""
                            };
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"Value"
                                    && let Ok(val_str) = std::str::from_utf8(&attr.value)
                                {
                                    handle_value_element(
                                        parent_tag,
                                        val_str,
                                        in_tempo,
                                        &mut bpm,
                                        &mut current_device,
                                        &mut current_track,
                                        &mut master_track,
                                    );
                                }
                            }
                        }
                        "Device" => {
                            let mut dev_id = 1u64;
                            let mut dev_type_str = "Custom".to_string();
                            let mut dev_name = "Device".to_string();
                            let mut dev_enabled = true;
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"Id" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            dev_id = s.parse().unwrap_or(1);
                                        }
                                    }
                                    b"Type" => {
                                        dev_type_str =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"Name" => {
                                        dev_name = String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"IsEnabled" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            dev_enabled = s == "true" || s == "1";
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            let dtype = match dev_type_str.as_str() {
                                "EqEight" | "Eq8" => DeviceType::EqEight,
                                "Compressor" | "Compressor2" => DeviceType::Compressor,
                                "Delay" => DeviceType::Delay,
                                "Reverb" => DeviceType::Reverb,
                                "Limiter" => DeviceType::Limiter,
                                "Utility" => DeviceType::Utility,
                                "Operator" => DeviceType::Operator,
                                "Simpler" => DeviceType::Simpler,
                                "Saturator" => DeviceType::Saturator,
                                "DrumRack" | "DrumGroupDevice" => DeviceType::DrumRack,
                                "AutoFilter" => DeviceType::AutoFilter,
                                "Chorus" => DeviceType::Chorus,
                                "AutoPan" => DeviceType::AutoPan,
                                "Gate" => DeviceType::Gate,
                                "EqThree" => DeviceType::EqThree,
                                "Redux" => DeviceType::Redux,
                                other => DeviceType::Custom(other.to_string()),
                            };
                            current_device = Some(Device {
                                id: dev_id,
                                name: dev_name,
                                device_type: dtype,
                                is_enabled: dev_enabled,
                                parameters: std::collections::HashMap::new(),
                            });
                        }
                        "MidiNoteEvent" => {
                            let mut time = 0.0f64;
                            let mut dur = 1.0f64;
                            let mut vel = 100u8;

                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"Time" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            time = s.parse().unwrap_or(0.0);
                                        }
                                    }
                                    b"Duration" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            dur = s.parse().unwrap_or(1.0);
                                        }
                                    }
                                    b"Velocity" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            let v: f32 = s.parse().unwrap_or(100.0);
                                            vel = v as u8;
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            if let Some(ref mut clip) = current_clip
                                && let ClipPayload::Midi(ref mut mdata) = clip.payload
                            {
                                mdata
                                    .notes
                                    .push(MidiNote::new(current_midi_key, time, dur, vel));
                            }
                        }
                        "Scene" => {
                            let scene_id = scenes.len() as u64 + 1;
                            scenes.push(Scene::new(scene_id, format!("{}", scene_id)));
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if tag_name == "Manual" || tag_name == "Value" {
                        let parent_tag = current_tag_stack.last().map(|s| s.as_str()).unwrap_or("");
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value"
                                && let Ok(val_str) = std::str::from_utf8(&attr.value)
                            {
                                handle_value_element(
                                    parent_tag,
                                    val_str,
                                    in_tempo,
                                    &mut bpm,
                                    &mut current_device,
                                    &mut current_track,
                                    &mut master_track,
                                );
                            }
                        }
                    } else if let Some(dtype) = map_ableton_tag_to_device_type(&tag_name) {
                        let mut dev_id = 0u64;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Id"
                                && let Ok(s) = std::str::from_utf8(&attr.value)
                            {
                                dev_id = s.parse().unwrap_or(0);
                            }
                        }
                        if dev_id == 0 {
                            dev_id = (tracks.len() as u64 * 100) + 1;
                        }
                        let dev_name = default_device_name_for_type(&dtype);
                        let dev = Device::new(dev_id, dev_name, dtype);
                        if let Some(ref mut trk) = current_track {
                            trk.device_chain.push(dev);
                        } else {
                            master_track.device_chain.push(dev);
                        }
                    } else if tag_name == "MidiKey" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value"
                                && let Ok(s) = std::str::from_utf8(&attr.value)
                            {
                                current_midi_key = s.parse().unwrap_or(60);
                            }
                        }
                    } else if tag_name == "MidiNoteEvent" {
                        let mut time = 0.0f64;
                        let mut dur = 1.0f64;
                        let mut vel = 100u8;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"Time" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        time = s.parse().unwrap_or(0.0);
                                    }
                                }
                                b"Duration" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        dur = s.parse().unwrap_or(1.0);
                                    }
                                }
                                b"Velocity" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        let v: f32 = s.parse().unwrap_or(100.0);
                                        vel = v as u8;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(ref mut clip) = current_clip
                            && let ClipPayload::Midi(ref mut mdata) = clip.payload
                        {
                            mdata
                                .notes
                                .push(MidiNote::new(current_midi_key, time, dur, vel));
                        }
                    } else if tag_name == "Parameter" {
                        let mut p_name = String::new();
                        let mut p_val = 0.0f32;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"Name" => {
                                    p_name = String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"Value" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        p_val = s.parse().unwrap_or(0.0);
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !p_name.is_empty()
                            && let Some(ref mut dev) = current_device
                        {
                            dev.set_param(p_name, p_val);
                        }
                    } else if tag_name == "RootNote" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value"
                                && let Ok(s) = std::str::from_utf8(&attr.value)
                            {
                                scale.root_note = s.parse().unwrap_or(0);
                            }
                        }
                    } else if tag_name == "ScaleType"
                        || (tag_name == "Name"
                            && current_tag_stack.last().map(|s| s.as_str())
                                == Some("ScaleInformation"))
                    {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value" {
                                let s = String::from_utf8_lossy(&attr.value);
                                for st in ScaleType::ALL {
                                    if st.name().eq_ignore_ascii_case(&s) {
                                        scale.scale_type = *st;
                                        scale.is_active = true;
                                        break;
                                    }
                                }
                            }
                        }
                    } else if tag_name == "IsActive" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value"
                                && let Ok(s) = std::str::from_utf8(&attr.value)
                            {
                                scale.is_active = s == "true" || s == "1";
                            }
                        }
                    } else if tag_name == "UserName" || tag_name == "EffectiveName" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if !val.is_empty() {
                                    if let Some(ref mut dev) = current_device {
                                        dev.name = val;
                                    } else if let Some(ref mut clip) = current_clip {
                                        clip.name = val;
                                    } else if let Some(ref mut trk) = current_track {
                                        trk.name = val;
                                    }
                                }
                            }
                        }
                    } else if tag_name == "ClipSlot" {
                        current_clip_slot_idx += 1;
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_tag_stack.pop();

                    match tag_name.as_str() {
                        "Tempo" => in_tempo = false,
                        "Device" => {
                            if let Some(dev) = current_device.take() {
                                current_device_tag = None;
                                if let Some(ref mut trk) = current_track {
                                    trk.device_chain.push(dev);
                                } else {
                                    master_track.device_chain.push(dev);
                                }
                            }
                        }
                        tag if current_device_tag.as_deref() == Some(tag) => {
                            if let Some(dev) = current_device.take() {
                                current_device_tag = None;
                                if let Some(ref mut trk) = current_track {
                                    trk.device_chain.push(dev);
                                } else {
                                    master_track.device_chain.push(dev);
                                }
                            }
                        }
                        "MidiClip" | "AudioClip" => {
                            if let Some(clip) = current_clip.take()
                                && let Some(ref mut trk) = current_track
                            {
                                if current_clip_slot_idx < trk.clip_slots.len() {
                                    trk.clip_slots[current_clip_slot_idx].clip = Some(clip);
                                } else {
                                    let slot_id = (trk.id * 1000) + trk.clip_slots.len() as u64;
                                    trk.clip_slots.push(ClipSlot::with_clip(slot_id, clip));
                                }
                            }
                            current_clip_slot_idx += 1;
                        }
                        "MidiTrack" | "AudioTrack" => {
                            if let Some(trk) = current_track.take() {
                                tracks.push(trk);
                            }
                            current_clip_slot_idx = 0;
                        }
                        "ReturnTrack" => {
                            if let Some(trk) = current_track.take() {
                                return_tracks.push(trk);
                            }
                            current_clip_slot_idx = 0;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(AlsError::Xml(err)),
                _ => {}
            }
            buf.clear();
        }

        // If no scenes were parsed, default to 8 scenes
        if scenes.is_empty() {
            scenes = (1..=8)
                .map(|i| Scene::new(i as u64, format!("{}", i)))
                .collect();
        }

        // Master track default devices if none parsed
        if master_track.device_chain.is_empty() {
            master_track
                .device_chain
                .push(Device::new(901, "Limiter", DeviceType::Limiter));
        }

        let transport = Transport {
            bpm,
            ..Default::default()
        };

        Ok(Project {
            schema_version,
            creator,
            name: project_name.to_string(),
            transport,
            scale,
            tuning: None,
            tracks,
            return_tracks,
            master_track,
            scenes,
        })
    }

    fn write_ableton_device<W: std::io::Write>(
        writer: &mut Writer<W>,
        dev: &Device,
    ) -> Result<(), quick_xml::Error> {
        let tag_name = match dev.device_type {
            DeviceType::Operator => "Operator",
            DeviceType::EqEight => "Eq8",
            DeviceType::Compressor => "Compressor2",
            DeviceType::Utility => "StereoGain",
            DeviceType::Delay => "Delay",
            DeviceType::Reverb => "Reverb",
            DeviceType::Saturator => "Saturator",
            DeviceType::AutoFilter => "AutoFilter",
            DeviceType::DrumRack => "DrumGroupDevice",
            DeviceType::Simpler => "OriginalSimpler",
            DeviceType::Chorus => "Chorus",
            DeviceType::AutoPan => "AutoPan",
            DeviceType::Gate => "Gate",
            DeviceType::EqThree => "FilterEQ3",
            DeviceType::Redux => "Redux2",
            DeviceType::Limiter => "Limiter",
            DeviceType::Custom(ref s) => s.as_str(),
        };

        let mut dev_elem = BytesStart::new(tag_name);
        let id_s = dev.id.to_string();
        dev_elem.push_attribute(("Id", id_s.as_str()));
        writer.write_event(Event::Start(dev_elem))?;

        if !dev.name.is_empty() {
            let mut uname = BytesStart::new("UserName");
            uname.push_attribute(("Value", dev.name.as_str()));
            writer.write_event(Event::Empty(uname))?;
        }

        writer.write_event(Event::Start(BytesStart::new("On")))?;
        let mut on_elem = BytesStart::new("Manual");
        on_elem.push_attribute(("Value", if dev.is_enabled { "true" } else { "false" }));
        writer.write_event(Event::Empty(on_elem))?;
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("On")))?;

        let mut sorted_params: Vec<(&String, &f32)> = dev.parameters.iter().collect();
        sorted_params.sort_by_key(|(k, _)| *k);

        for (pk, pv) in sorted_params {
            let param_tag = match pk.as_str() {
                "filter_cutoff" => "FilterCutoff",
                "filter_resonance" => "FilterResonance",
                "attack_ms" => "Attack",
                "decay_ms" => "Decay",
                "sustain" => "Sustain",
                "release_ms" => "Release",
                "drive_db" => "Drive",
                "feedback" => "Feedback",
                "dry_wet" => "DryWet",
                "time_ms" => "Time",
                "threshold_db" => "Threshold",
                "ratio" => "Ratio",
                "room_size" => "RoomSize",
                "damping" => "Damping",
                "stereo_width" => "StereoWidth",
                "ceiling_db" => "Ceiling",
                "gain_db" => "Gain",
                _ => "",
            };

            if !param_tag.is_empty() {
                writer.write_event(Event::Start(BytesStart::new(param_tag)))?;
                let mut manual = BytesStart::new("Manual");
                let val_str = pv.to_string();
                manual.push_attribute(("Value", val_str.as_str()));
                writer.write_event(Event::Empty(manual))?;
                writer.write_event(Event::End(quick_xml::events::BytesEnd::new(param_tag)))?;
            } else {
                let mut p_elem = BytesStart::new("Parameter");
                let val_str = pv.to_string();
                p_elem.push_attribute(("Name", pk.as_str()));
                p_elem.push_attribute(("Value", val_str.as_str()));
                writer.write_event(Event::Empty(p_elem))?;
            }
        }

        writer.write_event(Event::End(quick_xml::events::BytesEnd::new(tag_name)))?;
        Ok(())
    }

    /// Save a LiveCraft `Project` as a real, gzip-compressed Ableton Live `.als` file
    pub fn save_project_to_als<P: AsRef<Path>>(project: &Project, path: P) -> Result<(), AlsError> {
        let file = File::create(path)?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        let xml_string = Self::export_to_xml(project)?;
        encoder.write_all(xml_string.as_bytes())?;
        encoder.finish()?;
        Ok(())
    }

    /// Export a LiveCraft `Project` to an Ableton-compatible XML string
    pub fn export_to_xml(project: &Project) -> Result<String, AlsError> {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(Cursor::new(&mut buffer), b' ', 2);

        // Header
        let mut ableton_elem = BytesStart::new("Ableton");
        ableton_elem.push_attribute(("MajorVersion", "5"));
        ableton_elem.push_attribute(("MinorVersion", "12.0_12117"));
        let creator_str = if project.creator.is_empty() {
            "Ableton Live 12.0.5".to_string()
        } else {
            project.creator.clone()
        };
        ableton_elem.push_attribute(("Creator", creator_str.as_str()));
        ableton_elem.push_attribute(("Revision", "livecraft-r1"));
        writer.write_event(Event::Start(ableton_elem))?;

        writer.write_event(Event::Start(BytesStart::new("LiveSet")))?;

        // ScaleInformation
        writer.write_event(Event::Start(BytesStart::new("ScaleInformation")))?;
        let mut root_elem = BytesStart::new("RootNote");
        let root_s = project.scale.root_note.to_string();
        root_elem.push_attribute(("Value", root_s.as_str()));
        writer.write_event(Event::Empty(root_elem))?;

        let mut st_elem = BytesStart::new("Name");
        st_elem.push_attribute(("Value", project.scale.scale_type.name()));
        writer.write_event(Event::Empty(st_elem))?;
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
            "ScaleInformation",
        )))?;

        // Tracks
        writer.write_event(Event::Start(BytesStart::new("Tracks")))?;
        for track in &project.tracks {
            let tag_name = match track.track_type {
                TrackType::Audio => "AudioTrack",
                TrackType::Midi => "MidiTrack",
                TrackType::Return => "ReturnTrack",
                TrackType::Master => "MainTrack",
            };

            let mut track_elem = BytesStart::new(tag_name);
            let id_str = track.id.to_string();
            track_elem.push_attribute(("Id", id_str.as_str()));
            writer.write_event(Event::Start(track_elem))?;

            // Name
            writer.write_event(Event::Start(BytesStart::new("Name")))?;
            let mut uname = BytesStart::new("UserName");
            uname.push_attribute(("Value", track.name.as_str()));
            writer.write_event(Event::Empty(uname))?;
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Name")))?;

            // DeviceChain
            writer.write_event(Event::Start(BytesStart::new("DeviceChain")))?;

            // Devices
            writer.write_event(Event::Start(BytesStart::new("Devices")))?;
            for dev in &track.device_chain {
                Self::write_ableton_device(&mut writer, dev)?;
            }
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Devices")))?;

            // Mixer
            writer.write_event(Event::Start(BytesStart::new("Mixer")))?;

            // Volume
            writer.write_event(Event::Start(BytesStart::new("Volume")))?;
            let mut vol = BytesStart::new("Manual");
            let lin_vol = if track.volume_db <= -60.0 {
                0.0f32
            } else {
                10.0f32.powf(track.volume_db / 20.0)
            };
            let vol_str = lin_vol.to_string();
            vol.push_attribute(("Value", vol_str.as_str()));
            writer.write_event(Event::Empty(vol))?;
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Volume")))?;

            // Pan
            writer.write_event(Event::Start(BytesStart::new("Pan")))?;
            let mut pan = BytesStart::new("Manual");
            let pan_str = track.pan.to_string();
            pan.push_attribute(("Value", pan_str.as_str()));
            writer.write_event(Event::Empty(pan))?;
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Pan")))?;

            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Mixer")))?;

            // MainSequencer -> ClipSlotList
            writer.write_event(Event::Start(BytesStart::new("MainSequencer")))?;
            writer.write_event(Event::Start(BytesStart::new("ClipSlotList")))?;
            for slot in &track.clip_slots {
                let mut slot_elem = BytesStart::new("ClipSlot");
                let s_id = slot.id.to_string();
                slot_elem.push_attribute(("Id", s_id.as_str()));
                writer.write_event(Event::Start(slot_elem))?;
                writer.write_event(Event::Start(BytesStart::new("ClipSlot")))?;
                writer.write_event(Event::Start(BytesStart::new("Value")))?;

                if let Some(ref clip) = slot.clip {
                    match clip.payload {
                        ClipPayload::Midi(ref mdata) => {
                            let mut clip_elem = BytesStart::new("MidiClip");
                            let c_id = clip.id.to_string();
                            clip_elem.push_attribute(("Id", c_id.as_str()));
                            writer.write_event(Event::Start(clip_elem))?;

                            // Name
                            writer.write_event(Event::Start(BytesStart::new("Name")))?;
                            let mut c_name = BytesStart::new("UserName");
                            c_name.push_attribute(("Value", clip.name.as_str()));
                            writer.write_event(Event::Empty(c_name))?;
                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "Name",
                            )))?;

                            // KeyTracks
                            writer.write_event(Event::Start(BytesStart::new("KeyTracks")))?;
                            let mut notes_by_pitch: std::collections::BTreeMap<u8, Vec<&MidiNote>> =
                                std::collections::BTreeMap::new();
                            for note in &mdata.notes {
                                notes_by_pitch.entry(note.pitch).or_default().push(note);
                            }

                            for (pitch, notes) in notes_by_pitch {
                                writer.write_event(Event::Start(BytesStart::new("KeyTrack")))?;
                                let mut k_elem = BytesStart::new("MidiKey");
                                let p_str = pitch.to_string();
                                k_elem.push_attribute(("Value", p_str.as_str()));
                                writer.write_event(Event::Empty(k_elem))?;

                                writer.write_event(Event::Start(BytesStart::new("Notes")))?;
                                for note in notes {
                                    let mut n_elem = BytesStart::new("MidiNoteEvent");
                                    let t_str = note.start_beat.to_string();
                                    let d_str = note.duration_beats.to_string();
                                    let v_str = note.velocity.to_string();
                                    n_elem.push_attribute(("Time", t_str.as_str()));
                                    n_elem.push_attribute(("Duration", d_str.as_str()));
                                    n_elem.push_attribute(("Velocity", v_str.as_str()));
                                    writer.write_event(Event::Empty(n_elem))?;
                                }
                                writer.write_event(Event::End(
                                    quick_xml::events::BytesEnd::new("Notes"),
                                ))?;
                                writer.write_event(Event::End(
                                    quick_xml::events::BytesEnd::new("KeyTrack"),
                                ))?;
                            }
                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "KeyTracks",
                            )))?;
                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "MidiClip",
                            )))?;
                        }
                        ClipPayload::Audio(ref adata) => {
                            let mut clip_elem = BytesStart::new("AudioClip");
                            let c_id = clip.id.to_string();
                            clip_elem.push_attribute(("Id", c_id.as_str()));
                            writer.write_event(Event::Start(clip_elem))?;

                            writer.write_event(Event::Start(BytesStart::new("Name")))?;
                            let mut c_name = BytesStart::new("UserName");
                            c_name.push_attribute(("Value", clip.name.as_str()));
                            writer.write_event(Event::Empty(c_name))?;
                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "Name",
                            )))?;

                            writer.write_event(Event::Start(BytesStart::new("FileRef")))?;
                            let mut p_elem = BytesStart::new("Path");
                            p_elem.push_attribute(("Value", adata.file_path.as_str()));
                            writer.write_event(Event::Empty(p_elem))?;
                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "FileRef",
                            )))?;

                            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                                "AudioClip",
                            )))?;
                        }
                    }
                }

                writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Value")))?;
                writer.write_event(Event::End(quick_xml::events::BytesEnd::new("ClipSlot")))?;
                writer.write_event(Event::End(quick_xml::events::BytesEnd::new("ClipSlot")))?;
            }
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("ClipSlotList")))?;
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(
                "MainSequencer",
            )))?;

            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("DeviceChain")))?;

            writer.write_event(Event::End(quick_xml::events::BytesEnd::new(tag_name)))?;
        }
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Tracks")))?;

        // Transport
        writer.write_event(Event::Start(BytesStart::new("Transport")))?;
        writer.write_event(Event::Start(BytesStart::new("Tempo")))?;
        let mut manual_tempo = BytesStart::new("Manual");
        let bpm_str = project.transport.bpm.to_string();
        manual_tempo.push_attribute(("Value", bpm_str.as_str()));
        writer.write_event(Event::Empty(manual_tempo))?;
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Tempo")))?;
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Transport")))?;

        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("LiveSet")))?;
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("Ableton")))?;

        let xml = String::from_utf8(buffer).map_err(|e| AlsError::InvalidFormat(e.to_string()))?;
        Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}",
            xml
        ))
    }
}
