//! Mappers for Ableton `<Devices>` chains, 16 native devices, and lossless `DeviceType::Custom`
//! placeholders (VST/AU plug-ins, Max for Live devices, racks, and unrecognized Live devices).

use std::collections::{HashMap, HashSet};

use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{Device, DeviceType, Track, param_table};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNode, XmlNodeId, XmlTree};
use super::{
    AlsIdAllocator, EntityMap, PointeeTarget, append_value_child, bool_str, child_value_str,
    devices_equal, f32_eq, format_f32, manual_value_str, parse_bool_str, set_child_value,
    set_manual_value, sync_ordered_children,
};

/// Maps an Ableton XML device tag name to a native [`DeviceType`], or `None` for unknown/custom tags.
#[must_use]
pub fn ableton_tag_to_device_type(tag: &str) -> Option<DeviceType> {
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
        "Chorus" | "Chorus2" => Some(DeviceType::Chorus),
        "AutoPan" => Some(DeviceType::AutoPan),
        "Gate" => Some(DeviceType::Gate),
        "Redux2" | "Redux" => Some(DeviceType::Redux),
        "Limiter" => Some(DeviceType::Limiter),
        _ => None,
    }
}

/// Returns the canonical Ableton XML tag name for a [`DeviceType`].
#[must_use]
pub fn device_type_to_ableton_tag(dtype: &DeviceType) -> &str {
    match dtype {
        DeviceType::Operator => "Operator",
        DeviceType::Simpler => "OriginalSimpler",
        DeviceType::EqEight => "Eq8",
        DeviceType::Compressor => "Compressor2",
        DeviceType::Utility => "StereoGain",
        DeviceType::Reverb => "Reverb",
        DeviceType::Delay => "Delay",
        DeviceType::Saturator => "Saturator",
        DeviceType::DrumRack => "DrumGroupDevice",
        DeviceType::AutoFilter => "AutoFilter",
        DeviceType::EqThree => "FilterEQ3",
        DeviceType::Chorus => "Chorus",
        DeviceType::AutoPan => "AutoPan",
        DeviceType::Gate => "Gate",
        DeviceType::Redux => "Redux2",
        DeviceType::Limiter => "Limiter",
        DeviceType::Custom(tag) => tag.as_str(),
    }
}

/// Returns the default human-readable device name for a [`DeviceType`].
#[must_use]
pub fn default_device_name_for_type(dtype: &DeviceType) -> &str {
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
        DeviceType::Custom(tag) => tag.as_str(),
    }
}

/// Maps a native parameter key + device type to its primary Ableton XML element tag.
#[must_use]
pub(crate) fn param_key_to_xml_tag(dtype: &DeviceType, key: &str) -> Option<&'static str> {
    let tag = match key {
        "cutoff" => "Cutoff",
        "resonance" => "Resonance",
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
        "ping_pong" => "PingPong",
        "decay_time_sec" => "DecayTime",
        "makeup_gain_db" => "MakeupGain",
        "pan" => "Pan",
        "width" => "Width",
        "mute" => "Mute",
        "osc1_waveform" => "Osc1Waveform",
        "osc1_level" => "Osc1Level",
        "osc1_coarse" => "Osc1Coarse",
        "osc2_waveform" => "Osc2Waveform",
        "osc2_level" => "Osc2Level",
        "osc2_coarse" => "Osc2Coarse",
        "algorithm" => "Algorithm",
        "playback_mode" => "PlaybackMode",
        "start_pct" => "StartPct",
        "length_pct" => "LengthPct",
        "crossfade_pct" => "CrossfadePct",
        "curve_type" => "CurveType",
        "output_db" => "Output",
        "tone" => "Tone",
        "filter_type" => "FilterType",
        "lfo_rate" => "LfoRate",
        "lfo_depth" => "LfoDepth",
        "envelope_amount" => "EnvelopeAmount",
        "rate_hz" => "Rate",
        "depth" => "Depth",
        "amount" => "Amount",
        "phase_deg" => "Phase",
        "shape" => "Shape",
        "mode" => "Mode",
        "return_db" => "Return",
        "hold_ms" => "Hold",
        "floor_db" => "Floor",
        "low_gain_db" => "LowGain",
        "mid_gain_db" => "MidGain",
        "high_gain_db" => "HighGain",
        "low_freq" => "LowFreq",
        "high_freq" => "HighFreq",
        "low_on" => "LowOn",
        "mid_on" => "MidOn",
        "high_on" => "HighOn",
        "rate_factor" => "RateFactor",
        "bits" => "Bits",
        "jitter" => "Jitter",
        "band_1_freq" => "Band1Freq",
        "band_1_gain" => "Band1Gain",
        "band_1_q" => "Band1Q",
        "band_1_enabled" => "Band1Enabled",
        "band_2_freq" => "Band2Freq",
        "band_2_gain" => "Band2Gain",
        "band_2_q" => "Band2Q",
        "band_2_enabled" => "Band2Enabled",
        "band_3_freq" => "Band3Freq",
        "band_3_gain" => "Band3Gain",
        "band_3_q" => "Band3Q",
        "band_3_enabled" => "Band3Enabled",
        "band_4_freq" => "Band4Freq",
        "band_4_gain" => "Band4Gain",
        "band_4_q" => "Band4Q",
        "band_4_enabled" => "Band4Enabled",
        "band_5_freq" => "Band5Freq",
        "band_5_gain" => "Band5Gain",
        "band_5_q" => "Band5Q",
        "band_5_enabled" => "Band5Enabled",
        "band_6_freq" => "Band6Freq",
        "band_6_gain" => "Band6Gain",
        "band_6_q" => "Band6Q",
        "band_6_enabled" => "Band6Enabled",
        "band_7_freq" => "Band7Freq",
        "band_7_gain" => "Band7Gain",
        "band_7_q" => "Band7Q",
        "band_7_enabled" => "Band7Enabled",
        "band_8_freq" => "Band8Freq",
        "band_8_gain" => "Band8Gain",
        "band_8_q" => "Band8Q",
        "band_8_enabled" => "Band8Enabled",
        _ => return None,
    };
    let _ = dtype;
    Some(tag)
}

#[must_use]
fn xml_tag_to_param_key(dtype: &DeviceType, tag: &str) -> Option<&'static str> {
    if *dtype == DeviceType::AutoFilter {
        match tag {
            "FilterCutoff" | "Cutoff" => return Some("cutoff"),
            "FilterResonance" | "Resonance" => return Some("resonance"),
            _ => {}
        }
    }
    match tag {
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
        "PingPong" => Some("ping_pong"),
        "DecayTime" => Some("decay_time_sec"),
        "MakeupGain" => Some("makeup_gain_db"),
        "Pan" => Some("pan"),
        "Width" => Some("width"),
        "Mute" => Some("mute"),
        "Osc1Waveform" => Some("osc1_waveform"),
        "Osc1Level" => Some("osc1_level"),
        "Osc1Coarse" => Some("osc1_coarse"),
        "Osc2Waveform" => Some("osc2_waveform"),
        "Osc2Level" => Some("osc2_level"),
        "Osc2Coarse" => Some("osc2_coarse"),
        "Algorithm" => Some("algorithm"),
        "PlaybackMode" => Some("playback_mode"),
        "StartPct" => Some("start_pct"),
        "LengthPct" => Some("length_pct"),
        "CrossfadePct" => Some("crossfade_pct"),
        "CurveType" => Some("curve_type"),
        "Output" => Some("output_db"),
        "Tone" => Some("tone"),
        "FilterType" => Some("filter_type"),
        "LfoRate" => Some("lfo_rate"),
        "LfoDepth" => Some("lfo_depth"),
        "EnvelopeAmount" => Some("envelope_amount"),
        "Rate" => Some("rate_hz"),
        "Depth" => Some("depth"),
        "Amount" => Some("amount"),
        "Phase" => Some("phase_deg"),
        "Shape" => Some("shape"),
        "Mode" => Some("mode"),
        "Return" => Some("return_db"),
        "Hold" => Some("hold_ms"),
        "Floor" => Some("floor_db"),
        "LowGain" => Some("low_gain_db"),
        "MidGain" => Some("mid_gain_db"),
        "HighGain" => Some("high_gain_db"),
        "LowFreq" => Some("low_freq"),
        "HighFreq" => Some("high_freq"),
        "LowOn" => Some("low_on"),
        "MidOn" => Some("mid_on"),
        "HighOn" => Some("high_on"),
        "RateFactor" => Some("rate_factor"),
        "Bits" => Some("bits"),
        "Jitter" => Some("jitter"),
        _ => parse_eq8_band_tag(tag),
    }
}

#[must_use]
fn parse_eq8_band_tag(tag: &str) -> Option<&'static str> {
    match tag {
        "Band1Freq" => Some("band_1_freq"),
        "Band1Gain" => Some("band_1_gain"),
        "Band1Q" => Some("band_1_q"),
        "Band1Enabled" => Some("band_1_enabled"),
        "Band2Freq" => Some("band_2_freq"),
        "Band2Gain" => Some("band_2_gain"),
        "Band2Q" => Some("band_2_q"),
        "Band2Enabled" => Some("band_2_enabled"),
        "Band3Freq" => Some("band_3_freq"),
        "Band3Gain" => Some("band_3_gain"),
        "Band3Q" => Some("band_3_q"),
        "Band3Enabled" => Some("band_3_enabled"),
        "Band4Freq" => Some("band_4_freq"),
        "Band4Gain" => Some("band_4_gain"),
        "Band4Q" => Some("band_4_q"),
        "Band4Enabled" => Some("band_4_enabled"),
        "Band5Freq" => Some("band_5_freq"),
        "Band5Gain" => Some("band_5_gain"),
        "Band5Q" => Some("band_5_q"),
        "Band5Enabled" => Some("band_5_enabled"),
        "Band6Freq" => Some("band_6_freq"),
        "Band6Gain" => Some("band_6_gain"),
        "Band6Q" => Some("band_6_q"),
        "Band6Enabled" => Some("band_6_enabled"),
        "Band7Freq" => Some("band_7_freq"),
        "Band7Gain" => Some("band_7_gain"),
        "Band7Q" => Some("band_7_q"),
        "Band7Enabled" => Some("band_7_enabled"),
        "Band8Freq" => Some("band_8_freq"),
        "Band8Gain" => Some("band_8_gain"),
        "Band8Q" => Some("band_8_q"),
        "Band8Enabled" => Some("band_8_enabled"),
        _ => None,
    }
}

#[must_use]
pub(crate) fn find_devices_container(tree: &XmlTree, dc: XmlNodeId) -> Option<XmlNodeId> {
    tree.child(dc, "DeviceChain")
        .and_then(|inner| tree.child(inner, "Devices"))
        .or_else(|| tree.child(dc, "Devices"))
}

pub(crate) fn read_track_devices(
    tree: &XmlTree,
    track_id: TrackId,
    dc: XmlNodeId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<Vec<Device>, AlsError> {
    let Some(devices_node) = find_devices_container(tree, dc) else {
        return Ok(Vec::new());
    };
    let mut used = HashSet::new();
    let mut out = Vec::new();
    for (idx, child) in tree.children(devices_node).iter().copied().enumerate() {
        let Some(tag) = tree.node(child).and_then(XmlNode::element_name) else {
            continue;
        };
        let dev = read_single_device(tree, child, tag, track_id, idx, &mut used, entities, ids);
        out.push(dev);
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn read_single_device(
    tree: &XmlTree,
    node: XmlNodeId,
    tag: &str,
    track_id: TrackId,
    idx: usize,
    used: &mut HashSet<u64>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Device {
    if tag == "Device" {
        let parsed_id = tree.attr(node, "Id").and_then(|s| s.parse::<u64>().ok());
        let id = alloc_dev_id(parsed_id, track_id, idx, used, ids);
        let dtype_str = tree.attr(node, "Type").unwrap_or("Custom");
        let dtype = ableton_tag_to_device_type(dtype_str)
            .unwrap_or_else(|| DeviceType::Custom(dtype_str.to_owned()));
        let name = tree
            .attr(node, "Name")
            .map(str::to_owned)
            .unwrap_or_else(|| default_device_name_for_type(&dtype).to_owned());
        let mut dev = Device::new(DeviceId(id), name, dtype);
        if let Some(enabled) = tree.attr(node, "IsEnabled").and_then(parse_bool_str) {
            dev.is_enabled = enabled;
        }
        for p in tree.children_named(node, "Parameter") {
            if let (Some(k), Some(v)) = (
                tree.attr(p, "Name"),
                tree.value_attr(p).and_then(|s| s.parse::<f32>().ok()),
            ) {
                dev.set_param(k, v);
            }
        }
        entities.devices.insert(dev.id, node);
        return dev;
    }

    let parsed_id = tree.attr(node, "Id").and_then(|s| s.parse::<u64>().ok());
    let id = alloc_dev_id(parsed_id, track_id, idx, used, ids);
    let dtype =
        ableton_tag_to_device_type(tag).unwrap_or_else(|| DeviceType::Custom(tag.to_owned()));
    let name = read_device_name(tree, node)
        .unwrap_or_else(|| default_device_name_for_type(&dtype).to_owned());
    let mut dev = Device::new(DeviceId(id), name, dtype.clone());

    if let Some(on_node) = tree.child(node, "On")
        && let Some(on_val) = manual_value_str(tree, on_node).and_then(parse_bool_str)
    {
        dev.is_enabled = on_val;
    }

    if !matches!(dtype, DeviceType::Custom(_)) {
        read_native_device_params(tree, node, track_id, &mut dev, entities);
    }

    entities.devices.insert(dev.id, node);
    dev
}

fn alloc_dev_id(
    parsed: Option<u64>,
    track_id: TrackId,
    idx: usize,
    used: &mut HashSet<u64>,
    ids: &mut AlsIdAllocator,
) -> u64 {
    let id = match parsed {
        Some(v) if v > 0 && !used.contains(&v) => v,
        _ => {
            let mut cand = (track_id.0 * 100) + idx as u64 + 1;
            while used.contains(&cand) || cand == 0 {
                cand = ids.alloc();
            }
            cand
        }
    };
    used.insert(id);
    id
}

fn read_device_name(tree: &XmlTree, node: XmlNodeId) -> Option<String> {
    if let Some(u) = child_value_str(tree, node, "UserName")
        && !u.is_empty()
    {
        return Some(u.to_owned());
    }
    if let Some(e) = child_value_str(tree, node, "EffectiveName")
        && !e.is_empty()
    {
        return Some(e.to_owned());
    }
    if let Some(n) = tree.child(node, "Name") {
        if let Some(u) = child_value_str(tree, n, "UserName")
            && !u.is_empty()
        {
            return Some(u.to_owned());
        }
        if let Some(e) = child_value_str(tree, n, "EffectiveName")
            && !e.is_empty()
        {
            return Some(e.to_owned());
        }
        if let Some(v) = tree.value_attr(n)
            && !v.is_empty()
        {
            return Some(v.to_owned());
        }
    }
    None
}

fn read_native_device_params(
    tree: &XmlTree,
    node: XmlNodeId,
    track_id: TrackId,
    dev: &mut Device,
    entities: &mut EntityMap,
) {
    for &child in tree.children(node) {
        let Some(tag) = tree.node(child).and_then(XmlNode::element_name) else {
            continue;
        };
        if tag == "Parameter" {
            if let (Some(k), Some(v)) = (
                tree.attr(child, "Name"),
                tree.value_attr(child)
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| v.is_finite()),
            ) {
                dev.set_param(k, v);
            }
            continue;
        }
        if let Some(key) = xml_tag_to_param_key(&dev.device_type, tag) {
            if let Some(at) = tree.child(child, "AutomationTarget")
                && let Some(pid) = tree.attr(at, "Id").and_then(|s| s.parse::<u64>().ok())
            {
                entities.pointee_targets.insert(
                    pid,
                    PointeeTarget::DeviceParam {
                        track_id,
                        device_id: dev.id,
                        param_key: key.to_owned(),
                    },
                );
            }
            if let Some(val) = manual_value_str(tree, child)
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite())
            {
                dev.set_param(key, val);
            }
        }
    }

    if dev.device_type == DeviceType::EqEight {
        for band_idx in 0..8usize {
            let band_tag = format!("Bands.{band_idx}");
            if let Some(band_node) = tree.child(node, &band_tag)
                && let Some(pa) = tree
                    .child(band_node, "ParameterA")
                    .or_else(|| tree.child(band_node, "ParameterB"))
            {
                let b1 = band_idx + 1;
                let pairs = [
                    ("Freq", format!("band_{b1}_freq")),
                    ("Gain", format!("band_{b1}_gain")),
                    ("Q", format!("band_{b1}_q")),
                    ("IsOn", format!("band_{b1}_enabled")),
                ];
                for (sub, key) in pairs {
                    if let Some(sn) = tree.child(pa, sub) {
                        if let Some(at) = tree.child(sn, "AutomationTarget")
                            && let Some(pid) =
                                tree.attr(at, "Id").and_then(|s| s.parse::<u64>().ok())
                        {
                            entities.pointee_targets.insert(
                                pid,
                                PointeeTarget::DeviceParam {
                                    track_id,
                                    device_id: dev.id,
                                    param_key: key.clone(),
                                },
                            );
                        }
                        if let Some(v_str) = manual_value_str(tree, sn) {
                            if let Ok(v) = v_str.parse::<f32>()
                                && v.is_finite()
                            {
                                dev.set_param(&key, v);
                            } else if let Some(b) = parse_bool_str(v_str) {
                                dev.set_param(&key, if b { 1.0 } else { 0.0 });
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn find_device_param_node(
    tree: &XmlTree,
    dev_node: XmlNodeId,
    dtype: &DeviceType,
    param_key: &str,
) -> Option<XmlNodeId> {
    let tag = param_key_to_xml_tag(dtype, param_key)?;
    if let Some(direct) = tree.child(dev_node, tag) {
        return Some(direct);
    }
    if *dtype == DeviceType::EqEight
        && let Some(rest) = param_key.strip_prefix("band_")
        && let Some((num_s, field)) = rest.split_once('_')
        && let Ok(band_one_based) = num_s.parse::<usize>()
        && (1..=8).contains(&band_one_based)
    {
        let sub = match field {
            "freq" => "Freq",
            "gain" => "Gain",
            "q" => "Q",
            "enabled" => "IsOn",
            _ => return None,
        };
        let band_tag = format!("Bands.{}", band_one_based - 1);
        return tree
            .child(dev_node, &band_tag)
            .and_then(|bn| tree.child(bn, "ParameterA"))
            .and_then(|pa| tree.child(pa, sub));
    }
    None
}

pub(crate) fn ensure_device_param_node(
    tree: &mut XmlTree,
    dev_node: XmlNodeId,
    dtype: &DeviceType,
    param_key: &str,
) -> Option<XmlNodeId> {
    if let Some(existing) = find_device_param_node(tree, dev_node, dtype, param_key) {
        return Some(existing);
    }
    let tag = param_key_to_xml_tag(dtype, param_key)?;
    tree.ensure_child(dev_node, tag)
}

pub(crate) fn patch_track_devices(
    tree: &mut XmlTree,
    dc: XmlNodeId,
    prev: Option<&Track>,
    cur: &Track,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Result<(), AlsError> {
    let unchanged = prev.is_some_and(|p| {
        p.device_chain.len() == cur.device_chain.len()
            && p.device_chain
                .iter()
                .zip(&cur.device_chain)
                .all(|(a, b)| devices_equal(a, b))
    });
    if unchanged {
        return Ok(());
    }

    if cur.device_chain.is_empty() && find_devices_container(tree, dc).is_none() {
        return Ok(());
    }

    let devices_node = if let Some(existing) = find_devices_container(tree, dc) {
        existing
    } else {
        tree.ensure_child(dc, "DeviceChain")
            .and_then(|inner| tree.ensure_child(inner, "Devices"))
            .ok_or_else(|| AlsError::InvalidFormat("failed to create Devices node".to_owned()))?
    };

    let prev_by_id: HashMap<DeviceId, &Device> = prev
        .map(|p| p.device_chain.iter().map(|d| (d.id, d)).collect())
        .unwrap_or_default();

    let mut desired = Vec::with_capacity(cur.device_chain.len());
    for dev in &cur.device_chain {
        let expected_tag = device_type_to_ableton_tag(&dev.device_type);
        let existing = entities.devices.get(&dev.id).copied().filter(|&nid| {
            tree.node(nid).is_some_and(|n| {
                n.parent == Some(devices_node) && n.element_name() == Some(expected_tag)
            })
        });

        let dev_node = if let Some(nid) = existing {
            if let Some(prev_dev) = prev_by_id.get(&dev.id).copied() {
                if !devices_equal(prev_dev, dev) {
                    patch_device_node(tree, nid, Some(prev_dev), dev);
                }
            } else {
                patch_device_node(tree, nid, None, dev);
            }
            nid
        } else {
            let fresh = tree.alloc_element_with(
                expected_tag,
                vec![XmlAttr::new("Id", dev.id.0.to_string())],
                Some(false),
            );
            patch_device_node(tree, fresh, None, dev);
            let _ = ids;
            fresh
        };

        entities.devices.insert(dev.id, dev_node);
        desired.push(dev_node);
    }

    sync_ordered_children(tree, devices_node, |n| n.element_name().is_some(), desired);
    Ok(())
}

fn patch_device_node(tree: &mut XmlTree, node: XmlNodeId, prev: Option<&Device>, cur: &Device) {
    if prev.is_none() {
        if !cur.name.is_empty() {
            append_value_child(tree, node, "UserName", &cur.name);
        }
        let on_node = tree.alloc_element("On");
        tree.append_child(node, on_node);
        append_value_child(tree, on_node, "Manual", bool_str(cur.is_enabled));

        if matches!(cur.device_type, DeviceType::Custom(_)) {
            return;
        }
        for (key, val) in cur.named_params() {
            if let Some(tag) = param_key_to_xml_tag(&cur.device_type, key) {
                let pnode = tree.alloc_element(tag);
                tree.append_child(node, pnode);
                append_value_child(tree, pnode, "Manual", format_f32(val));
            }
        }
        return;
    }

    if prev.is_none_or(|p| p.name != cur.name) && !cur.name.is_empty() {
        set_child_value(tree, node, "UserName", &cur.name);
    }
    if prev.is_none_or(|p| p.is_enabled != cur.is_enabled)
        && let Some(on_node) = tree.ensure_child(node, "On")
    {
        set_manual_value(tree, on_node, bool_str(cur.is_enabled));
    }

    if matches!(cur.device_type, DeviceType::Custom(_)) {
        // Never alter unknown/plug-in device internals beyond Name and On.
        return;
    }

    let table = param_table(&cur.device_type);
    for spec in table {
        let cur_val = cur.param(spec.name);
        let prev_val = prev.and_then(|p| p.param(spec.name));
        let changed = match (prev_val, cur_val) {
            (Some(a), Some(b)) => !f32_eq(a, b),
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        };
        if !changed {
            continue;
        }
        match cur_val {
            Some(val) => {
                if let Some(pnode) =
                    ensure_device_param_node(tree, node, &cur.device_type, spec.name)
                {
                    set_manual_value(tree, pnode, format_f32(val));
                }
            }
            None => {
                if let Some(pnode) = find_device_param_node(tree, node, &cur.device_type, spec.name)
                {
                    tree.remove_child(node, pnode);
                }
            }
        }
    }
}
