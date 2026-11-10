//! Mappers for Ableton `<AutomationEnvelopes>` (track and clip automation lanes)
//! and `EnvelopeTarget/PointeeId` <-> `AutomationTarget/@Id` resolution.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use livecraft_model::ids::{LaneId, TrackId};
use livecraft_model::model::{AutomationLane, AutomationPoint, AutomationTarget, Clip, Track};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNodeId, XmlTree};
use super::devices;
use super::{
    AlsIdAllocator, EntityMap, PointeeTarget, bool_str, child_value_str, format_f32, format_f64,
    instantiate_template_cached, parse_child_bool, set_child_value, sync_ordered_children,
};

const ENVELOPE_TEMPLATE_XML: &str = include_str!("../../../templates/automation_envelope.xml");
static ENVELOPE_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
const SENTINEL_THRESHOLD: f64 = -1_000_000.0;

pub(crate) fn read_track_automation(
    tree: &XmlTree,
    track_node: XmlNodeId,
    track_id: TrackId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Vec<AutomationLane> {
    let Some(envelopes) = tree
        .child(track_node, "AutomationEnvelopes")
        .and_then(|ae| tree.child(ae, "Envelopes"))
    else {
        return Vec::new();
    };
    read_envelopes_list(tree, envelopes, track_id, entities, ids)
}

pub(crate) fn read_clip_automation(
    tree: &XmlTree,
    clip_node: XmlNodeId,
    track_id: TrackId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Vec<AutomationLane> {
    let Some(envelopes) = tree
        .child(clip_node, "Envelopes")
        .and_then(|e| tree.child(e, "Envelopes"))
    else {
        return Vec::new();
    };
    read_envelopes_list(tree, envelopes, track_id, entities, ids)
}

fn read_envelopes_list(
    tree: &XmlTree,
    envelopes_node: XmlNodeId,
    track_id: TrackId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Vec<AutomationLane> {
    let mut out = Vec::new();
    let mut used = HashSet::new();

    for env in tree.children_named(envelopes_node, "AutomationEnvelope") {
        let Some(pid) = tree
            .child(env, "EnvelopeTarget")
            .and_then(|et| child_value_str(tree, et, "PointeeId"))
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        let Some(target) =
            pointee_to_automation_target(entities.pointee_targets.get(&pid), track_id)
        else {
            continue;
        };

        let parsed_id = tree.attr(env, "Id").and_then(|s| s.parse::<u64>().ok());
        let id = match parsed_id {
            Some(v) if !used.contains(&v) => {
                used.insert(v);
                v
            }
            _ => {
                let fresh = ids.alloc();
                used.insert(fresh);
                fresh
            }
        };
        let lane_id = LaneId(id);
        entities.automation_lanes.insert(lane_id, env);

        let points = read_float_events(tree, env);
        let is_enabled = parse_child_bool(tree, env, "LiveCraftEnabled").unwrap_or(true);
        out.push(AutomationLane {
            id: lane_id,
            target,
            points,
            is_enabled,
        });
    }

    out
}

fn pointee_to_automation_target(
    pt: Option<&PointeeTarget>,
    track_id: TrackId,
) -> Option<AutomationTarget> {
    match pt? {
        PointeeTarget::Tempo | PointeeTarget::TimeSignature => None,
        PointeeTarget::TrackVolume(tid) if *tid == track_id => Some(AutomationTarget::TrackVolume),
        PointeeTarget::TrackPan(tid) if *tid == track_id => Some(AutomationTarget::TrackPan),
        PointeeTarget::SendLevel(tid, idx) if *tid == track_id => {
            Some(AutomationTarget::SendLevel(*idx))
        }
        PointeeTarget::DeviceParam {
            track_id: tid,
            device_id,
            param_key,
        } if *tid == track_id => Some(AutomationTarget::DeviceParam {
            device_id: *device_id,
            param_key: param_key.clone(),
        }),
        _ => None,
    }
}

fn read_float_events(tree: &XmlTree, env_node: XmlNodeId) -> Vec<AutomationPoint> {
    let Some(events_node) = tree
        .child(env_node, "Automation")
        .and_then(|a| tree.child(a, "Events"))
    else {
        return Vec::new();
    };
    let mut points = Vec::new();
    let events = tree.children_named(events_node, "FloatEvent");
    let has_non_sentinel = events.iter().any(|&ev| {
        tree.attr(ev, "Time")
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(|t| t > SENTINEL_THRESHOLD)
    });

    for ev in events {
        let Some(raw_time) = tree.attr(ev, "Time").and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let Some(value) = tree.attr(ev, "Value").and_then(|s| s.parse::<f32>().ok()) else {
            continue;
        };
        if !raw_time.is_finite() || !value.is_finite() {
            continue;
        }
        if raw_time <= SENTINEL_THRESHOLD && has_non_sentinel {
            continue;
        }
        let beat = if raw_time <= SENTINEL_THRESHOLD {
            0.0
        } else {
            raw_time
        };
        let curve = tree
            .attr(ev, "CurveControl1Y")
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|&y| y > 0.0 && y < 1.0 && (y - 0.5).abs() > 1e-6)
            .map(|y| {
                let exp = -y.ln() / std::f64::consts::LN_2;
                if exp > 0.0 {
                    (-(exp.ln()) / 2.0) as f32
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        points.push(AutomationPoint { beat, value, curve });
    }
    points
}

pub(crate) fn patch_track_automation(
    tree: &mut XmlTree,
    track_node: XmlNodeId,
    dc: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let unchanged = prev.is_some_and(|p| p.automation_lanes == cur.automation_lanes);
    if unchanged {
        return Ok(());
    }
    if cur.automation_lanes.is_empty() && tree.child(track_node, "AutomationEnvelopes").is_none() {
        return Ok(());
    }
    let Some(envelopes) = tree
        .ensure_child(track_node, "AutomationEnvelopes")
        .and_then(|ae| tree.ensure_child(ae, "Envelopes"))
    else {
        return Ok(());
    };
    let prev_lanes = prev.map(|p| p.automation_lanes.as_slice()).unwrap_or(&[]);
    patch_envelopes_list(
        tree,
        envelopes,
        Some(dc),
        cur,
        prev_lanes,
        &cur.automation_lanes,
        entities,
        ids,
    )
}

pub(crate) fn patch_clip_automation(
    tree: &mut XmlTree,
    clip_node: XmlNodeId,
    _track_id: TrackId,
    prev: Option<&Clip>,
    cur: &Clip,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let unchanged = prev.is_some_and(|p| p.automation_lanes == cur.automation_lanes);
    if unchanged {
        return Ok(());
    }
    if cur.automation_lanes.is_empty() && tree.child(clip_node, "Envelopes").is_none() {
        return Ok(());
    }
    let Some(envelopes) = tree
        .ensure_child(clip_node, "Envelopes")
        .and_then(|e| tree.ensure_child(e, "Envelopes"))
    else {
        return Ok(());
    };
    let prev_lanes = prev.map(|p| p.automation_lanes.as_slice()).unwrap_or(&[]);

    // Reuse track/device targets already registered in `entities` for clip automation.
    let dummy_track = Track::new(
        TrackId(0),
        "",
        cur.automation_lanes
            .first()
            .map_or(livecraft_model::model::TrackType::Midi, |_| {
                livecraft_model::model::TrackType::Midi
            }),
        0,
    );
    let _ = dummy_track;
    patch_envelopes_list_clip(
        tree,
        envelopes,
        prev_lanes,
        &cur.automation_lanes,
        entities,
        ids,
    )
}

#[allow(clippy::too_many_arguments)]
fn patch_envelopes_list(
    tree: &mut XmlTree,
    envelopes_node: XmlNodeId,
    dc: Option<XmlNodeId>,
    track: &Track,
    prev_lanes: &[AutomationLane],
    cur_lanes: &[AutomationLane],
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let prev_by_id: HashMap<LaneId, &AutomationLane> =
        prev_lanes.iter().map(|l| (l.id, l)).collect();

    // Preserve unmapped Live envelopes (such as MainTrack Tempo and TimeSignature).
    let mut preserved_unmapped = Vec::new();
    for env in tree.children_named(envelopes_node, "AutomationEnvelope") {
        let pid = tree
            .child(env, "EnvelopeTarget")
            .and_then(|et| child_value_str(tree, et, "PointeeId"))
            .and_then(|s| s.parse::<u64>().ok());
        let is_unmapped = pid.is_none_or(|p| {
            pointee_to_automation_target(entities.pointee_targets.get(&p), track.id).is_none()
        });
        if is_unmapped {
            preserved_unmapped.push(env);
        }
    }

    let mut desired = preserved_unmapped;
    for lane in cur_lanes {
        let pointee_id = resolve_or_create_pointee_id(tree, dc, track, &lane.target, entities, ids);
        let existing = entities
            .automation_lanes
            .get(&lane.id)
            .copied()
            .filter(|&nid| {
                tree.node(nid)
                    .is_some_and(|n| n.parent == Some(envelopes_node))
            });

        let env_node = if let Some(nid) = existing {
            if prev_by_id.get(&lane.id).is_none_or(|pl| *pl != lane) {
                patch_envelope_node(tree, nid, pointee_id, lane, ids);
            }
            nid
        } else {
            let fresh = instantiate_template_cached(tree, &ENVELOPE_CACHE, ENVELOPE_TEMPLATE_XML)?;
            tree.set_attr(fresh, "Id", lane.id.0.to_string());
            patch_envelope_node(tree, fresh, pointee_id, lane, ids);
            fresh
        };
        entities.automation_lanes.insert(lane.id, env_node);
        desired.push(env_node);
    }

    sync_ordered_children(
        tree,
        envelopes_node,
        |n| n.element_name() == Some("AutomationEnvelope"),
        desired,
    );
    Ok(())
}

fn patch_envelopes_list_clip(
    tree: &mut XmlTree,
    envelopes_node: XmlNodeId,
    prev_lanes: &[AutomationLane],
    cur_lanes: &[AutomationLane],
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let prev_by_id: HashMap<LaneId, &AutomationLane> =
        prev_lanes.iter().map(|l| (l.id, l)).collect();
    let mut desired = Vec::new();
    for lane in cur_lanes {
        let pointee_id =
            find_existing_pointee(&lane.target, entities).unwrap_or_else(|| ids.alloc());
        let existing = entities
            .automation_lanes
            .get(&lane.id)
            .copied()
            .filter(|&nid| {
                tree.node(nid)
                    .is_some_and(|n| n.parent == Some(envelopes_node))
            });
        let env_node = if let Some(nid) = existing {
            if prev_by_id.get(&lane.id).is_none_or(|pl| *pl != lane) {
                patch_envelope_node(tree, nid, pointee_id, lane, ids);
            }
            nid
        } else {
            let fresh = instantiate_template_cached(tree, &ENVELOPE_CACHE, ENVELOPE_TEMPLATE_XML)?;
            tree.set_attr(fresh, "Id", lane.id.0.to_string());
            patch_envelope_node(tree, fresh, pointee_id, lane, ids);
            fresh
        };
        entities.automation_lanes.insert(lane.id, env_node);
        desired.push(env_node);
    }
    sync_ordered_children(
        tree,
        envelopes_node,
        |n| n.element_name() == Some("AutomationEnvelope"),
        desired,
    );
    Ok(())
}

fn find_existing_pointee(target: &AutomationTarget, entities: &EntityMap) -> Option<u64> {
    entities.pointee_targets.iter().find_map(|(&pid, pt)| {
        let matches = match (target, pt) {
            (AutomationTarget::TrackVolume, PointeeTarget::TrackVolume(_)) => true,
            (AutomationTarget::TrackPan, PointeeTarget::TrackPan(_)) => true,
            (AutomationTarget::SendLevel(a), PointeeTarget::SendLevel(_, b)) => a == b,
            (
                AutomationTarget::DeviceParam {
                    device_id: da,
                    param_key: ka,
                },
                PointeeTarget::DeviceParam {
                    device_id: db,
                    param_key: kb,
                    ..
                },
            ) => da == db && ka == kb,
            _ => false,
        };
        matches.then_some(pid)
    })
}

fn resolve_or_create_pointee_id(
    tree: &mut XmlTree,
    dc: Option<XmlNodeId>,
    track: &Track,
    target: &AutomationTarget,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> u64 {
    if let Some((&pid, _)) = entities
        .pointee_targets
        .iter()
        .find(|(_, pt)| match (target, *pt) {
            (AutomationTarget::TrackVolume, PointeeTarget::TrackVolume(tid)) => *tid == track.id,
            (AutomationTarget::TrackPan, PointeeTarget::TrackPan(tid)) => *tid == track.id,
            (AutomationTarget::SendLevel(a), PointeeTarget::SendLevel(tid, b)) => {
                *tid == track.id && a == b
            }
            (
                AutomationTarget::DeviceParam {
                    device_id: da,
                    param_key: ka,
                },
                PointeeTarget::DeviceParam {
                    track_id: tid,
                    device_id: db,
                    param_key: kb,
                },
            ) => *tid == track.id && da == db && ka == kb,
            _ => false,
        })
    {
        return pid;
    }

    let mut target_param_node: Option<XmlNodeId> = None;
    let mut pointee_kind: Option<PointeeTarget> = None;
    if let Some(dc_node) = dc {
        match target {
            AutomationTarget::TrackVolume => {
                target_param_node = tree
                    .child(dc_node, "Mixer")
                    .and_then(|mx| tree.ensure_child(mx, "Volume"));
                pointee_kind = Some(PointeeTarget::TrackVolume(track.id));
            }
            AutomationTarget::TrackPan => {
                target_param_node = tree
                    .child(dc_node, "Mixer")
                    .and_then(|mx| tree.ensure_child(mx, "Pan"));
                pointee_kind = Some(PointeeTarget::TrackPan(track.id));
            }
            AutomationTarget::SendLevel(idx) => {
                if let Some(sends) = tree
                    .child(dc_node, "Mixer")
                    .and_then(|mx| tree.child(mx, "Sends"))
                {
                    let idx_s = idx.to_string();
                    target_param_node = tree
                        .children_named(sends, "TrackSendHolder")
                        .into_iter()
                        .find(|&h| tree.attr(h, "Id") == Some(idx_s.as_str()))
                        .and_then(|h| tree.ensure_child(h, "Send"));
                }
                pointee_kind = Some(PointeeTarget::SendLevel(track.id, *idx));
            }
            AutomationTarget::DeviceParam {
                device_id,
                param_key,
            } => {
                if let (Some(&dev_node), Some(dev)) = (
                    entities.devices.get(device_id),
                    track.device_chain.iter().find(|d| d.id == *device_id),
                ) {
                    target_param_node = devices::ensure_device_param_node(
                        tree,
                        dev_node,
                        &dev.device_type,
                        param_key,
                    );
                }
                pointee_kind = Some(PointeeTarget::DeviceParam {
                    track_id: track.id,
                    device_id: *device_id,
                    param_key: param_key.clone(),
                });
            }
        }
    }

    if let Some(pnode) = target_param_node {
        if let Some(at) = tree.child(pnode, "AutomationTarget")
            && let Some(pid) = tree.attr(at, "Id").and_then(|s| s.parse::<u64>().ok())
        {
            if let Some(pt) = pointee_kind {
                entities.pointee_targets.insert(pid, pt);
            }
            return pid;
        }
        let pid = ids.alloc();
        if let Some(at) = tree.ensure_child(pnode, "AutomationTarget") {
            tree.set_attr(at, "Id", pid.to_string());
            set_child_value(tree, at, "LockEnvelope", "0");
        }
        if let Some(pt) = pointee_kind {
            entities.pointee_targets.insert(pid, pt);
        }
        return pid;
    }

    let pid = ids.alloc();
    if let Some(pt) = pointee_kind {
        entities.pointee_targets.insert(pid, pt);
    }
    pid
}

fn patch_envelope_node(
    tree: &mut XmlTree,
    env_node: XmlNodeId,
    pointee_id: u64,
    lane: &AutomationLane,
    ids: &mut AlsIdAllocator,
) {
    if let Some(et) = tree.ensure_child(env_node, "EnvelopeTarget") {
        set_child_value(tree, et, "PointeeId", pointee_id.to_string());
    }

    if lane.is_enabled {
        if let Some(old) = tree.child(env_node, "LiveCraftEnabled") {
            tree.remove_child(env_node, old);
        }
    } else {
        set_child_value(tree, env_node, "LiveCraftEnabled", bool_str(false));
    }

    if let Some(auto) = tree.ensure_child(env_node, "Automation")
        && let Some(events_node) = tree.ensure_child(auto, "Events")
    {
        tree.clear_children(events_node);
        for pt in &lane.points {
            let mut attrs = vec![
                XmlAttr::new("Id", ids.alloc().to_string()),
                XmlAttr::new("Time", format_f64(pt.beat)),
                XmlAttr::new("Value", format_f32(pt.value)),
            ];
            if pt.curve.abs() > 1e-4 {
                let y = 0.5_f64.powf((-f64::from(pt.curve) * 2.0).exp());
                let y_str = format_f64(y);
                attrs.push(XmlAttr::new("CurveControl1X", "0.5"));
                attrs.push(XmlAttr::new("CurveControl1Y", y_str.clone()));
                attrs.push(XmlAttr::new("CurveControl2X", "0.5"));
                attrs.push(XmlAttr::new("CurveControl2Y", y_str));
            }
            let ev = tree.alloc_element_with("FloatEvent", attrs, Some(true));
            tree.append_child(events_node, ev);
        }
    }
}
