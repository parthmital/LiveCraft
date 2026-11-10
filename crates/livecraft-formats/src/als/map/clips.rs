//! Mappers for Session `ClipSlotList`, Arrangement clips (`ClipTimeable` / `Sample`),
//! `MidiClip`, and `AudioClip` (`SampleRef`, `WarpMarkers`).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use livecraft_model::color::ABLETON_PALETTE;
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::{
    ArrangementClip, Clip, ClipLaunch, ClipPayload, ClipSlot, FollowActions, MidiClipData, Track,
    TrackType,
};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNode, XmlNodeId, XmlTree};
use super::audio_clips::{patch_audio_clip_payload, read_audio_clip};
use super::automation;
use super::notes::{
    patch_clip_follow_actions, patch_clip_launch, patch_midi_notes, read_clip_follow_actions,
    read_clip_launch, read_midi_notes,
};
use super::scenes::{color_from_xml, color_to_xml};
use super::{
    AlsIdAllocator, EntityMap, bool_str, child_value_str, clips_equal, f64_eq, format_f64,
    instantiate_template_cached, parse_child_bool, parse_child_f64, set_child_value,
    sync_ordered_children,
};

const MIDI_CLIP_TEMPLATE_XML: &str = include_str!("../../../templates/midi_clip.xml");
const AUDIO_CLIP_TEMPLATE_XML: &str = include_str!("../../../templates/audio_clip.xml");
static MIDI_CLIP_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
static AUDIO_CLIP_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();

#[must_use]
fn default_midi_color() -> &'static str {
    ABLETON_PALETTE[2]
}

#[must_use]
pub(super) fn default_audio_color() -> &'static str {
    ABLETON_PALETTE[10]
}

pub(crate) fn read_track_clips(
    tree: &XmlTree,
    track_id: TrackId,
    track_type: TrackType,
    main_seq: XmlNodeId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(Vec<ClipSlot>, Vec<ArrangementClip>), AlsError> {
    let mut used_clip_ids = HashSet::new();
    let mut clip_slots = Vec::new();

    if let Some(slot_list) = tree.child(main_seq, "ClipSlotList") {
        for (idx, slot_node) in tree
            .children_named(slot_list, "ClipSlot")
            .into_iter()
            .enumerate()
        {
            entities.clip_slots.insert((track_id, idx), slot_node);
            let slot_id = tree
                .attr(slot_node, "Id")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or_else(|| track_id.0.saturating_mul(1000).saturating_add(idx as u64));
            let has_stop_button = parse_child_bool(tree, slot_node, "HasStop").unwrap_or(true);
            let value_node = tree
                .child(slot_node, "ClipSlot")
                .and_then(|inner| tree.child(inner, "Value"))
                .or_else(|| tree.child(slot_node, "Value"));

            let clip = value_node.and_then(|vn| {
                if let Some(mc) = tree.child(vn, "MidiClip") {
                    Some(read_midi_clip(
                        tree,
                        mc,
                        track_id,
                        idx as u64,
                        &mut used_clip_ids,
                        entities,
                        ids,
                    ))
                } else {
                    tree.child(vn, "AudioClip").map(|ac| {
                        read_audio_clip(
                            tree,
                            ac,
                            track_id,
                            idx as u64,
                            &mut used_clip_ids,
                            entities,
                            ids,
                        )
                    })
                }
            });

            clip_slots.push(ClipSlot {
                id: slot_id,
                clip,
                has_stop_button,
            });
        }
    }

    let mut arrangement_clips = Vec::new();
    let arr_events_node = match track_type {
        TrackType::Midi => tree
            .child(main_seq, "ClipTimeable")
            .and_then(|ct| tree.child(ct, "ArrangerAutomation"))
            .and_then(|aa| tree.child(aa, "Events")),
        TrackType::Audio => tree
            .child(main_seq, "Sample")
            .and_then(|s| tree.child(s, "ArrangerAutomation"))
            .and_then(|aa| tree.child(aa, "Events")),
        _ => None,
    };

    if let Some(events_node) = arr_events_node {
        for (idx, &child) in tree.children(events_node).iter().enumerate() {
            let Some(tag) = tree.node(child).and_then(XmlNode::element_name) else {
                continue;
            };
            let clip = match tag {
                "MidiClip" => read_midi_clip(
                    tree,
                    child,
                    track_id,
                    idx as u64,
                    &mut used_clip_ids,
                    entities,
                    ids,
                ),
                "AudioClip" => read_audio_clip(
                    tree,
                    child,
                    track_id,
                    idx as u64,
                    &mut used_clip_ids,
                    entities,
                    ids,
                ),
                _ => continue,
            };
            let start_beat = tree
                .attr(child, "Time")
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| parse_child_f64(tree, child, "CurrentStart"))
                .unwrap_or(0.0);
            let length_beats = clip.length_beats;
            arrangement_clips.push(ArrangementClip::new(
                clip.id,
                clip,
                start_beat,
                length_beats,
            ));
        }
    }

    Ok((clip_slots, arrangement_clips))
}

pub(super) fn alloc_unique_clip_id(
    tree: &XmlTree,
    clip_node: XmlNodeId,
    fallback: u64,
    used: &mut HashSet<u64>,
    ids: &mut AlsIdAllocator,
) -> ClipId {
    let parsed = tree
        .attr(clip_node, "Id")
        .and_then(|s| s.parse::<u64>().ok());
    let id = match parsed {
        Some(v) if !used.contains(&v) => v,
        _ => {
            let mut candidate = fallback.max(1);
            while used.contains(&candidate) {
                candidate = ids.alloc();
            }
            candidate
        }
    };
    used.insert(id);
    ClipId(id)
}

pub(super) struct CommonClipFields {
    pub(super) name: String,
    pub(super) color: String,
    pub(super) length_beats: f64,
    pub(super) loop_start_beat: f64,
    pub(super) loop_end_beat: f64,
    pub(super) is_looping: bool,
    pub(super) start_marker_beat: f64,
    pub(super) end_marker_beat: f64,
    pub(super) launch: ClipLaunch,
    pub(super) follow: FollowActions,
}

pub(super) fn read_clip_common(
    tree: &XmlTree,
    node: XmlNodeId,
    default_name: &str,
    default_color: &str,
) -> CommonClipFields {
    let name = read_clip_name(tree, node).unwrap_or_else(|| default_name.to_owned());
    let color = child_value_str(tree, node, "Color")
        .map(|v| color_from_xml(v, default_color))
        .unwrap_or_else(|| default_color.to_owned());

    let current_start = parse_child_f64(tree, node, "CurrentStart");
    let current_end = parse_child_f64(tree, node, "CurrentEnd");
    let loop_node = tree.child(node, "Loop");
    let loop_start_beat = loop_node
        .and_then(|ln| parse_child_f64(tree, ln, "LoopStart"))
        .unwrap_or(0.0);
    let loop_end_opt = loop_node.and_then(|ln| parse_child_f64(tree, ln, "LoopEnd"));
    let is_looping = loop_node
        .and_then(|ln| parse_child_bool(tree, ln, "LoopOn"))
        .unwrap_or(true);

    let span = match (current_start, current_end) {
        (Some(s), Some(e)) if e > s => Some(e - s),
        (None, Some(e)) if e > 0.0 => Some(e),
        _ => None,
    };
    let length_beats = span
        .or_else(|| {
            loop_end_opt
                .map(|e| (e - loop_start_beat).max(0.0))
                .filter(|&l| l > 0.0)
        })
        .unwrap_or(4.0);
    let loop_end_beat = loop_end_opt.unwrap_or(length_beats);
    let start_marker_beat = loop_node
        .and_then(|ln| parse_child_f64(tree, ln, "StartRelative"))
        .map(|sr| loop_start_beat + sr)
        .unwrap_or(0.0);
    let end_marker_beat = loop_node
        .and_then(|ln| parse_child_f64(tree, ln, "OutMarker"))
        .unwrap_or(start_marker_beat + length_beats);

    CommonClipFields {
        name,
        color,
        length_beats,
        loop_start_beat,
        loop_end_beat,
        is_looping,
        start_marker_beat,
        end_marker_beat,
        launch: read_clip_launch(tree, node),
        follow: read_clip_follow_actions(tree, node),
    }
}

fn read_clip_name(tree: &XmlTree, node: XmlNodeId) -> Option<String> {
    let name_node = tree.child(node, "Name")?;
    if let Some(v) = tree.value_attr(name_node)
        && !v.is_empty()
    {
        return Some(v.to_owned());
    }
    if let Some(u) = child_value_str(tree, name_node, "UserName")
        && !u.is_empty()
    {
        return Some(u.to_owned());
    }
    if let Some(e) = child_value_str(tree, name_node, "EffectiveName")
        && !e.is_empty()
    {
        return Some(e.to_owned());
    }
    (tree.value_attr(name_node) == Some("")).then(String::new)
}

fn read_midi_clip(
    tree: &XmlTree,
    node: XmlNodeId,
    track_id: TrackId,
    slot_idx: u64,
    used: &mut HashSet<u64>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Clip {
    let clip_id = alloc_unique_clip_id(tree, node, 1000 + slot_idx, used, ids);
    entities.clips.insert(clip_id, node);
    let f = read_clip_common(tree, node, "Clip", default_midi_color());
    let notes = read_midi_notes(tree, node);
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
        payload: ClipPayload::Midi(MidiClipData { notes }),
        automation_lanes,
        launch: f.launch,
        follow: f.follow,
    }
}

fn instantiate_clip_template(
    tree: &mut XmlTree,
    payload: &ClipPayload,
) -> Result<XmlNodeId, AlsError> {
    match payload {
        ClipPayload::Midi(_) => {
            instantiate_template_cached(tree, &MIDI_CLIP_CACHE, MIDI_CLIP_TEMPLATE_XML)
        }
        ClipPayload::Audio(_) => {
            instantiate_template_cached(tree, &AUDIO_CLIP_CACHE, AUDIO_CLIP_TEMPLATE_XML)
        }
    }
}

pub(crate) fn patch_track_clips(
    tree: &mut XmlTree,
    main_seq: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    patch_session_slots(tree, main_seq, prev, cur, entities, ids)?;
    patch_arrangement_clips(tree, main_seq, prev, cur, entities, ids)?;
    Ok(())
}

fn patch_session_slots(
    tree: &mut XmlTree,
    main_seq: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let slots_unchanged = prev.is_some_and(|p| {
        p.clip_slots.len() == cur.clip_slots.len()
            && p.clip_slots.iter().zip(&cur.clip_slots).all(|(a, b)| {
                a.id == b.id
                    && a.has_stop_button == b.has_stop_button
                    && match (&a.clip, &b.clip) {
                        (None, None) => true,
                        (Some(ca), Some(cb)) => clips_equal(ca, cb),
                        _ => false,
                    }
            })
    });
    if slots_unchanged {
        return Ok(());
    }
    if cur.clip_slots.is_empty() && tree.child(main_seq, "ClipSlotList").is_none() {
        return Ok(());
    }
    let Some(slot_list) = tree.ensure_child(main_seq, "ClipSlotList") else {
        return Ok(());
    };

    let existing_slots = tree.children_named(slot_list, "ClipSlot");
    let mut desired = Vec::with_capacity(cur.clip_slots.len());

    for (idx, slot) in cur.clip_slots.iter().enumerate() {
        let prev_slot = prev.and_then(|p| p.clip_slots.get(idx));
        let slot_node = if let Some(&existing) = existing_slots.get(idx) {
            if prev_slot.is_none_or(|ps| ps.id != slot.id) {
                tree.set_attr(existing, "Id", slot.id.to_string());
            }
            if prev_slot.is_none_or(|ps| ps.has_stop_button != slot.has_stop_button) {
                set_child_value(tree, existing, "HasStop", bool_str(slot.has_stop_button));
            }
            patch_slot_clip(
                tree,
                existing,
                prev_slot.and_then(|ps| ps.clip.as_ref()),
                slot.clip.as_ref(),
                cur.id,
                entities,
                ids,
            )?;
            existing
        } else {
            let s = tree.alloc_element_with(
                "ClipSlot",
                vec![XmlAttr::new("Id", slot.id.to_string())],
                Some(false),
            );
            set_child_value(tree, s, "LomId", "0");
            if let Some(inner) = tree.ensure_child(s, "ClipSlot") {
                let _ = tree.ensure_child(inner, "Value");
            }
            set_child_value(tree, s, "HasStop", bool_str(slot.has_stop_button));
            patch_slot_clip(tree, s, None, slot.clip.as_ref(), cur.id, entities, ids)?;
            s
        };
        entities.clip_slots.insert((cur.id, idx), slot_node);
        desired.push(slot_node);
    }

    sync_ordered_children(
        tree,
        slot_list,
        |n| n.element_name() == Some("ClipSlot"),
        desired,
    );
    Ok(())
}

fn patch_slot_clip(
    tree: &mut XmlTree,
    slot_node: XmlNodeId,
    prev_clip: Option<&Clip>,
    cur_clip: Option<&Clip>,
    track_id: TrackId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    if let (Some(a), Some(b)) = (prev_clip, cur_clip)
        && clips_equal(a, b)
    {
        return Ok(());
    }
    if prev_clip.is_none() && cur_clip.is_none() {
        return Ok(());
    }

    let val_node = tree
        .child(slot_node, "ClipSlot")
        .and_then(|inner| tree.ensure_child(inner, "Value"))
        .or_else(|| {
            tree.child(slot_node, "Value").or_else(|| {
                tree.ensure_child(slot_node, "ClipSlot")
                    .and_then(|i| tree.ensure_child(i, "Value"))
            })
        })
        .ok_or_else(|| AlsError::InvalidFormat("failed to create ClipSlot/Value".to_owned()))?;

    let Some(clip) = cur_clip else {
        tree.clear_children(val_node);
        if let Some(old) = prev_clip {
            entities.clips.remove(&old.id);
        }
        return Ok(());
    };

    let expected_tag = match clip.payload {
        ClipPayload::Midi(_) => "MidiClip",
        ClipPayload::Audio(_) => "AudioClip",
    };
    let existing_node = entities.clips.get(&clip.id).copied().filter(|&nid| {
        tree.node(nid)
            .is_some_and(|n| n.parent == Some(val_node) && n.element_name() == Some(expected_tag))
    });

    let clip_node = if let Some(nid) = existing_node {
        patch_clip_node(
            tree, nid, prev_clip, clip, 0.0, false, track_id, entities, ids,
        )?;
        nid
    } else {
        tree.clear_children(val_node);
        let fresh = instantiate_clip_template(tree, &clip.payload)?;
        tree.set_attr(fresh, "Id", clip.id.0.to_string());
        tree.set_attr(fresh, "Time", "0");
        tree.append_child(val_node, fresh);
        patch_clip_node(tree, fresh, None, clip, 0.0, false, track_id, entities, ids)?;
        fresh
    };
    entities.clips.insert(clip.id, clip_node);
    Ok(())
}

fn patch_arrangement_clips(
    tree: &mut XmlTree,
    main_seq: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let unchanged = prev.is_some_and(|p| {
        p.arrangement_clips.len() == cur.arrangement_clips.len()
            && p.arrangement_clips
                .iter()
                .zip(&cur.arrangement_clips)
                .all(|(a, b)| {
                    a.id == b.id
                        && f64_eq(a.start_beat, b.start_beat)
                        && f64_eq(a.length_beats, b.length_beats)
                        && clips_equal(&a.clip, &b.clip)
                })
    });
    if unchanged {
        return Ok(());
    }

    let container_name = match cur.track_type {
        TrackType::Midi => "ClipTimeable",
        TrackType::Audio => "Sample",
        _ => return Ok(()),
    };
    if cur.arrangement_clips.is_empty() && tree.child(main_seq, container_name).is_none() {
        return Ok(());
    }
    let Some(events_node) = tree
        .ensure_child(main_seq, container_name)
        .and_then(|c| tree.ensure_child(c, "ArrangerAutomation"))
        .and_then(|aa| tree.ensure_child(aa, "Events"))
    else {
        return Ok(());
    };

    let prev_by_id: HashMap<ClipId, &ArrangementClip> = prev
        .map(|p| p.arrangement_clips.iter().map(|a| (a.id, a)).collect())
        .unwrap_or_default();
    let mut desired = Vec::with_capacity(cur.arrangement_clips.len());

    for arr in &cur.arrangement_clips {
        let prev_arr = prev_by_id.get(&arr.id).copied();
        let expected_tag = match arr.clip.payload {
            ClipPayload::Midi(_) => "MidiClip",
            ClipPayload::Audio(_) => "AudioClip",
        };
        let existing = entities.clips.get(&arr.clip.id).copied().filter(|&nid| {
            tree.node(nid).is_some_and(|n| {
                n.parent == Some(events_node) && n.element_name() == Some(expected_tag)
            })
        });

        let clip_node = if let Some(nid) = existing {
            if prev_arr.is_none_or(|pa| {
                !f64_eq(pa.start_beat, arr.start_beat)
                    || !f64_eq(pa.length_beats, arr.length_beats)
                    || !clips_equal(&pa.clip, &arr.clip)
            }) {
                patch_clip_node(
                    tree,
                    nid,
                    prev_arr.map(|pa| &pa.clip),
                    &arr.clip,
                    arr.start_beat,
                    true,
                    cur.id,
                    entities,
                    ids,
                )?;
            }
            nid
        } else {
            let fresh = instantiate_clip_template(tree, &arr.clip.payload)?;
            tree.set_attr(fresh, "Id", arr.clip.id.0.to_string());
            patch_clip_node(
                tree,
                fresh,
                None,
                &arr.clip,
                arr.start_beat,
                true,
                cur.id,
                entities,
                ids,
            )?;
            fresh
        };
        entities.clips.insert(arr.clip.id, clip_node);
        desired.push(clip_node);
    }

    sync_ordered_children(
        tree,
        events_node,
        |n| matches!(n.element_name(), Some("MidiClip" | "AudioClip")),
        desired,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn patch_clip_node(
    tree: &mut XmlTree,
    node: XmlNodeId,
    prev: Option<&Clip>,
    cur: &Clip,
    start_beat: f64,
    is_arrangement: bool,
    track_id: TrackId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    if is_arrangement {
        tree.set_attr(node, "Time", format_f64(start_beat));
    }

    if prev.is_none_or(|p| p.name != cur.name)
        && let Some(name_node) = tree.ensure_child(node, "Name")
    {
        if tree.child(name_node, "UserName").is_some()
            || tree.child(name_node, "EffectiveName").is_some()
        {
            set_child_value(tree, name_node, "UserName", &cur.name);
        } else {
            tree.set_value_attr(name_node, &cur.name);
        }
    }

    let def_color = match cur.payload {
        ClipPayload::Midi(_) => default_midi_color(),
        ClipPayload::Audio(_) => default_audio_color(),
    };
    if prev.is_none_or(|p| p.color != cur.color) {
        set_child_value(tree, node, "Color", color_to_xml(&cur.color, def_color));
    }

    let cur_start = if is_arrangement { start_beat } else { 0.0 };
    let cur_end = cur_start + cur.length_beats;
    if prev.is_none_or(|p| !f64_eq(p.length_beats, cur.length_beats)) || is_arrangement {
        set_child_value(tree, node, "CurrentStart", format_f64(cur_start));
        set_child_value(tree, node, "CurrentEnd", format_f64(cur_end));
    }

    let loop_changed = prev.is_none_or(|p| {
        !f64_eq(p.loop_start_beat, cur.loop_start_beat)
            || !f64_eq(p.loop_end_beat, cur.loop_end_beat)
            || p.is_looping != cur.is_looping
            || !f64_eq(p.start_marker_beat, cur.start_marker_beat)
            || !f64_eq(p.end_marker_beat, cur.end_marker_beat)
    });
    if loop_changed && let Some(ln) = tree.ensure_child(node, "Loop") {
        set_child_value(tree, ln, "LoopStart", format_f64(cur.loop_start_beat));
        set_child_value(tree, ln, "LoopEnd", format_f64(cur.loop_end_beat));
        set_child_value(
            tree,
            ln,
            "StartRelative",
            format_f64(cur.start_marker_beat - cur.loop_start_beat),
        );
        set_child_value(tree, ln, "LoopOn", bool_str(cur.is_looping));
        set_child_value(tree, ln, "OutMarker", format_f64(cur.end_marker_beat));
    }

    if prev.is_none_or(|p| p.launch != cur.launch)
        && (cur.launch != ClipLaunch::default()
            || tree.child(node, "LaunchMode").is_some()
            || tree.child(node, "LaunchQuantisation").is_some()
            || tree.child(node, "Legato").is_some()
            || tree.child(node, "VelocityAmount").is_some())
    {
        patch_clip_launch(tree, node, &cur.launch);
    }

    if prev.is_none_or(|p| p.follow != cur.follow)
        && (cur.follow != FollowActions::default() || tree.child(node, "FollowAction").is_some())
    {
        patch_clip_follow_actions(tree, node, &cur.follow);
    }

    match &cur.payload {
        ClipPayload::Midi(mdata) => {
            let prev_midi = prev.and_then(|p| match &p.payload {
                ClipPayload::Midi(m) => Some(m),
                ClipPayload::Audio(_) => None,
            });
            patch_midi_notes(tree, node, prev_midi, mdata, ids);
        }
        ClipPayload::Audio(adata) => {
            let prev_audio = prev.and_then(|p| match &p.payload {
                ClipPayload::Audio(a) => Some(a),
                ClipPayload::Midi(_) => None,
            });
            patch_audio_clip_payload(tree, node, prev_audio, adata, ids);
        }
    }

    automation::patch_clip_automation(tree, node, track_id, prev, cur, entities, ids)?;
    Ok(())
}
