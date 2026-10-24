use crate::model::*;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
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

        let mut buf = Vec::new();
        let mut current_track: Option<Track> = None;
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
                        "Name" | "UserName" | "EffectiveName" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"Value" {
                                    let val = String::from_utf8_lossy(&attr.value).to_string();
                                    if !val.is_empty() {
                                        if let Some(ref mut trk) = current_track {
                                            trk.name = val;
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
                        "KeyTrack" => {
                            // Container for MIDI key
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

                            if let Some(ref mut clip) = current_clip {
                                if let ClipPayload::Midi(ref mut mdata) = clip.payload {
                                    mdata.notes.push(MidiNote::new(
                                        current_midi_key,
                                        time,
                                        dur,
                                        vel,
                                    ));
                                }
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

                    if in_tempo && (tag_name == "Manual" || tag_name == "Value") {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    if let Ok(val) = s.parse::<f32>() {
                                        bpm = val;
                                    }
                                }
                            }
                        }
                    } else if tag_name == "MidiKey" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    current_midi_key = s.parse().unwrap_or(60);
                                }
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

                        if let Some(ref mut clip) = current_clip {
                            if let ClipPayload::Midi(ref mut mdata) = clip.payload {
                                mdata
                                    .notes
                                    .push(MidiNote::new(current_midi_key, time, dur, vel));
                            }
                        }
                    } else if tag_name == "UserName" || tag_name == "EffectiveName" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Value" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if !val.is_empty() {
                                    if let Some(ref mut trk) = current_track {
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
                        "MidiClip" | "AudioClip" => {
                            if let Some(clip) = current_clip.take() {
                                if let Some(ref mut trk) = current_track {
                                    if current_clip_slot_idx < trk.clip_slots.len() {
                                        trk.clip_slots[current_clip_slot_idx].clip = Some(clip);
                                    } else {
                                        let slot_id = (trk.id * 1000) + trk.clip_slots.len() as u64;
                                        trk.clip_slots.push(ClipSlot::with_clip(slot_id, clip));
                                    }
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

        // Master track default devices
        master_track
            .device_chain
            .push(Device::new(901, "Limiter", DeviceType::Limiter));

        let transport = Transport {
            bpm,
            ..Default::default()
        };

        Ok(Project {
            schema_version,
            creator,
            name: project_name.to_string(),
            transport,
            tracks,
            return_tracks,
            master_track,
            scenes,
        })
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
        ableton_elem.push_attribute(("Creator", project.creator.as_str()));
        ableton_elem.push_attribute(("Revision", "livecraft-r1"));
        writer.write_event(Event::Start(ableton_elem))?;

        writer.write_event(Event::Start(BytesStart::new("LiveSet")))?;

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

            // DeviceChain -> Mixer
            writer.write_event(Event::Start(BytesStart::new("DeviceChain")))?;
            writer.write_event(Event::Start(BytesStart::new("Mixer")))?;

            // Volume
            writer.write_event(Event::Start(BytesStart::new("Volume")))?;
            let mut vol = BytesStart::new("Manual");
            let vol_str = track.volume_db.to_string();
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
