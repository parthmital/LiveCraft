//! Mappers for Ableton `<AudioClip>` payloads (`SampleRef`, `FileRef`, `WarpMarkers`, gain, pitch).

use std::collections::HashSet;

use livecraft_model::ids::TrackId;
use livecraft_model::model::{AudioClipData, Clip, ClipPayload, WarpMarker, WarpMode};

use super::super::tree::{XmlAttr, XmlNodeId, XmlTree};
use super::automation;
use super::clips::{alloc_unique_clip_id, default_audio_color, read_clip_common};
use super::{
    AlsIdAllocator, EntityMap, bool_str, child_value_str, db_to_linear, f64_eq, format_f32,
    format_f64, linear_to_db, parse_child_bool, parse_child_f32, parse_child_f64, parse_child_i64,
    parse_child_u64, set_child_value,
};

pub(super) fn read_audio_clip(
    tree: &XmlTree,
    node: XmlNodeId,
    track_id: TrackId,
    slot_idx: u64,
    used: &mut HashSet<u64>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Clip {
    let clip_id = alloc_unique_clip_id(tree, node, 2000 + slot_idx, used, ids);
    entities.clips.insert(clip_id, node);
    let f = read_clip_common(tree, node, "Audio", default_audio_color());

    let sample_ref = tree.child(node, "SampleRef");
    let file_ref = sample_ref
        .and_then(|sr| tree.child(sr, "FileRef"))
        .or_else(|| tree.child(node, "FileRef"));
    let path_val =
        file_ref.and_then(|fr| child_value_str(tree, fr, "Path").filter(|s| !s.is_empty()));
    let rel_val =
        file_ref.and_then(|fr| child_value_str(tree, fr, "RelativePath").filter(|s| !s.is_empty()));
    let file_path = path_val.or(rel_val).unwrap_or("").to_owned();
    let relative_path = rel_val.map(str::to_owned);

    let is_warped = parse_child_bool(tree, node, "IsWarped").unwrap_or(true);
    let warp_mode = match parse_child_u64(tree, node, "WarpMode").unwrap_or(0) {
        1 => WarpMode::Tones,
        2 => WarpMode::Texture,
        3 => WarpMode::RePitch,
        4 => WarpMode::Complex,
        5 | 6 => WarpMode::ComplexPro,
        _ => WarpMode::Beats,
    };

    let mut warp_markers = Vec::new();
    if let Some(wms) = tree.child(node, "WarpMarkers") {
        for wm in tree.children_named(wms, "WarpMarker") {
            let sample_time_sec = tree
                .attr(wm, "SecTime")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let beat_time = tree
                .attr(wm, "BeatTime")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            warp_markers.push(WarpMarker {
                sample_time_sec,
                beat_time,
            });
        }
    }

    let gain_db = parse_child_f32(tree, node, "SampleVolume")
        .or_else(|| parse_child_f32(tree, node, "SampleVolumeModulation"))
        .or_else(|| parse_child_f32(tree, node, "Gain"))
        .map(linear_to_db)
        .unwrap_or(0.0);

    let pitch_coarse = parse_child_i64(tree, node, "PitchCoarse").unwrap_or(0) as i32;
    let pitch_fine = parse_child_i64(tree, node, "PitchFine").unwrap_or(0) as i32;
    let pitch_cents = pitch_coarse.saturating_mul(100).saturating_add(pitch_fine);

    let sample_rate = sample_ref
        .and_then(|sr| parse_child_u64(tree, sr, "DefaultSampleRate"))
        .map(|v| v as u32)
        .unwrap_or(44100);
    let meta = tree.child(node, "LiveCraftAudioMeta");
    let channels = meta
        .and_then(|m| parse_child_u64(tree, m, "Channels"))
        .map(|v| v as u16)
        .unwrap_or(2);
    let original_bpm = meta
        .and_then(|m| parse_child_f64(tree, m, "OriginalBpm"))
        .unwrap_or(120.0);

    let automation_lanes = automation::read_clip_automation(tree, node, track_id, entities, ids);

    Clip {
        id: clip_id,
        name: f.name,
        color: f.color,
        length_beats: f.length_beats,
        loop_start_beat: f.loop_start_beat,
        loop_end_beat: f.loop_end_beat,
        is_looping: f.is_looping,
        start_marker_beat: f.start_marker_beat,
        end_marker_beat: f.end_marker_beat,
        payload: ClipPayload::Audio(AudioClipData {
            file_path,
            relative_path,
            sample_rate,
            channels,
            warp_mode,
            is_warped,
            original_bpm,
            warp_markers,
            gain_db,
            pitch_cents,
        }),
        automation_lanes,
        launch: f.launch,
        follow: f.follow,
    }
}

fn ensure_audio_fileref(tree: &mut XmlTree, clip_node: XmlNodeId) -> Option<XmlNodeId> {
    if tree.child(clip_node, "SampleRef").is_none()
        && let Some(direct) = tree.child(clip_node, "FileRef")
    {
        return Some(direct);
    }
    tree.ensure_child(clip_node, "SampleRef")
        .and_then(|sr| tree.ensure_child(sr, "FileRef"))
}

pub(super) fn patch_audio_clip_payload(
    tree: &mut XmlTree,
    clip_node: XmlNodeId,
    prev: Option<&AudioClipData>,
    cur: &AudioClipData,
    ids: &mut AlsIdAllocator,
) {
    if prev.is_none_or(|p| p.file_path != cur.file_path)
        && let Some(fr) = ensure_audio_fileref(tree, clip_node)
    {
        set_child_value(tree, fr, "Path", &cur.file_path);
    }
    if prev.is_none_or(|p| p.relative_path != cur.relative_path) {
        match &cur.relative_path {
            Some(rel) => {
                if let Some(fr) = ensure_audio_fileref(tree, clip_node) {
                    set_child_value(tree, fr, "RelativePathType", "3");
                    set_child_value(tree, fr, "RelativePath", rel);
                }
            }
            None => {
                if prev.is_some_and(|p| p.relative_path.is_some())
                    && let Some(fr) = tree
                        .child(clip_node, "SampleRef")
                        .and_then(|sr| tree.child(sr, "FileRef"))
                        .or_else(|| tree.child(clip_node, "FileRef"))
                {
                    if tree.child(fr, "RelativePath").is_some() {
                        set_child_value(tree, fr, "RelativePath", "");
                    }
                    if tree.child(fr, "RelativePathType").is_some() {
                        set_child_value(tree, fr, "RelativePathType", "0");
                    }
                }
            }
        }
    }
    if prev.is_none_or(|p| p.is_warped != cur.is_warped) {
        set_child_value(tree, clip_node, "IsWarped", bool_str(cur.is_warped));
    }
    if prev.is_none_or(|p| p.warp_mode != cur.warp_mode) {
        let code = match cur.warp_mode {
            WarpMode::Beats => "0",
            WarpMode::Tones => "1",
            WarpMode::Texture => "2",
            WarpMode::RePitch => "3",
            WarpMode::Complex => "4",
            WarpMode::ComplexPro => "6",
        };
        set_child_value(tree, clip_node, "WarpMode", code);
    }

    let markers_changed = prev.is_none_or(|p| {
        p.warp_markers.len() != cur.warp_markers.len()
            || p.warp_markers.iter().zip(&cur.warp_markers).any(|(a, b)| {
                !f64_eq(a.sample_time_sec, b.sample_time_sec) || !f64_eq(a.beat_time, b.beat_time)
            })
    });
    if markers_changed
        && (!cur.warp_markers.is_empty() || tree.child(clip_node, "WarpMarkers").is_some())
        && let Some(wms) = tree.ensure_child(clip_node, "WarpMarkers")
    {
        tree.clear_children(wms);
        for wm in &cur.warp_markers {
            let el = tree.alloc_element_with(
                "WarpMarker",
                vec![
                    XmlAttr::new("Id", ids.alloc().to_string()),
                    XmlAttr::new("SecTime", format_f64(wm.sample_time_sec)),
                    XmlAttr::new("BeatTime", format_f64(wm.beat_time)),
                ],
                Some(true),
            );
            tree.append_child(wms, el);
        }
    }

    if prev.is_none_or(|p| p.gain_db != cur.gain_db) {
        let lin = format_f32(db_to_linear(cur.gain_db));
        if tree.child(clip_node, "SampleVolume").is_some() {
            set_child_value(tree, clip_node, "SampleVolume", &lin);
        } else {
            set_child_value(tree, clip_node, "SampleVolumeModulation", &lin);
        }
    }

    if prev.is_none_or(|p| p.pitch_cents != cur.pitch_cents) {
        set_child_value(
            tree,
            clip_node,
            "PitchCoarse",
            (cur.pitch_cents / 100).to_string(),
        );
        set_child_value(
            tree,
            clip_node,
            "PitchFine",
            (cur.pitch_cents % 100).to_string(),
        );
    }

    if prev.is_none_or(|p| p.sample_rate != cur.sample_rate)
        && (cur.sample_rate != 44100
            || tree
                .child(clip_node, "SampleRef")
                .and_then(|sr| tree.child(sr, "DefaultSampleRate"))
                .is_some())
        && let Some(sr) = tree.ensure_child(clip_node, "SampleRef")
    {
        set_child_value(tree, sr, "DefaultSampleRate", cur.sample_rate.to_string());
    }

    let needs_meta = cur.channels != 2 || !f64_eq(cur.original_bpm, 120.0);
    if prev.is_none_or(|p| p.channels != cur.channels || !f64_eq(p.original_bpm, cur.original_bpm))
    {
        if !needs_meta {
            if let Some(old) = tree.child(clip_node, "LiveCraftAudioMeta") {
                tree.remove_child(clip_node, old);
            }
        } else if let Some(meta) = tree.ensure_child(clip_node, "LiveCraftAudioMeta") {
            set_child_value(tree, meta, "Channels", cur.channels.to_string());
            set_child_value(tree, meta, "OriginalBpm", format_f64(cur.original_bpm));
        }
    }
}
