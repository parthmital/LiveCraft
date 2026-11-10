//! Ableton Live Clip (`.alc`) gzip XML reader and writer (manual 5.3, page 134).

use std::fs;
use std::path::{Component, Path};

use livecraft_model::ids::TrackId;
use livecraft_model::model::{Clip, ClipPayload, ClipSlot, Device, Track, TrackType};

use crate::als::AlsError;
use crate::als::map::{
    AlsDocument, AlsIdAllocator, EntityMap, append_value_child, clips, devices, set_child_value,
};
use crate::als::tree::{XmlAttr, XmlTree};
use crate::als::write::write_als_gz;
use crate::health::{copy_dedup, path_is_inside, relative_forward_slash};
use crate::project_dir::{
    ProjectContext, ProjectDirError, SAMPLES_DIR, SAMPLES_IMPORTED_DIR, SampleLocation,
    SampleReference, SampleResolution, resolve_sample_reference,
};

/// A standalone Live Clip (`.alc`) containing a single clip and its track's device chain.
#[derive(Debug, Clone)]
pub struct LiveClipFile {
    pub clip: Clip,
    pub devices: Vec<Device>,
}

fn validate_project_dir(project_dir: &Path) -> Result<(), AlsError> {
    if project_dir.as_os_str().is_empty()
        || project_dir.to_string_lossy().contains('\0')
        || project_dir
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(AlsError::InvalidFormat(format!(
            "invalid project directory `{}`: traversal or empty path is forbidden",
            project_dir.display()
        )));
    }
    Ok(())
}

/// Saves `live_clip` as a gzip-compressed Ableton Live Clip (`.alc`) XML file.
///
/// When `copy_audio_into` is `Some(project_dir)` and the clip is an Audio clip whose
/// sample exists on disk, the sample is opened read-only and deduplicated/copied into
/// `<project_dir>/Samples/Imported/` without moving or modifying the original,
/// and the clip's stored/relative sample paths are updated to reference the copy.
pub fn save_live_clip(
    live_clip: &LiveClipFile,
    path: &Path,
    copy_audio_into: Option<&Path>,
) -> Result<(), AlsError> {
    save_live_clip_in_context(live_clip, path, copy_audio_into, None)
}

/// Saves `live_clip` as a `.alc` file using an optional [`ProjectContext`] to resolve
/// relative sample references before copying into `copy_audio_into` or preserving
/// `audio.relative_path`.
pub fn save_live_clip_in_context(
    live_clip: &LiveClipFile,
    path: &Path,
    copy_audio_into: Option<&Path>,
    ctx: Option<&ProjectContext>,
) -> Result<(), AlsError> {
    let mut clip_to_save = live_clip.clip.clone();

    if let Some(project_dir) = copy_audio_into {
        validate_project_dir(project_dir)?;
        if let ClipPayload::Audio(ref mut audio) = clip_to_save.payload
            && (!audio.file_path.trim().is_empty()
                || audio
                    .relative_path
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty()))
        {
            let r = SampleReference {
                path: audio.file_path.clone(),
                relative_path: audio.relative_path.clone(),
            };
            let resolved_src = if let Some(context) = ctx {
                match resolve_sample_reference(&r, context) {
                    SampleResolution::Resolved { resolved_path, .. } => Some(resolved_path),
                    SampleResolution::Missing { .. } => None,
                }
            } else {
                let proj_ctx = ProjectContext::for_project(project_dir);
                match resolve_sample_reference(&r, &proj_ctx) {
                    SampleResolution::Resolved { resolved_path, .. } => Some(resolved_path),
                    SampleResolution::Missing { .. } => {
                        match resolve_sample_reference(&r, &ProjectContext::default()) {
                            SampleResolution::Resolved { resolved_path, .. } => Some(resolved_path),
                            SampleResolution::Missing { .. } => None,
                        }
                    }
                }
            };
            if let Some(src_path) = resolved_src {
                let imported_dir = project_dir.join(SAMPLES_DIR).join(SAMPLES_IMPORTED_DIR);
                if !path_is_inside(&imported_dir, project_dir) {
                    return Err(AlsError::InvalidFormat(format!(
                        "destination `{}` escapes project directory `{}`",
                        imported_dir.display(),
                        project_dir.display()
                    )));
                }
                fs::create_dir_all(&imported_dir)?;
                if !path_is_inside(&imported_dir, project_dir) {
                    return Err(AlsError::InvalidFormat(format!(
                        "destination `{}` escapes project directory `{}`",
                        imported_dir.display(),
                        project_dir.display()
                    )));
                }
                let (dest_path, rel_str, _reused) =
                    copy_dedup(&src_path, &imported_dir, project_dir).map_err(|e| match e {
                        ProjectDirError::Io(io) => AlsError::Io(io),
                        other => AlsError::InvalidFormat(other.to_string()),
                    })?;
                let raw_dest = dest_path.to_string_lossy().into_owned();
                let normalized_dest = if raw_dest.starts_with(r"\\?\") {
                    raw_dest
                } else {
                    raw_dest.replace('\\', "/")
                };
                audio.file_path = normalized_dest;
                audio.relative_path = Some(rel_str);
            }
        }
    } else if let Some(context) = ctx
        && let ClipPayload::Audio(ref mut audio) = clip_to_save.payload
        && audio.relative_path.is_none()
        && !audio.file_path.trim().is_empty()
    {
        let r = SampleReference::from_path(&audio.file_path);
        if let SampleResolution::Resolved {
            resolved_path,
            location: SampleLocation::InProject,
            ..
        } = resolve_sample_reference(&r, context)
            && let Some(root) = context.project_root.as_deref()
            && let Some(rel) = relative_forward_slash(&resolved_path, root)
        {
            audio.relative_path = Some(rel);
        }
    }

    let mut tree = XmlTree::with_root(
        "Ableton",
        vec![
            XmlAttr::new("MajorVersion", "5"),
            XmlAttr::new("MinorVersion", "12.0_12117"),
            XmlAttr::new("SchemaChangeCount", "10"),
            XmlAttr::new("Creator", "LiveCraft 0.1.0"),
        ],
    );
    let root = tree.root();
    let live_set = tree.alloc_element("LiveSet");
    tree.append_child(root, live_set);
    append_value_child(&mut tree, live_set, "NextPointeeId", "100");
    let tracks_node = tree.alloc_element("Tracks");
    tree.append_child(live_set, tracks_node);

    let (track_tag, track_type) = match clip_to_save.payload {
        ClipPayload::Midi(_) => ("MidiTrack", TrackType::Midi),
        ClipPayload::Audio(_) => ("AudioTrack", TrackType::Audio),
    };
    let track_name = if clip_to_save.name.is_empty() {
        "Clip".to_owned()
    } else {
        clip_to_save.name.clone()
    };

    let track_node = tree.alloc_element_with(track_tag, vec![XmlAttr::new("Id", "1")], Some(false));
    tree.append_child(tracks_node, track_node);

    let name_node = tree.alloc_element("Name");
    tree.append_child(track_node, name_node);
    append_value_child(&mut tree, name_node, "EffectiveName", &track_name);
    append_value_child(&mut tree, name_node, "UserName", "");

    let dc_node = tree.alloc_element("DeviceChain");
    tree.append_child(track_node, dc_node);
    let main_seq = tree.alloc_element("MainSequencer");
    tree.append_child(dc_node, main_seq);

    let mut track = Track::new(TrackId(1), track_name, track_type, 0);
    track.clip_slots = vec![ClipSlot::with_clip(0, clip_to_save.clone())];
    track.device_chain = live_clip.devices.clone();

    let mut entities = EntityMap::default();
    let mut ids = AlsIdAllocator::new(100);
    ids.reserve(1);
    ids.reserve(clip_to_save.id.0);
    for dev in &live_clip.devices {
        ids.reserve(dev.id.0);
    }
    for lane in &clip_to_save.automation_lanes {
        ids.reserve(lane.id.0);
    }

    devices::patch_track_devices(&mut tree, dc_node, None, &track, &mut entities, &mut ids)?;
    clips::patch_track_clips(&mut tree, main_seq, None, &track, &mut entities, &mut ids)?;

    if ids.next_id() > 100 {
        set_child_value(
            &mut tree,
            live_set,
            "NextPointeeId",
            ids.next_id().to_string(),
        );
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    write_als_gz(&tree, path)
}

/// Loads an Ableton Live Clip (`.alc`) file, returning its [`Clip`] and embedded device chain.
pub fn load_live_clip(path: &Path) -> Result<LiveClipFile, AlsError> {
    let doc = AlsDocument::open(path)?;
    let tracks = &doc.project().tracks;
    if tracks.is_empty() {
        return Err(AlsError::InvalidFormat(
            "Live Clip (.alc) contains no tracks".to_owned(),
        ));
    }

    for track in tracks {
        if let Some(clip) = track
            .clip_slots
            .iter()
            .find_map(|slot| slot.clip.clone())
            .or_else(|| track.arrangement_clips.first().map(|arr| arr.clip.clone()))
        {
            return Ok(LiveClipFile {
                clip,
                devices: track.device_chain.clone(),
            });
        }
    }

    Err(AlsError::InvalidFormat(
        "Live Clip (.alc) contains no clip".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::AlsParser;
    use crate::project_dir::create_project_layout;
    use livecraft_model::ids::{ClipId, DeviceId};
    use livecraft_model::model::{DeviceType, MidiNote};
    use proptest::prelude::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("livecraft_alc_unit_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn midi_live_clip_roundtrips_with_device_chain() {
        let dir = temp_dir("midi_roundtrip");
        let alc_path = dir.join("BassLine.alc");

        let mut clip = Clip::new_midi(ClipId(15), "Acid Bass", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(36, 0.0, 0.5, 110));
            mdata.notes.push(MidiNote::new(48, 1.5, 0.25, 95));
        }
        let mut op = Device::new(DeviceId(21), "Acid Synth", DeviceType::Operator);
        op.set_param("filter_cutoff", 1450.0);
        let mut sat = Device::new(DeviceId(22), "Drive", DeviceType::Saturator);
        sat.set_param("drive_db", 9.5);

        let lc = LiveClipFile {
            clip,
            devices: vec![op, sat],
        };
        save_live_clip(&lc, &alc_path, None).expect("save .alc");

        let xml = AlsParser::decompress_to_xml(&alc_path).expect("decompress .alc");
        assert!(xml.contains("<Ableton MajorVersion=\"5\""));
        assert!(xml.contains("<MidiTrack Id=\"1\">"));
        assert!(xml.contains("<MidiClip"));

        let loaded = load_live_clip(&alc_path).expect("load .alc");
        assert_eq!(loaded.clip.name, "Acid Bass");
        assert_eq!(loaded.devices.len(), 2);
        assert_eq!(loaded.devices[0].device_type, DeviceType::Operator);
        assert!((loaded.devices[0].get_param("filter_cutoff") - 1450.0).abs() < 1e-3);
        assert_eq!(loaded.devices[1].device_type, DeviceType::Saturator);
        assert!((loaded.devices[1].get_param("drive_db") - 9.5).abs() < 1e-3);

        if let ClipPayload::Midi(m) = &loaded.clip.payload {
            assert_eq!(m.notes.len(), 2);
            assert_eq!(m.notes[0].pitch, 36);
            assert_eq!(m.notes[1].pitch, 48);
        } else {
            panic!("expected MIDI clip payload");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audio_live_clip_copies_sample_readonly_when_requested() {
        let dir = temp_dir("audio_copy");
        let src_dir = dir.join("External");
        let proj_dir = dir.join("Project");
        fs::create_dir_all(&src_dir).expect("src dir");
        fs::create_dir_all(&proj_dir).expect("proj dir");

        let sample_src = src_dir.join("kick_loop.wav");
        let payload = b"RIFFdemo_wav_bytes";
        fs::write(&sample_src, payload).expect("write sample");

        let clip = Clip::new_audio(ClipId(33), "Kick Loop", sample_src.to_string_lossy(), 8.0);
        let alc_path = proj_dir.join("Clips").join("Kick Loop.alc");
        save_live_clip(
            &LiveClipFile {
                clip,
                devices: Vec::new(),
            },
            &alc_path,
            Some(&proj_dir),
        )
        .expect("save audio .alc");

        let copied = proj_dir
            .join("Samples")
            .join("Imported")
            .join("kick_loop.wav");
        assert!(
            copied.is_file(),
            "sample must be copied into Samples/Imported"
        );
        assert_eq!(fs::read(&copied).expect("read copied"), payload);
        assert_eq!(fs::read(&sample_src).expect("read original"), payload);

        let xml = AlsParser::decompress_to_xml(&alc_path).expect("decompress audio .alc");
        assert!(xml.contains("Samples/Imported/kick_loop.wav"));

        let loaded = load_live_clip(&alc_path).expect("load audio .alc");
        match &loaded.clip.payload {
            ClipPayload::Audio(a) => {
                assert!(a.file_path.contains("Samples/Imported/kick_loop.wav"));
                assert!(Path::new(&a.file_path).is_file());
            }
            other => panic!("expected audio payload, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_parent_traversal_dedups_collisions_and_preserves_relative_path() {
        let dir = temp_dir("security_and_rel");
        let proj = dir.join("Proj Project");
        create_project_layout(&proj).expect("layout");
        let ext_a = dir.join("ExtA/kick.wav");
        let ext_b = dir.join("ExtB/kick.wav");
        fs::create_dir_all(ext_a.parent().expect("parent a")).expect("mkdir a");
        fs::create_dir_all(ext_b.parent().expect("parent b")).expect("mkdir b");
        fs::write(&ext_a, b"RIFFaaaa").expect("write ext_a");
        fs::write(&ext_b, b"RIFFbbbb").expect("write ext_b");

        let mut clip_a = Clip::new_audio(ClipId(10), "Kick A", ext_a.to_string_lossy(), 4.0);
        let alc_a = proj.join("Clips/KickA.alc");
        save_live_clip(
            &LiveClipFile {
                clip: clip_a.clone(),
                devices: Vec::new(),
            },
            &alc_a,
            Some(&proj.join("..").join("Escape")),
        )
        .expect_err(".. traversal in project_dir must fail");

        save_live_clip(
            &LiveClipFile {
                clip: clip_a.clone(),
                devices: Vec::new(),
            },
            &alc_a,
            Some(&proj),
        )
        .expect("save Kick A");

        // Saving a different sample with the same filename must disambiguate, never overwrite.
        let clip_b = Clip::new_audio(ClipId(11), "Kick B", ext_b.to_string_lossy(), 4.0);
        let alc_b = proj.join("Clips/KickB.alc");
        save_live_clip(
            &LiveClipFile {
                clip: clip_b,
                devices: Vec::new(),
            },
            &alc_b,
            Some(&proj),
        )
        .expect("save Kick B");

        assert_eq!(
            fs::read(proj.join("Samples/Imported/kick.wav")).expect("read kick"),
            b"RIFFaaaa"
        );
        assert_eq!(
            fs::read(proj.join("Samples/Imported/kick_1.wav")).expect("read kick_1"),
            b"RIFFbbbb"
        );

        // Preserves explicit audio.relative_path even when copy_audio_into is None.
        if let ClipPayload::Audio(ref mut a) = clip_a.payload {
            a.relative_path = Some("Samples/Imported/kick.wav".to_owned());
        }
        let alc_rel = proj.join("Clips/KickRel.alc");
        save_live_clip(
            &LiveClipFile {
                clip: clip_a,
                devices: Vec::new(),
            },
            &alc_rel,
            None,
        )
        .expect("save without copy");
        let loaded_rel = load_live_clip(&alc_rel).expect("load preserved rel");
        match loaded_rel.clip.payload {
            ClipPayload::Audio(a) => {
                assert_eq!(
                    a.relative_path.as_deref(),
                    Some("Samples/Imported/kick.wav")
                );
            }
            other => panic!("expected audio clip, got {other:?}"),
        }

        // save_live_clip_in_context resolves relative path in source context and copies to target.
        let src_proj = dir.join("Source Project");
        create_project_layout(&src_proj).expect("src layout");
        let src_wav = src_proj.join("Samples/Recorded/vox.wav");
        fs::write(&src_wav, b"RIFFvox!").expect("write vox");
        let mut rel_clip = Clip::new_audio(ClipId(12), "Vox", "", 4.0);
        if let ClipPayload::Audio(ref mut a) = rel_clip.payload {
            a.relative_path = Some("Samples/Recorded/vox.wav".to_owned());
        }
        let dst_proj = dir.join("Dest Project");
        create_project_layout(&dst_proj).expect("dst layout");
        let dst_alc = dst_proj.join("Clips/Vox.alc");
        let src_ctx = ProjectContext::for_project(&src_proj);
        save_live_clip_in_context(
            &LiveClipFile {
                clip: rel_clip,
                devices: Vec::new(),
            },
            &dst_alc,
            Some(&dst_proj),
            Some(&src_ctx),
        )
        .expect("save in context");
        assert!(dst_proj.join("Samples/Imported/vox.wav").is_file());
        let loaded_ctx = load_live_clip(&dst_alc).expect("load ctx alc");
        match loaded_ctx.clip.payload {
            ClipPayload::Audio(a) => {
                assert_eq!(a.relative_path.as_deref(), Some("Samples/Imported/vox.wav"));
            }
            other => panic!("expected audio clip, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn arbitrary_bytes_never_panic_in_load_live_clip(
            bytes in proptest::collection::vec(any::<u8>(), 0..1024)
        ) {
            let path = std::env::temp_dir().join(format!(
                "livecraft_alc_prop_{}.alc",
                std::process::id()
            ));
            let _ = fs::write(&path, &bytes);
            let _ = load_live_clip(&path);
            let _ = fs::remove_file(&path);
        }
    }
}
