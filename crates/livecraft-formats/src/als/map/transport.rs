//! Mappers for Ableton `<Ableton>`, `<Transport>`, Tempo/TimeSignature automation,
//! and `<GlobalQuantisation>`.

use std::sync::OnceLock;

use livecraft_model::model::{Project, ScaleConfig, Scene, Transport, TuningSystem};
use livecraft_time::{Beats, MIN_BPM, TempoMap, TimeSignatureMap};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNodeId, XmlTree};
use super::PointeeTarget;
use super::scenes::{
    global_index_to_quantization, patch_livecraft_transport, patch_scale_config, patch_scenes,
    patch_tuning, quantization_to_global_index, read_livecraft_transport, read_scale_config,
    read_scenes, read_tuning,
};
use super::{
    AlsIdAllocator, EntityMap, append_value_child, bool_str, child_value_str, format_f64,
    instantiate_template_cached, manual_value_str, parse_child_bool, parse_child_f64,
    parse_child_u64, set_child_value, set_manual_value,
};

const ENVELOPE_TEMPLATE_XML: &str = include_str!("../../../templates/automation_envelope.xml");
static ENVELOPE_TEMPLATE_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
const SENTINEL_EVENT_TIME: f64 = -63_072_000.0;

#[derive(Debug, Clone)]
pub(crate) struct ParsedGlobals {
    pub schema_version: String,
    pub creator: String,
    pub transport: Transport,
    pub scale: ScaleConfig,
    pub tuning: Option<TuningSystem>,
    pub scenes: Vec<Scene>,
}

/// Live's `EncodedTimeSignature` = `(numerator - 1) + (log2(denominator) * 99)`.
#[must_use]
pub fn encode_time_signature(numerator: u8, denominator: u8) -> i64 {
    let num = i64::from(numerator.clamp(1, 99) - 1);
    let log2_den = match denominator {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        _ => 2,
    };
    num + log2_den * 99
}

/// Decodes Live's `EncodedTimeSignature` back into `(numerator, denominator)`.
#[must_use]
pub fn decode_time_signature(encoded: i64) -> Option<(u8, u8)> {
    if encoded < 0 {
        return None;
    }
    let log2_den = encoded / 99;
    let num = (encoded % 99) + 1;
    if log2_den <= 4 && (1..=99).contains(&num) {
        Some((num as u8, 1u8 << log2_den))
    } else {
        None
    }
}

pub(crate) fn read_globals(
    tree: &XmlTree,
    root: XmlNodeId,
    live_set: XmlNodeId,
    main_track_node: Option<XmlNodeId>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<ParsedGlobals, AlsError> {
    let creator = tree
        .attr(root, "Creator")
        .filter(|s| !s.is_empty())
        .unwrap_or("Ableton Live")
        .to_owned();
    let schema_version = tree
        .attr(root, "MinorVersion")
        .filter(|s| !s.is_empty())
        .map(|v| format!("Live {v}"))
        .unwrap_or_else(|| "Unknown".to_owned());

    let mixer_node = main_track_node
        .and_then(|mt| tree.child(mt, "DeviceChain"))
        .and_then(|dc| tree.child(dc, "Mixer"));

    let tempo = read_tempo_map(tree, live_set, main_track_node, mixer_node, entities);
    let time_signature = read_time_signature_map(tree, main_track_node, mixer_node, entities);

    let mut transport = Transport {
        tempo,
        time_signature,
        ..Transport::default()
    };

    if let Some(tr_node) = tree.child(live_set, "Transport") {
        if let Some(v) = parse_child_bool(tree, tr_node, "LoopOn") {
            transport.loop_enabled = v;
        }
        if let Some(v) = parse_child_f64(tree, tr_node, "LoopStart") {
            transport.loop_start_beats = v;
        }
        if let Some(v) = parse_child_f64(tree, tr_node, "LoopLength") {
            transport.loop_length_beats = v;
        }
        if let Some(v) = parse_child_f64(tree, tr_node, "CurrentTime") {
            transport.playhead_beats = v;
        }
        if let Some(v) = parse_child_bool(tree, tr_node, "PunchIn") {
            transport.punch_in = v;
        }
        if let Some(v) = parse_child_bool(tree, tr_node, "PunchOut") {
            transport.punch_out = v;
        }
    }

    if let Some(gq) = parse_child_u64(tree, live_set, "GlobalQuantisation")
        && let Some(q) = global_index_to_quantization(gq as usize)
    {
        transport.global_quantization = q;
    }

    read_livecraft_transport(tree, live_set, &mut transport);

    Ok(ParsedGlobals {
        schema_version,
        creator,
        transport,
        scale: read_scale_config(tree, live_set),
        tuning: read_tuning(tree, live_set),
        scenes: read_scenes(tree, live_set, entities, ids),
    })
}

fn find_main_envelope(
    tree: &XmlTree,
    main_track_node: XmlNodeId,
    pointee_id: u64,
) -> Option<XmlNodeId> {
    let envelopes = tree
        .child(main_track_node, "AutomationEnvelopes")
        .and_then(|ae| tree.child(ae, "Envelopes"))?;
    let target_str = pointee_id.to_string();
    for env in tree.children_named(envelopes, "AutomationEnvelope") {
        let pid = tree
            .child(env, "EnvelopeTarget")
            .and_then(|et| child_value_str(tree, et, "PointeeId"));
        if pid == Some(target_str.as_str()) {
            return Some(env);
        }
    }
    None
}

fn read_tempo_map(
    tree: &XmlTree,
    live_set: XmlNodeId,
    main_track_node: Option<XmlNodeId>,
    mixer_node: Option<XmlNodeId>,
    entities: &mut EntityMap,
) -> TempoMap {
    let tempo_node = mixer_node
        .and_then(|mx| tree.child(mx, "Tempo"))
        .or_else(|| {
            tree.child(live_set, "Transport")
                .and_then(|tr| tree.child(tr, "Tempo"))
        });

    let manual_bpm = tempo_node
        .and_then(|tn| manual_value_str(tree, tn))
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(120.0);

    if let (Some(mt), Some(tn)) = (main_track_node, tempo_node)
        && let Some(pid) = tree
            .child(tn, "AutomationTarget")
            .and_then(|at| tree.attr(at, "Id"))
            .and_then(|s| s.parse::<u64>().ok())
    {
        entities.pointee_targets.insert(pid, PointeeTarget::Tempo);
        if let Some(env_node) = find_main_envelope(tree, mt, pid) {
            entities.tempo_envelope_node = Some(env_node);
            if let Some(map) = parse_tempo_envelope(tree, env_node, manual_bpm) {
                return map;
            }
        }
    }

    TempoMap::constant(manual_bpm)
}

fn parse_tempo_envelope(tree: &XmlTree, env_node: XmlNodeId, manual_bpm: f64) -> Option<TempoMap> {
    let events_node = tree
        .child(env_node, "Automation")
        .and_then(|a| tree.child(a, "Events"))?;
    let mut raw: Vec<(f64, f64)> = Vec::new();
    for ev in tree.children_named(events_node, "FloatEvent") {
        let Some(t) = tree.attr(ev, "Time").and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let Some(v) = tree.attr(ev, "Value").and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        if t.is_finite() && v.is_finite() {
            raw.push((if t <= -1_000_000.0 { 0.0 } else { t }, v));
        }
    }
    if raw.is_empty() {
        return None;
    }
    raw.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut groups: Vec<(f64, Vec<f64>)> = Vec::new();
    for (t, v) in raw {
        if let Some(last) = groups.last_mut()
            && (last.0 - t).abs() < 1e-9
        {
            last.1.push(v);
        } else {
            groups.push((t, vec![v]));
        }
    }

    if groups.len() == 1 && groups[0].0 <= 0.0 {
        let val = groups[0].1.last().copied().unwrap_or(manual_bpm);
        let bpm = if (val - manual_bpm).abs() > 1e-6 && groups[0].1.len() == 1 {
            manual_bpm
        } else {
            val
        };
        return Some(TempoMap::constant(bpm));
    }

    let first_bpm = groups[0].1.last().copied().unwrap_or(manual_bpm);
    let mut map = TempoMap::constant(first_bpm);
    let mut points: Vec<(f64, f64, bool)> = vec![(groups[0].0.max(0.0), first_bpm, false)];
    for grp in groups.iter().skip(1) {
        let is_step = grp.1.len() >= 2;
        if let Some(prev) = points.last_mut() {
            prev.2 = !is_step;
        }
        let next_bpm = grp.1.last().copied().unwrap_or(first_bpm);
        points.push((grp.0, next_bpm, false));
    }
    for (at, bpm, ramp) in points {
        map.insert(Beats(at), bpm, ramp);
    }
    Some(map)
}

fn read_time_signature_map(
    tree: &XmlTree,
    main_track_node: Option<XmlNodeId>,
    mixer_node: Option<XmlNodeId>,
    entities: &mut EntityMap,
) -> TimeSignatureMap {
    let Some(ts_node) = mixer_node.and_then(|mx| tree.child(mx, "TimeSignature")) else {
        return TimeSignatureMap::default();
    };
    let manual_sig = manual_value_str(tree, ts_node)
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(decode_time_signature);

    if let Some(mt) = main_track_node
        && let Some(pid) = tree
            .child(ts_node, "AutomationTarget")
            .and_then(|at| tree.attr(at, "Id"))
            .and_then(|s| s.parse::<u64>().ok())
    {
        entities
            .pointee_targets
            .insert(pid, PointeeTarget::TimeSignature);
        if let Some(env_node) = find_main_envelope(tree, mt, pid) {
            entities.time_signature_envelope_node = Some(env_node);
            if let Some(map) = parse_time_signature_envelope(tree, env_node, manual_sig) {
                return map;
            }
        }
    }

    if let Some((n, d)) = manual_sig
        && let Ok(map) = TimeSignatureMap::constant(n, d)
    {
        return map;
    }

    if let Some(tss) = tree.child(ts_node, "TimeSignatures") {
        let mut entries: Vec<(f64, u8, u8)> = Vec::new();
        for rts in tree.children_named(tss, "RemoteableTimeSignature") {
            let num = parse_child_u64(tree, rts, "Numerator").unwrap_or(4) as u8;
            let den = parse_child_u64(tree, rts, "Denominator").unwrap_or(4) as u8;
            let time = parse_child_f64(tree, rts, "Time").unwrap_or(0.0);
            entries.push((time, num, den));
        }
        if let Some(map) = build_time_sig_map_from_beats(&entries) {
            return map;
        }
    }

    TimeSignatureMap::default()
}

fn parse_time_signature_envelope(
    tree: &XmlTree,
    env_node: XmlNodeId,
    manual_sig: Option<(u8, u8)>,
) -> Option<TimeSignatureMap> {
    let events_node = tree
        .child(env_node, "Automation")
        .and_then(|a| tree.child(a, "Events"))?;
    let mut entries: Vec<(f64, u8, u8)> = Vec::new();
    for ev in tree.children_named(events_node, "EnumEvent") {
        let Some(t) = tree.attr(ev, "Time").and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let Some(enc) = tree.attr(ev, "Value").and_then(|s| s.parse::<i64>().ok()) else {
            continue;
        };
        if let Some((n, d)) = decode_time_signature(enc) {
            entries.push((if t <= -1_000_000.0 { 0.0 } else { t }, n, d));
        }
    }
    if entries.is_empty() {
        return None;
    }
    if entries.len() == 1
        && entries[0].0 <= 0.0
        && let Some((mn, md)) = manual_sig
        && (mn, md) != (entries[0].1, entries[0].2)
    {
        return TimeSignatureMap::constant(mn, md).ok();
    }
    build_time_sig_map_from_beats(&entries)
}

fn build_time_sig_map_from_beats(entries: &[(f64, u8, u8)]) -> Option<TimeSignatureMap> {
    if entries.is_empty() {
        return None;
    }
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));

    let (mut cur_n, mut cur_d) = (sorted[0].1, sorted[0].2);
    let mut map = TimeSignatureMap::constant(cur_n, cur_d).ok()?;
    let mut prev_bar = 0i32;
    let mut prev_start = 0.0f64;
    let mut prev_bar_beats = f64::from(cur_n) * 4.0 / f64::from(cur_d);

    for &(beat, n, d) in sorted.iter().skip(1) {
        if beat <= prev_start {
            if let Ok(m) = TimeSignatureMap::constant(n, d) {
                map = m;
                cur_n = n;
                cur_d = d;
                prev_bar_beats = f64::from(cur_n) * 4.0 / f64::from(cur_d);
            }
            continue;
        }
        let delta_bars = ((beat - prev_start) / prev_bar_beats).round().max(1.0) as i32;
        let bar = prev_bar + delta_bars;
        if map.insert(bar, n, d).is_ok() {
            prev_start += f64::from(delta_bars) * prev_bar_beats;
            prev_bar = bar;
            prev_bar_beats = f64::from(n) * 4.0 / f64::from(d);
        }
    }
    Some(map)
}

/// Extracts ordered `(beat, bpm, ramp_to_next)` breakpoints from a [`TempoMap`] using its public API.
#[must_use]
pub fn extract_tempo_points(tempo: &TempoMap) -> Vec<(f64, f64, bool)> {
    if tempo.is_constant() {
        return vec![(0.0, tempo.bpm_at(Beats(0.0)).max(MIN_BPM), false)];
    }
    let mut out = Vec::new();
    let mut cur = tempo.next_change_after(Beats(f64::NEG_INFINITY));
    while let Some(pt) = cur {
        out.push((pt.0, tempo.bpm_at(pt), tempo.ramps_at(pt)));
        cur = tempo.next_change_after(pt);
    }
    if out.is_empty() {
        out.push((0.0, tempo.bpm_at(Beats(0.0)).max(MIN_BPM), false));
    }
    out
}

/// Extracts ordered `(bar, numerator, denominator)` changes from a [`TimeSignatureMap`].
#[must_use]
pub fn extract_time_sig_changes(sig: &TimeSignatureMap) -> Vec<(i32, u8, u8)> {
    let (n0, d0) = sig.signature_at(Beats(0.0));
    let mut out = vec![(0, n0, d0)];
    let Ok(mut recon) = TimeSignatureMap::constant(n0, d0) else {
        return out;
    };
    if recon == *sig {
        return out;
    }
    let mut prev = (n0, d0);
    for bar in 1..=16_384 {
        let cur = sig.signature_at(sig.bar_start(bar));
        if cur != prev {
            let _ = recon.insert(bar, cur.0, cur.1);
            out.push((bar, cur.0, cur.1));
            prev = cur;
            if recon == *sig {
                break;
            }
        }
    }
    out
}

pub(crate) fn patch_globals(
    tree: &mut XmlTree,
    root: XmlNodeId,
    live_set: XmlNodeId,
    baseline: &Project,
    project: &Project,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    if project.creator != baseline.creator && !project.creator.is_empty() {
        tree.set_attr(root, "Creator", &project.creator);
    }
    let main_track_node = entities.main_track;
    if project.transport.tempo != baseline.transport.tempo {
        patch_tempo(
            tree,
            live_set,
            main_track_node,
            &project.transport.tempo,
            entities,
            ids,
        )?;
    }
    if project.transport.time_signature != baseline.transport.time_signature {
        patch_time_signature(
            tree,
            main_track_node,
            &project.transport.time_signature,
            entities,
            ids,
        )?;
    }
    patch_transport_scalars(tree, live_set, &baseline.transport, &project.transport);
    patch_scale_config(tree, live_set, &baseline.scale, &project.scale);
    patch_tuning(tree, live_set, &baseline.tuning, &project.tuning);
    patch_scenes(tree, live_set, &baseline.scenes, &project.scenes, entities)?;
    Ok(())
}

fn patch_tempo(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    main_track_node: Option<XmlNodeId>,
    tempo: &TempoMap,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let points = extract_tempo_points(tempo);
    let bpm0 = format_f64(points[0].1);

    let mixer_tempo = main_track_node
        .and_then(|mt| tree.child(mt, "DeviceChain"))
        .and_then(|dc| tree.child(dc, "Mixer"))
        .and_then(|mx| tree.ensure_child(mx, "Tempo"));
    if let Some(tn) = mixer_tempo {
        set_manual_value(tree, tn, &bpm0);
    }
    if let Some(tr) = tree.child(live_set, "Transport")
        && let Some(tr_tempo) = tree.child(tr, "Tempo")
    {
        set_manual_value(tree, tr_tempo, &bpm0);
    }

    if let (Some(mt), Some(tn)) = (main_track_node, mixer_tempo) {
        if points.len() <= 1 {
            if let Some(env_node) = entities.tempo_envelope_node {
                write_tempo_envelope_events(tree, env_node, &points, ids);
            }
        } else {
            let pid = ensure_automation_target(tree, tn, ids);
            entities.pointee_targets.insert(pid, PointeeTarget::Tempo);
            let env_node = match entities.tempo_envelope_node {
                Some(n) => n,
                None => {
                    let n = create_main_envelope(tree, mt, pid, ids)?;
                    entities.tempo_envelope_node = Some(n);
                    n
                }
            };
            write_tempo_envelope_events(tree, env_node, &points, ids);
        }
    }
    Ok(())
}

fn write_tempo_envelope_events(
    tree: &mut XmlTree,
    env_node: XmlNodeId,
    points: &[(f64, f64, bool)],
    ids: &mut AlsIdAllocator,
) {
    let Some(auto) = tree.ensure_child(env_node, "Automation") else {
        return;
    };
    let Some(events_node) = tree.ensure_child(auto, "Events") else {
        return;
    };
    tree.clear_children(events_node);
    if points.len() <= 1 {
        append_float_event(
            tree,
            events_node,
            ids.alloc(),
            SENTINEL_EVENT_TIME,
            points[0].1,
        );
        return;
    }
    for (idx, &(beat, bpm, _)) in points.iter().enumerate() {
        if idx == 0 {
            let t = if beat == 0.0 {
                SENTINEL_EVENT_TIME
            } else {
                beat
            };
            append_float_event(tree, events_node, ids.alloc(), t, bpm);
        } else {
            let (prev_bpm, prev_ramp) = (points[idx - 1].1, points[idx - 1].2);
            if !prev_ramp {
                append_float_event(tree, events_node, ids.alloc(), beat, prev_bpm);
            }
            append_float_event(tree, events_node, ids.alloc(), beat, bpm);
        }
    }
}

fn append_float_event(tree: &mut XmlTree, parent: XmlNodeId, id: u64, time: f64, value: f64) {
    let ev = tree.alloc_element_with(
        "FloatEvent",
        vec![
            XmlAttr::new("Id", id.to_string()),
            XmlAttr::new("Time", format_f64(time)),
            XmlAttr::new("Value", format_f64(value)),
        ],
        Some(true),
    );
    tree.append_child(parent, ev);
}

fn patch_time_signature(
    tree: &mut XmlTree,
    main_track_node: Option<XmlNodeId>,
    sig: &TimeSignatureMap,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let Some(mt) = main_track_node else {
        return Ok(());
    };
    let Some(ts_node) = tree
        .child(mt, "DeviceChain")
        .and_then(|dc| tree.child(dc, "Mixer"))
        .and_then(|mx| tree.ensure_child(mx, "TimeSignature"))
    else {
        return Ok(());
    };

    let changes = extract_time_sig_changes(sig);
    let (_, n0, d0) = changes[0];
    let encoded0 = encode_time_signature(n0, d0).to_string();

    if tree.child(ts_node, "Manual").is_some() || entities.time_signature_envelope_node.is_some() {
        set_manual_value(tree, ts_node, &encoded0);
    }

    if let Some(tss) = tree.child(ts_node, "TimeSignatures") {
        tree.clear_children(tss);
        for &(bar, num, den) in &changes {
            let beat = if bar == 0 { 0.0 } else { sig.bar_start(bar).0 };
            let rts = tree.alloc_element_with(
                "RemoteableTimeSignature",
                vec![XmlAttr::new("Id", ids.alloc().to_string())],
                Some(false),
            );
            append_value_child(tree, rts, "Numerator", num.to_string());
            append_value_child(tree, rts, "Denominator", den.to_string());
            append_value_child(tree, rts, "Time", format_f64(beat));
            tree.append_child(tss, rts);
        }
    }

    if changes.len() > 1 || entities.time_signature_envelope_node.is_some() {
        let pid = ensure_automation_target(tree, ts_node, ids);
        entities
            .pointee_targets
            .insert(pid, PointeeTarget::TimeSignature);
        let env_node = match entities.time_signature_envelope_node {
            Some(n) => n,
            None => {
                let n = create_main_envelope(tree, mt, pid, ids)?;
                entities.time_signature_envelope_node = Some(n);
                n
            }
        };
        if let Some(auto) = tree.ensure_child(env_node, "Automation")
            && let Some(events_node) = tree.ensure_child(auto, "Events")
        {
            tree.clear_children(events_node);
            for &(bar, num, den) in &changes {
                let beat = if bar == 0 {
                    SENTINEL_EVENT_TIME
                } else {
                    sig.bar_start(bar).0
                };
                let ev = tree.alloc_element_with(
                    "EnumEvent",
                    vec![
                        XmlAttr::new("Id", ids.alloc().to_string()),
                        XmlAttr::new("Time", format_f64(beat)),
                        XmlAttr::new("Value", encode_time_signature(num, den).to_string()),
                    ],
                    Some(true),
                );
                tree.append_child(events_node, ev);
            }
        }
    }

    Ok(())
}

fn ensure_automation_target(
    tree: &mut XmlTree,
    param_node: XmlNodeId,
    ids: &mut AlsIdAllocator,
) -> u64 {
    if let Some(at) = tree.child(param_node, "AutomationTarget")
        && let Some(id_str) = tree.attr(at, "Id")
        && let Ok(id) = id_str.parse::<u64>()
    {
        return id;
    }
    let pid = ids.alloc();
    if let Some(at) = tree.ensure_child(param_node, "AutomationTarget") {
        tree.set_attr(at, "Id", pid.to_string());
        set_child_value(tree, at, "LockEnvelope", "0");
    }
    pid
}

fn create_main_envelope(
    tree: &mut XmlTree,
    main_track_node: XmlNodeId,
    pointee_id: u64,
    ids: &mut AlsIdAllocator,
) -> Result<XmlNodeId, AlsError> {
    let ae = tree
        .ensure_child(main_track_node, "AutomationEnvelopes")
        .and_then(|n| tree.ensure_child(n, "Envelopes"))
        .ok_or_else(|| {
            AlsError::InvalidFormat("failed to create MainTrack Envelopes".to_owned())
        })?;
    let env = instantiate_template_cached(tree, &ENVELOPE_TEMPLATE_CACHE, ENVELOPE_TEMPLATE_XML)?;
    tree.set_attr(env, "Id", ids.alloc().to_string());
    if let Some(et) = tree.ensure_child(env, "EnvelopeTarget") {
        set_child_value(tree, et, "PointeeId", pointee_id.to_string());
    }
    tree.append_child(ae, env);
    Ok(env)
}

fn patch_transport_scalars(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    base: &Transport,
    cur: &Transport,
) {
    if (cur.loop_enabled != base.loop_enabled
        || cur.loop_start_beats != base.loop_start_beats
        || cur.loop_length_beats != base.loop_length_beats
        || cur.playhead_beats != base.playhead_beats
        || cur.punch_in != base.punch_in
        || cur.punch_out != base.punch_out)
        && let Some(tr) = tree.ensure_child(live_set, "Transport")
    {
        if cur.loop_enabled != base.loop_enabled {
            set_child_value(tree, tr, "LoopOn", bool_str(cur.loop_enabled));
        }
        if cur.loop_start_beats != base.loop_start_beats {
            set_child_value(tree, tr, "LoopStart", format_f64(cur.loop_start_beats));
        }
        if cur.loop_length_beats != base.loop_length_beats {
            set_child_value(tree, tr, "LoopLength", format_f64(cur.loop_length_beats));
        }
        if cur.playhead_beats != base.playhead_beats {
            set_child_value(tree, tr, "CurrentTime", format_f64(cur.playhead_beats));
        }
        if cur.punch_in != base.punch_in {
            set_child_value(tree, tr, "PunchIn", bool_str(cur.punch_in));
        }
        if cur.punch_out != base.punch_out {
            set_child_value(tree, tr, "PunchOut", bool_str(cur.punch_out));
        }
    }

    if cur.global_quantization != base.global_quantization
        && let Some(idx) = quantization_to_global_index(cur.global_quantization)
    {
        set_child_value(tree, live_set, "GlobalQuantisation", idx.to_string());
    }

    patch_livecraft_transport(tree, live_set, base, cur);
}
