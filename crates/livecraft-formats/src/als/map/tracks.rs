//! Mappers for Ableton `<Tracks>`, `<MidiTrack>`, `<AudioTrack>`, `<GroupTrack>`,
//! `<ReturnTrack>`, `<MainTrack>`/`<MasterTrack>`, `<Mixer>`, and `<*Routing>`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use livecraft_model::color::ABLETON_PALETTE;
use livecraft_model::ids::TrackId;
use livecraft_model::model::{MonitoringMode, Project, Track, TrackType};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNode, XmlNodeId, XmlTree};
use super::automation;
use super::clips;
use super::devices;
use super::routing;
use super::scenes::{color_from_xml, color_to_xml};
use super::{
    AlsIdAllocator, EntityMap, PointeeTarget, append_value_child, bool_str, child_value_str,
    db_to_linear, element_has_name, f32_eq, format_f32, instantiate_template_cached, linear_to_db,
    manual_value_str, parse_child_bool, parse_child_i64, parse_child_u64, set_child_value,
    set_manual_value, sync_ordered_children, tracks_equal,
};

const MIDI_TRACK_TEMPLATE_XML: &str = include_str!("../../../templates/midi_track.xml");
const AUDIO_TRACK_TEMPLATE_XML: &str = include_str!("../../../templates/audio_track.xml");
const GROUP_TRACK_TEMPLATE_XML: &str = include_str!("../../../templates/group_track.xml");
const RETURN_TRACK_TEMPLATE_XML: &str = include_str!("../../../templates/return_track.xml");
static MIDI_TRACK_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
static AUDIO_TRACK_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
static GROUP_TRACK_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
static RETURN_TRACK_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) struct ParsedTracks {
    pub tracks: Vec<Track>,
    pub return_tracks: Vec<Track>,
    pub main_track: Track,
}

#[must_use]
pub(crate) fn default_track_color(track_type: TrackType) -> &'static str {
    match track_type {
        TrackType::Audio => ABLETON_PALETTE[10],
        TrackType::Midi => ABLETON_PALETTE[2],
        TrackType::Return => ABLETON_PALETTE[14],
        TrackType::Group | TrackType::Main => "#888888",
    }
}

/// Locates `<MainTrack>` or `<MasterTrack>` under `<LiveSet>` (or `<LiveSet>/<Tracks>` fallback).
#[must_use]
pub(crate) fn find_main_track_node(tree: &XmlTree, live_set: XmlNodeId) -> Option<XmlNodeId> {
    tree.child(live_set, "MainTrack")
        .or_else(|| tree.child(live_set, "MasterTrack"))
        .or_else(|| {
            tree.child(live_set, "Tracks").and_then(|tracks| {
                tree.child(tracks, "MainTrack")
                    .or_else(|| tree.child(tracks, "MasterTrack"))
            })
        })
}

pub(crate) fn read_all_tracks(
    tree: &XmlTree,
    live_set: XmlNodeId,
    main_track_node: Option<XmlNodeId>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<ParsedTracks, AlsError> {
    let mut tracks = Vec::new();
    let mut return_tracks = Vec::new();
    let mut used_ids = HashSet::new();

    if let Some(tracks_node) = tree.child(live_set, "Tracks") {
        for &child in tree.children(tracks_node) {
            let Some(tag) = tree.node(child).and_then(XmlNode::element_name) else {
                continue;
            };
            let track_type = match tag {
                "MidiTrack" => TrackType::Midi,
                "AudioTrack" => TrackType::Audio,
                "GroupTrack" => TrackType::Group,
                "ReturnTrack" => TrackType::Return,
                _ => continue,
            };
            let track = read_single_track(
                tree,
                child,
                track_type,
                tracks.len(),
                return_tracks.len(),
                &mut used_ids,
                entities,
                ids,
            )?;
            if track_type == TrackType::Return {
                return_tracks.push(track);
            } else {
                tracks.push(track);
            }
        }
    }

    let main_track = if let Some(mt_node) = main_track_node {
        entities.main_track = Some(mt_node);
        entities.main_track_had_id_attr = tree.attr(mt_node, "Id").is_some();
        let mt = read_single_track(
            tree,
            mt_node,
            TrackType::Main,
            0,
            0,
            &mut used_ids,
            entities,
            ids,
        )?;
        entities.tracks.remove(&mt.id);
        mt
    } else {
        let mut id_num = 999u64;
        while used_ids.contains(&id_num) {
            id_num += 1;
        }
        used_ids.insert(id_num);
        Track::new(TrackId(id_num), "Master", TrackType::Main, 0)
    };

    let return_count = return_tracks.len();
    for trk in tracks.iter_mut().chain(return_tracks.iter_mut()) {
        if trk.sends.len() < return_count {
            trk.sends.resize(return_count, 0.0);
        }
    }
    let mut main_track = main_track;
    if main_track.sends.len() != return_count {
        main_track.sends.resize(return_count, 0.0);
    }

    Ok(ParsedTracks {
        tracks,
        return_tracks,
        main_track,
    })
}

#[allow(clippy::too_many_arguments)]
fn read_single_track(
    tree: &XmlTree,
    node: XmlNodeId,
    track_type: TrackType,
    track_idx: usize,
    return_idx: usize,
    used_ids: &mut HashSet<u64>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<Track, AlsError> {
    let parsed_id = tree.attr(node, "Id").and_then(|s| s.parse::<u64>().ok());
    let id_num = match parsed_id {
        Some(v) if !used_ids.contains(&v) => {
            used_ids.insert(v);
            v
        }
        _ => {
            let mut candidate = match track_type {
                TrackType::Return => return_idx as u64 + 101,
                TrackType::Main => 999,
                _ => track_idx as u64 + 1,
            };
            while used_ids.contains(&candidate) {
                candidate = ids.alloc();
            }
            used_ids.insert(candidate);
            candidate
        }
    };
    let track_id = TrackId(id_num);
    if track_type != TrackType::Main {
        entities.tracks.insert(track_id, node);
    }

    let fallback_name = match track_type {
        TrackType::Midi => format!("{id_num}-MIDI"),
        TrackType::Audio => format!("{id_num}-Audio"),
        TrackType::Group => format!("{id_num}-Group"),
        TrackType::Return => {
            let letter = (b'A' + (return_idx as u8 % 26)) as char;
            format!("{letter}-Return")
        }
        TrackType::Main => "Master".to_owned(),
    };
    let name = read_track_name(tree, node).unwrap_or(fallback_name);
    let def_color = default_track_color(track_type);
    let color = child_value_str(tree, node, "Color")
        .map(|v| color_from_xml(v, def_color))
        .unwrap_or_else(|| def_color.to_owned());

    let parent = parse_child_i64(tree, node, "TrackGroupId")
        .filter(|&v| v >= 0)
        .map(|v| TrackId(v as u64));

    let dc = tree.child(node, "DeviceChain");
    let mixer = dc.and_then(|d| tree.child(d, "Mixer"));
    let main_seq = dc.and_then(|d| tree.child(d, "MainSequencer"));

    let monitoring = main_seq
        .and_then(|ms| parse_child_u64(tree, ms, "MonitoringEnum"))
        .map(|v| match v {
            0 => MonitoringMode::In,
            2 => MonitoringMode::Off,
            _ => MonitoringMode::Auto,
        })
        .unwrap_or(MonitoringMode::Auto);

    let is_armed = main_seq
        .and_then(|ms| tree.child(ms, "Recorder"))
        .and_then(|rec| parse_child_bool(tree, rec, "IsArmed"))
        .unwrap_or(false);

    let routing = dc
        .map(|d| routing::read_routing(tree, d))
        .unwrap_or_default();

    let (volume_db, pan, is_muted, is_soloed, sends) = if let Some(mx) = mixer {
        read_mixer(tree, mx, track_id, entities)
    } else {
        (0.0, 0.0, false, false, Vec::new())
    };

    let (clip_slots, arrangement_clips) = if let Some(ms) = main_seq {
        clips::read_track_clips(tree, track_id, track_type, ms, entities, ids)?
    } else {
        (Vec::new(), Vec::new())
    };

    let device_chain = if let Some(d) = dc {
        devices::read_track_devices(tree, track_id, d, entities, ids)?
    } else {
        Vec::new()
    };

    let automation_lanes = automation::read_track_automation(tree, node, track_id, entities, ids);

    Ok(Track {
        id: track_id,
        name,
        track_type,
        parent,
        routing,
        color,
        volume_db,
        pan,
        is_muted,
        is_soloed,
        is_armed,
        monitoring,
        sends,
        clip_slots,
        arrangement_clips,
        automation_lanes,
        device_chain,
    })
}

fn read_track_name(tree: &XmlTree, track_node: XmlNodeId) -> Option<String> {
    let name_node = tree.child(track_node, "Name")?;
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
    if let Some(v) = tree.value_attr(name_node)
        && !v.is_empty()
    {
        return Some(v.to_owned());
    }
    None
}

fn register_target_id(
    tree: &XmlTree,
    param_node: XmlNodeId,
    target: PointeeTarget,
    entities: &mut EntityMap,
) {
    if let Some(at) = tree.child(param_node, "AutomationTarget")
        && let Some(id_str) = tree.attr(at, "Id")
        && let Ok(pid) = id_str.parse::<u64>()
    {
        entities.pointee_targets.insert(pid, target);
    }
}

fn read_mixer(
    tree: &XmlTree,
    mixer: XmlNodeId,
    track_id: TrackId,
    entities: &mut EntityMap,
) -> (f32, f32, bool, bool, Vec<f32>) {
    let volume_db = if let Some(vol) = tree.child(mixer, "Volume") {
        register_target_id(tree, vol, PointeeTarget::TrackVolume(track_id), entities);
        manual_value_str(tree, vol)
            .and_then(|s| s.parse::<f32>().ok())
            .map(linear_to_db)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let pan = if let Some(pan_node) = tree.child(mixer, "Pan") {
        register_target_id(tree, pan_node, PointeeTarget::TrackPan(track_id), entities);
        manual_value_str(tree, pan_node)
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(-1.0, 1.0))
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let is_muted = tree
        .child(mixer, "Speaker")
        .and_then(|sp| manual_value_str(tree, sp))
        .and_then(|s| match s {
            "true" | "1" => Some(false),
            "false" | "0" => Some(true),
            _ => None,
        })
        .unwrap_or(false);

    let is_soloed = parse_child_bool(tree, mixer, "SoloSink").unwrap_or(false);

    let mut sends = Vec::new();
    if let Some(sends_node) = tree.child(mixer, "Sends") {
        for (pos, holder) in tree
            .children_named(sends_node, "TrackSendHolder")
            .iter()
            .copied()
            .enumerate()
        {
            let idx = tree
                .attr(holder, "Id")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(pos);
            if sends.len() <= idx {
                sends.resize(idx + 1, 0.0);
            }
            if let Some(send) = tree.child(holder, "Send") {
                register_target_id(
                    tree,
                    send,
                    PointeeTarget::SendLevel(track_id, idx),
                    entities,
                );
                if let Some(raw) = manual_value_str(tree, send).and_then(|s| s.parse::<f32>().ok())
                    && raw.is_finite()
                {
                    sends[idx] = if raw <= 0.000_317 {
                        0.0
                    } else {
                        raw.clamp(0.0, 1.0)
                    };
                }
            }
        }
    }

    (volume_db, pan, is_muted, is_soloed, sends)
}

pub(crate) fn patch_all_tracks(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    baseline: &Project,
    project: &Project,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let tracks_node = tree
        .ensure_child(live_set, "Tracks")
        .ok_or_else(|| AlsError::InvalidFormat("missing `<Tracks>` container".to_owned()))?;

    let mut base_by_id: HashMap<TrackId, &Track> =
        baseline.tracks.iter().map(|t| (t.id, t)).collect();
    for rt in &baseline.return_tracks {
        base_by_id.insert(rt.id, rt);
    }

    let mut new_track_map = HashMap::new();
    let mut desired_children =
        Vec::with_capacity(project.tracks.len() + project.return_tracks.len());

    for track in project.tracks.iter().chain(&project.return_tracks) {
        let expected_tag = track_tag_name(track.track_type);
        let existing_node =
            entities.tracks.get(&track.id).copied().filter(|&nid| {
                tree.node(nid).and_then(XmlNode::element_name) == Some(expected_tag)
            });

        let node_id = if let Some(nid) = existing_node {
            if let Some(prev) = base_by_id.get(&track.id) {
                if !tracks_equal(prev, track) {
                    patch_track_node(tree, nid, Some(prev), track, entities, ids)?;
                }
            } else {
                patch_track_node(tree, nid, None, track, entities, ids)?;
            }
            nid
        } else {
            let fresh = create_track_from_template(tree, track.track_type)?;
            tree.set_attr(fresh, "Id", track.id.0.to_string());
            patch_track_node(tree, fresh, None, track, entities, ids)?;
            fresh
        };
        new_track_map.insert(track.id, node_id);
        desired_children.push(node_id);
    }

    // Only reorder `<Tracks>` children if track membership/order actually changed from baseline.
    let same_regular = baseline.tracks.len() == project.tracks.len()
        && baseline
            .tracks
            .iter()
            .zip(&project.tracks)
            .all(|(a, b)| a.id == b.id && a.track_type == b.track_type);
    let same_returns = baseline.return_tracks.len() == project.return_tracks.len()
        && baseline
            .return_tracks
            .iter()
            .zip(&project.return_tracks)
            .all(|(a, b)| a.id == b.id && a.track_type == b.track_type);
    if !(same_regular && same_returns) {
        sync_ordered_children(
            tree,
            tracks_node,
            |n| element_has_name(n, &["MidiTrack", "AudioTrack", "GroupTrack", "ReturnTrack"]),
            desired_children,
        );
    }
    entities.tracks = new_track_map;

    if !tracks_equal(&baseline.main_track, &project.main_track) || entities.main_track.is_none() {
        let mt_node = if let Some(existing) = entities.main_track {
            if project.main_track.id != baseline.main_track.id {
                tree.set_attr(existing, "Id", project.main_track.id.0.to_string());
                entities.main_track_had_id_attr = true;
            }
            patch_track_node(
                tree,
                existing,
                Some(&baseline.main_track),
                &project.main_track,
                entities,
                ids,
            )?;
            existing
        } else {
            let fresh = tree.alloc_element_with(
                "MainTrack",
                vec![XmlAttr::new("Id", project.main_track.id.0.to_string())],
                Some(false),
            );
            tree.append_child(live_set, fresh);
            entities.main_track = Some(fresh);
            entities.main_track_had_id_attr = true;
            patch_track_node(tree, fresh, None, &project.main_track, entities, ids)?;
            fresh
        };
        entities.main_track = Some(mt_node);
    }

    Ok(())
}

fn track_tag_name(tt: TrackType) -> &'static str {
    match tt {
        TrackType::Audio => "AudioTrack",
        TrackType::Midi => "MidiTrack",
        TrackType::Group => "GroupTrack",
        TrackType::Return => "ReturnTrack",
        TrackType::Main => "MainTrack",
    }
}

fn create_track_from_template(tree: &mut XmlTree, tt: TrackType) -> Result<XmlNodeId, AlsError> {
    match tt {
        TrackType::Midi => {
            instantiate_template_cached(tree, &MIDI_TRACK_CACHE, MIDI_TRACK_TEMPLATE_XML)
        }
        TrackType::Audio => {
            instantiate_template_cached(tree, &AUDIO_TRACK_CACHE, AUDIO_TRACK_TEMPLATE_XML)
        }
        TrackType::Group => {
            instantiate_template_cached(tree, &GROUP_TRACK_CACHE, GROUP_TRACK_TEMPLATE_XML)
        }
        TrackType::Return => {
            instantiate_template_cached(tree, &RETURN_TRACK_CACHE, RETURN_TRACK_TEMPLATE_XML)
        }
        TrackType::Main => Err(AlsError::InvalidFormat(
            "unexpected Main track in Tracks".to_owned(),
        )),
    }
}

fn patch_track_node(
    tree: &mut XmlTree,
    node: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    if prev.is_none_or(|p| p.name != cur.name)
        && let Some(name_node) = tree.ensure_child(node, "Name")
    {
        if tree.child(name_node, "EffectiveName").is_some()
            || tree.child(name_node, "UserName").is_some()
        {
            set_child_value(tree, name_node, "EffectiveName", &cur.name);
            set_child_value(tree, name_node, "UserName", &cur.name);
        } else {
            set_child_value(tree, name_node, "UserName", &cur.name);
        }
    }

    let def_color = default_track_color(cur.track_type);
    if prev.is_none_or(|p| p.color != cur.color) {
        set_child_value(tree, node, "Color", color_to_xml(&cur.color, def_color));
    }

    if prev.is_none_or(|p| p.parent != cur.parent) {
        let group_str = match cur.parent {
            Some(gid) => gid.0.to_string(),
            None => "-1".to_owned(),
        };
        set_child_value(tree, node, "TrackGroupId", group_str);
    }

    let dc = tree
        .ensure_child(node, "DeviceChain")
        .ok_or_else(|| AlsError::InvalidFormat("missing Track DeviceChain".to_owned()))?;

    if prev.is_none_or(|p| p.routing != cur.routing) {
        routing::patch_routing(tree, dc, prev.map(|p| &p.routing), &cur.routing);
    }

    let mx = tree
        .ensure_child(dc, "Mixer")
        .ok_or_else(|| AlsError::InvalidFormat("missing Track Mixer".to_owned()))?;
    patch_mixer(tree, mx, prev, cur);

    if !matches!(cur.track_type, TrackType::Return | TrackType::Main)
        && (prev.is_none_or(|p| {
            p.monitoring != cur.monitoring
                || p.is_armed != cur.is_armed
                || p.clip_slots.len() != cur.clip_slots.len()
                || !p.clip_slots.is_empty()
                || !cur.clip_slots.is_empty()
                || !p.arrangement_clips.is_empty()
                || !cur.arrangement_clips.is_empty()
        }))
        && let Some(ms) = tree.ensure_child(dc, "MainSequencer")
    {
        if prev.is_none_or(|p| p.monitoring != cur.monitoring) {
            let mon_val = match cur.monitoring {
                MonitoringMode::In => "0",
                MonitoringMode::Auto => "1",
                MonitoringMode::Off => "2",
            };
            set_child_value(tree, ms, "MonitoringEnum", mon_val);
        }
        if prev.is_none_or(|p| p.is_armed != cur.is_armed)
            && (cur.is_armed || tree.child(ms, "Recorder").is_some())
            && let Some(rec) = tree.ensure_child(ms, "Recorder")
        {
            set_child_value(tree, rec, "IsArmed", bool_str(cur.is_armed));
        }
        clips::patch_track_clips(tree, ms, prev, cur, entities, ids)?;
    }

    devices::patch_track_devices(tree, dc, prev, cur, entities, ids)?;
    automation::patch_track_automation(tree, node, dc, prev, cur, entities, ids)?;

    Ok(())
}

fn patch_mixer(tree: &mut XmlTree, mx: XmlNodeId, prev: Option<&Track>, cur: &Track) {
    if prev.is_none_or(|p| !f32_eq(p.volume_db, cur.volume_db))
        && let Some(vol) = tree.ensure_child(mx, "Volume")
    {
        set_manual_value(tree, vol, format_f32(db_to_linear(cur.volume_db)));
    }
    if prev.is_none_or(|p| !f32_eq(p.pan, cur.pan))
        && let Some(pan) = tree.ensure_child(mx, "Pan")
    {
        set_manual_value(tree, pan, format_f32(cur.pan));
    }
    if prev.is_none_or(|p| p.is_muted != cur.is_muted)
        && let Some(sp) = tree.ensure_child(mx, "Speaker")
    {
        set_manual_value(tree, sp, bool_str(!cur.is_muted));
    }
    if prev.is_none_or(|p| p.is_soloed != cur.is_soloed) {
        set_child_value(tree, mx, "SoloSink", bool_str(cur.is_soloed));
    }

    let sends_changed = prev.is_none_or(|p| {
        p.sends.len() != cur.sends.len()
            || p.sends.iter().zip(&cur.sends).any(|(&a, &b)| !f32_eq(a, b))
    });
    if sends_changed {
        let should_write_main = cur.track_type != TrackType::Main
            || tree
                .child(mx, "Sends")
                .is_some_and(|s| !tree.children_named(s, "TrackSendHolder").is_empty())
            || cur.sends.iter().any(|&v| v != 0.0);
        if should_write_main && let Some(sends_node) = tree.ensure_child(mx, "Sends") {
            let existing_holders = tree.children_named(sends_node, "TrackSendHolder");
            let mut by_idx: HashMap<usize, XmlNodeId> = HashMap::new();
            for (pos, h) in existing_holders.into_iter().enumerate() {
                let idx = tree
                    .attr(h, "Id")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(pos);
                by_idx.insert(idx, h);
            }
            let mut desired = Vec::with_capacity(cur.sends.len());
            for (idx, &level) in cur.sends.iter().enumerate() {
                let prev_level = prev.and_then(|p| p.sends.get(idx).copied());
                let holder = if let Some(&h) = by_idx.get(&idx) {
                    if prev_level.is_none_or(|pl| !f32_eq(pl, level))
                        && let Some(send) = tree.ensure_child(h, "Send")
                    {
                        set_manual_value(tree, send, format_f32(level));
                    }
                    h
                } else {
                    let h = tree.alloc_element_with(
                        "TrackSendHolder",
                        vec![XmlAttr::new("Id", idx.to_string())],
                        Some(false),
                    );
                    let send = tree.alloc_element("Send");
                    tree.append_child(h, send);
                    append_value_child(tree, send, "Manual", format_f32(level));
                    h
                };
                desired.push(holder);
            }
            sync_ordered_children(
                tree,
                sends_node,
                |n| n.element_name() == Some("TrackSendHolder"),
                desired,
            );
        }
    }
}
