//! Ableton Device (`.adv`) and Rack (`.adg`) preset gzip XML reader and writer.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use livecraft_model::ids::TrackId;
use livecraft_model::model::{Device, DeviceType};

use crate::als::AlsError;
use crate::als::map::{
    AlsIdAllocator, EntityMap, append_value_child, child_value_str, devices, format_f32,
    manual_value_str,
};
use crate::als::read::read_als_tree;
use crate::als::tree::{XmlAttr, XmlNode, XmlNodeId, XmlTree};
use crate::als::write::write_als_gz;

/// Ableton Rack kind selecting the `<*GroupDevice>` and `<*BranchPreset>` XML tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RackKind {
    #[default]
    AudioEffect,
    Instrument,
    MidiEffect,
}

/// A Rack preset (`.adg`) holding a rack name, its device chain, and 8 macro values.
#[derive(Debug, Clone)]
pub struct RackPreset {
    pub name: String,
    pub devices: Vec<Device>,
    pub macros: [f32; 8],
}

fn is_instrument_device(dtype: &DeviceType) -> bool {
    match dtype {
        DeviceType::Operator | DeviceType::Simpler | DeviceType::DrumRack => true,
        DeviceType::Custom(tag) => {
            matches!(
                tag.as_str(),
                "InstrumentGroupDevice"
                    | "MxDeviceInstrument"
                    | "Sampler"
                    | "Wavetable"
                    | "Drift"
                    | "Meld"
                    | "Analog"
                    | "Electric"
                    | "Collision"
                    | "Tension"
                    | "Impulse"
            ) || tag.contains("Instrument")
        }
        _ => false,
    }
}

fn is_midi_effect_device(dtype: &DeviceType) -> bool {
    match dtype {
        DeviceType::Custom(tag) => {
            matches!(
                tag.as_str(),
                "MidiEffectGroupDevice"
                    | "MxDeviceMidiEffect"
                    | "Arpeggiator"
                    | "Chord"
                    | "NoteLength"
                    | "Pitch"
                    | "Random"
                    | "Scale"
                    | "Velocity"
            ) || tag.contains("MidiEffect")
        }
        _ => false,
    }
}

/// Infers the Ableton [`RackKind`] from the devices inside a rack preset.
#[must_use]
pub fn infer_rack_kind(devices: &[Device]) -> RackKind {
    if devices.iter().any(|d| is_instrument_device(&d.device_type)) {
        return RackKind::Instrument;
    }
    if !devices.is_empty()
        && devices
            .iter()
            .all(|d| is_midi_effect_device(&d.device_type))
    {
        return RackKind::MidiEffect;
    }
    RackKind::AudioEffect
}

/// Saves a single [`Device`] as a gzip-compressed `.adv` device preset.
pub fn save_device_preset(device: &Device, path: &Path) -> Result<(), AlsError> {
    let mut tree = new_ableton_tree();
    let root = tree.root();

    let tag = devices::device_type_to_ableton_tag(&device.device_type);
    let dev_node = tree.alloc_element_with(
        tag,
        vec![XmlAttr::new("Id", device.id.0.to_string())],
        Some(false),
    );
    tree.append_child(root, dev_node);
    devices::patch_device_node(&mut tree, dev_node, None, device);

    ensure_parent_dir(path)?;
    write_als_gz(&tree, path)
}

/// Loads a single [`Device`] from a gzip-compressed `.adv` (or plain XML) preset file.
pub fn load_device_preset(path: &Path) -> Result<Device, AlsError> {
    let tree = read_als_tree(path)?;
    let root = verify_ableton_root(&tree)?;

    let dev_node = find_device_preset_node(&tree, root).ok_or_else(|| {
        AlsError::InvalidFormat("device preset (.adv) contains no device element".to_owned())
    })?;
    let tag = tree
        .node(dev_node)
        .and_then(XmlNode::element_name)
        .ok_or_else(|| AlsError::InvalidFormat("invalid device element".to_owned()))?;

    let mut used = HashSet::new();
    let mut entities = EntityMap::default();
    let mut ids = AlsIdAllocator::new(1);
    Ok(devices::read_single_device(
        &tree,
        dev_node,
        tag,
        TrackId(0),
        0,
        &mut used,
        &mut entities,
        &mut ids,
    ))
}

/// Saves a [`RackPreset`] as a gzip-compressed `.adg` rack preset, inferring its [`RackKind`].
pub fn save_rack_preset(preset: &RackPreset, path: &Path) -> Result<(), AlsError> {
    save_rack_preset_with_kind(preset, path, infer_rack_kind(&preset.devices))
}

/// Saves a [`RackPreset`] as a gzip-compressed `.adg` rack preset with an explicit [`RackKind`].
pub fn save_rack_preset_with_kind(
    preset: &RackPreset,
    path: &Path,
    kind: RackKind,
) -> Result<(), AlsError> {
    let (group_tag, branch_tag) = match kind {
        RackKind::AudioEffect => ("AudioEffectGroupDevice", "AudioEffectBranchPreset"),
        RackKind::Instrument => ("InstrumentGroupDevice", "InstrumentBranchPreset"),
        RackKind::MidiEffect => ("MidiEffectGroupDevice", "MidiEffectBranchPreset"),
    };

    let mut tree = new_ableton_tree();
    let root = tree.root();

    let group_preset = tree.alloc_element("GroupDevicePreset");
    tree.append_child(root, group_preset);
    append_value_child(&mut tree, group_preset, "Name", &preset.name);

    let device_wrap = tree.alloc_element("Device");
    tree.append_child(group_preset, device_wrap);
    let rack_dev = tree.alloc_element_with(group_tag, vec![XmlAttr::new("Id", "0")], Some(false));
    tree.append_child(device_wrap, rack_dev);
    append_value_child(&mut tree, rack_dev, "UserName", &preset.name);

    for (idx, &val) in preset.macros.iter().enumerate() {
        let mc_tag = format!("MacroControls.{idx}");
        let mc_node = tree.alloc_element(mc_tag);
        tree.append_child(rack_dev, mc_node);
        append_value_child(&mut tree, mc_node, "Manual", format_f32(val));
    }

    let branches = tree.alloc_element("BranchPresets");
    tree.append_child(group_preset, branches);
    let branch = tree.alloc_element_with(branch_tag, vec![XmlAttr::new("Id", "0")], Some(false));
    tree.append_child(branches, branch);
    append_value_child(&mut tree, branch, "Name", "");
    let dev_presets = tree.alloc_element("DevicePresets");
    tree.append_child(branch, dev_presets);

    for (idx, dev) in preset.devices.iter().enumerate() {
        let adp = tree.alloc_element_with(
            "AbletonDevicePreset",
            vec![XmlAttr::new("Id", idx.to_string())],
            Some(false),
        );
        tree.append_child(dev_presets, adp);
        let dev_slot = tree.alloc_element("Device");
        tree.append_child(adp, dev_slot);

        let tag = devices::device_type_to_ableton_tag(&dev.device_type);
        let dev_node = tree.alloc_element_with(
            tag,
            vec![XmlAttr::new("Id", dev.id.0.to_string())],
            Some(false),
        );
        tree.append_child(dev_slot, dev_node);
        devices::patch_device_node(&mut tree, dev_node, None, dev);
    }

    ensure_parent_dir(path)?;
    write_als_gz(&tree, path)
}

/// Loads a [`RackPreset`] from a gzip-compressed `.adg` (or plain XML) rack preset file.
pub fn load_rack_preset(path: &Path) -> Result<RackPreset, AlsError> {
    load_rack_preset_with_kind(path).map(|(preset, _)| preset)
}

/// Loads a [`RackPreset`] and its detected [`RackKind`] from an `.adg` rack preset file.
pub fn load_rack_preset_with_kind(path: &Path) -> Result<(RackPreset, RackKind), AlsError> {
    let tree = read_als_tree(path)?;
    let root = verify_ableton_root(&tree)?;

    let group_node = tree
        .child(root, "GroupDevicePreset")
        .or_else(|| tree.child(root, "RackPreset"))
        .ok_or_else(|| {
            AlsError::InvalidFormat(
                "rack preset (.adg) missing <GroupDevicePreset> or <RackPreset>".to_owned(),
            )
        })?;

    let rack_device_node = tree
        .child(group_node, "Device")
        .and_then(|d| first_child_element(&tree, d));

    let name = child_value_str(&tree, group_node, "Name")
        .map(str::to_owned)
        .or_else(|| rack_device_node.and_then(|rd| devices::read_device_name(&tree, rd)))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("Rack")
                .to_owned()
        });

    let mut macros = [0.0_f32; 8];
    let macro_host = rack_device_node.unwrap_or(group_node);
    for (idx, slot) in macros.iter_mut().enumerate() {
        let mc_tag = format!("MacroControls.{idx}");
        if let Some(mc_node) = tree
            .child(macro_host, &mc_tag)
            .or_else(|| tree.child(group_node, &mc_tag))
            && let Some(val_str) =
                manual_value_str(&tree, mc_node).or_else(|| tree.value_attr(mc_node))
            && let Ok(val) = val_str.parse::<f32>()
            && val.is_finite()
        {
            *slot = val;
        }
    }

    let mut used = HashSet::new();
    let mut entities = EntityMap::default();
    let mut ids = AlsIdAllocator::new(1);
    let mut out_devices = Vec::new();
    collect_rack_devices(
        &tree,
        group_node,
        &mut out_devices,
        &mut used,
        &mut entities,
        &mut ids,
    );

    let kind_from_dev = rack_device_node
        .and_then(|n| tree.node(n))
        .and_then(XmlNode::element_name)
        .and_then(|tag| match tag {
            "InstrumentGroupDevice" => Some(RackKind::Instrument),
            "MidiEffectGroupDevice" => Some(RackKind::MidiEffect),
            "AudioEffectGroupDevice" => Some(RackKind::AudioEffect),
            _ => None,
        });
    let kind_from_branch = tree
        .child(group_node, "BranchPresets")
        .and_then(|bs| first_child_element(&tree, bs))
        .and_then(|b| tree.node(b))
        .and_then(XmlNode::element_name)
        .and_then(|tag| match tag {
            "InstrumentBranchPreset" => Some(RackKind::Instrument),
            "MidiEffectBranchPreset" => Some(RackKind::MidiEffect),
            "AudioEffectBranchPreset" => Some(RackKind::AudioEffect),
            _ => None,
        });
    let kind = kind_from_dev
        .or(kind_from_branch)
        .unwrap_or_else(|| infer_rack_kind(&out_devices));

    Ok((
        RackPreset {
            name,
            devices: out_devices,
            macros,
        },
        kind,
    ))
}

fn new_ableton_tree() -> XmlTree {
    XmlTree::with_root(
        "Ableton",
        vec![
            XmlAttr::new("MajorVersion", "5"),
            XmlAttr::new("MinorVersion", "12.0_12117"),
            XmlAttr::new("SchemaChangeCount", "10"),
            XmlAttr::new("Creator", "LiveCraft 0.1.0"),
        ],
    )
}

fn verify_ableton_root(tree: &XmlTree) -> Result<XmlNodeId, AlsError> {
    let root = tree.root();
    let root_name = tree.node(root).and_then(XmlNode::element_name);
    if root_name != Some("Ableton") {
        return Err(AlsError::InvalidFormat(format!(
            "expected `<Ableton>` root element, got `{}`",
            root_name.unwrap_or("<non-element>")
        )));
    }
    Ok(root)
}

fn ensure_parent_dir(path: &Path) -> Result<(), AlsError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn first_child_element(tree: &XmlTree, parent: XmlNodeId) -> Option<XmlNodeId> {
    tree.children(parent)
        .iter()
        .copied()
        .find(|&cid| tree.node(cid).and_then(XmlNode::element_name).is_some())
}

fn find_device_preset_node(tree: &XmlTree, root: XmlNodeId) -> Option<XmlNodeId> {
    if let Some(dev_wrap) = tree.child(root, "Device") {
        if tree.attr(dev_wrap, "Type").is_some() {
            return Some(dev_wrap);
        }
        if let Some(inner) = first_child_element(tree, dev_wrap) {
            return Some(inner);
        }
        return Some(dev_wrap);
    }

    if let Some(preset) = tree
        .child(root, "Preset")
        .or_else(|| tree.child(root, "AbletonDevicePreset"))
    {
        if let Some(dev_wrap) = tree.child(preset, "Device") {
            return first_child_element(tree, dev_wrap).or(Some(dev_wrap));
        }
        if let Some(inner) = first_child_element(tree, preset) {
            return Some(inner);
        }
    }

    if let Some(group) = tree.child(root, "GroupDevicePreset")
        && let Some(dev_wrap) = tree.child(group, "Device")
        && let Some(inner) = first_child_element(tree, dev_wrap)
    {
        return Some(inner);
    }

    for &cid in tree.children(root) {
        if let Some(tag) = tree.node(cid).and_then(XmlNode::element_name)
            && !matches!(tag, "OverwriteProtectionNumber" | "PresetRef")
        {
            return Some(cid);
        }
    }
    None
}

fn collect_rack_devices(
    tree: &XmlTree,
    group_node: XmlNodeId,
    out: &mut Vec<Device>,
    used: &mut HashSet<u64>,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) {
    if let Some(devices_el) = tree.child(group_node, "Devices") {
        for &cid in tree.children(devices_el) {
            if let Some(tag) = tree.node(cid).and_then(XmlNode::element_name) {
                let idx = out.len();
                out.push(devices::read_single_device(
                    tree,
                    cid,
                    tag,
                    TrackId(0),
                    idx,
                    used,
                    entities,
                    ids,
                ));
            }
        }
    }

    let Some(branches) = tree.child(group_node, "BranchPresets") else {
        return;
    };
    for &branch in tree.children(branches) {
        if tree.node(branch).and_then(XmlNode::element_name).is_none() {
            continue;
        }
        let Some(dev_presets) = tree.child(branch, "DevicePresets") else {
            continue;
        };
        for &dp in tree.children(dev_presets) {
            let Some(dp_tag) = tree.node(dp).and_then(XmlNode::element_name) else {
                continue;
            };
            if dp_tag == "GroupDevicePreset" {
                collect_rack_devices(tree, dp, out, used, entities, ids);
                continue;
            }
            let dev_node = if dp_tag == "AbletonDevicePreset" {
                tree.child(dp, "Device").and_then(|dw| {
                    if tree.attr(dw, "Type").is_some() {
                        Some(dw)
                    } else {
                        first_child_element(tree, dw)
                    }
                })
            } else {
                Some(dp)
            };
            if let Some(dn) = dev_node
                && let Some(tag) = tree.node(dn).and_then(XmlNode::element_name)
            {
                let idx = out.len();
                out.push(devices::read_single_device(
                    tree,
                    dn,
                    tag,
                    TrackId(0),
                    idx,
                    used,
                    entities,
                    ids,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::read::decompress_als_or_xml;
    use livecraft_model::ids::DeviceId;
    use livecraft_model::model::{DeviceSampleRef, DeviceType};
    use proptest::prelude::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("livecraft_presets_unit_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn device_preset_roundtrips_native_and_custom_devices() {
        let dir = temp_dir("adv");
        let adv_path = dir.join("WarmEQ.adv");

        let mut eq = Device::new(DeviceId(77), "Vocal Shelf", DeviceType::EqEight);
        eq.is_enabled = false;
        eq.set_param("band_1_freq", 95.0);
        eq.set_param("band_1_gain", -3.5);
        eq.set_param("band_1_enabled", 1.0);
        eq.set_param("band_5_gain", 2.25);

        save_device_preset(&eq, &adv_path).expect("save .adv");
        let loaded = load_device_preset(&adv_path).expect("load .adv");

        assert_eq!(loaded.id, DeviceId(77));
        assert_eq!(loaded.name, "Vocal Shelf");
        assert_eq!(loaded.device_type, DeviceType::EqEight);
        assert!(!loaded.is_enabled);
        assert!((loaded.get_param("band_1_freq") - 95.0).abs() < 1e-3);
        assert!((loaded.get_param("band_1_gain") - (-3.5)).abs() < 1e-3);
        assert!((loaded.get_param("band_5_gain") - 2.25).abs() < 1e-3);

        let custom = Device::new(
            DeviceId(88),
            "External Synth",
            DeviceType::Custom("AuPluginDevice".to_owned()),
        );
        let custom_path = dir.join("Custom.adv");
        save_device_preset(&custom, &custom_path).expect("save custom .adv");
        let loaded_custom = load_device_preset(&custom_path).expect("load custom .adv");
        assert_eq!(loaded_custom.id, DeviceId(88));
        assert_eq!(loaded_custom.name, "External Synth");
        assert_eq!(
            loaded_custom.device_type,
            DeviceType::Custom("AuPluginDevice".to_owned())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rack_preset_roundtrips_devices_and_macros() {
        let dir = temp_dir("adg");
        let adg_path = dir.join("MasteringChain.adg");

        let mut comp = Device::new(DeviceId(11), "Glue", DeviceType::Compressor);
        comp.set_param("threshold_db", -14.0);
        comp.set_param("ratio", 3.0);

        let mut lim = Device::new(DeviceId(12), "Ceiling", DeviceType::Limiter);
        lim.set_param("ceiling_db", -0.8);

        let preset = RackPreset {
            name: "Clean Master".to_owned(),
            devices: vec![comp, lim],
            macros: [0.1, 0.25, 0.5, 0.75, 1.0, 0.0, 0.33, 0.9],
        };
        save_rack_preset(&preset, &adg_path).expect("save .adg");

        let loaded = load_rack_preset(&adg_path).expect("load .adg");
        assert_eq!(loaded.name, "Clean Master");
        assert_eq!(loaded.devices.len(), 2);
        assert_eq!(loaded.devices[0].device_type, DeviceType::Compressor);
        assert!((loaded.devices[0].get_param("threshold_db") - (-14.0)).abs() < 1e-3);
        assert_eq!(loaded.devices[1].device_type, DeviceType::Limiter);
        assert!((loaded.devices[1].get_param("ceiling_db") - (-0.8)).abs() < 1e-3);
        for (a, b) in preset.macros.iter().zip(&loaded.macros) {
            assert!((*a - *b).abs() < 1e-5, "macro mismatch {a} vs {b}");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_ableton_wrapped_device_and_rack_variants() {
        let dir = temp_dir("wrappers");
        let wrapped_adv = dir.join("Wrapped.adv");
        let xml = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Ableton MajorVersion=\"5\" MinorVersion=\"12.0_12117\">\n",
            "\t<Device>\n",
            "\t\t<Delay Id=\"5\">\n",
            "\t\t\t<UserName Value=\"Slap\" />\n",
            "\t\t\t<On><Manual Value=\"true\" /></On>\n",
            "\t\t\t<Time><Manual Value=\"120\" /></Time>\n",
            "\t\t</Delay>\n",
            "\t</Device>\n",
            "</Ableton>\n",
        );
        let tree = crate::als::read::read_xml_tree(xml).expect("parse xml");
        write_als_gz(&tree, &wrapped_adv).expect("write gz");

        let loaded = load_device_preset(&wrapped_adv).expect("load wrapped .adv");
        assert_eq!(loaded.device_type, DeviceType::Delay);
        assert_eq!(loaded.name, "Slap");
        assert!((loaded.get_param("time_ms") - 120.0).abs() < 1e-3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rack_kind_inference_explicit_kind_and_sample_ref_roundtrip() {
        let dir = temp_dir("rack_kind_and_refs");

        // Instrument rack inferred from Operator + Reverb.
        let mut op = Device::new(DeviceId(1), "Lead", DeviceType::Operator);
        op.set_param("filter_cutoff", 1600.0);
        let rev = Device::new(DeviceId(2), "Verb", DeviceType::Reverb);
        let inst_adg = dir.join("InstRack.adg");
        save_rack_preset(
            &RackPreset {
                name: "Synth Stack".to_owned(),
                devices: vec![op, rev],
                macros: [0.5; 8],
            },
            &inst_adg,
        )
        .expect("save instrument rack");
        let inst_xml = decompress_als_or_xml(&inst_adg).expect("xml inst");
        assert!(inst_xml.contains("<InstrumentGroupDevice"));
        assert!(inst_xml.contains("<InstrumentBranchPreset"));
        let (loaded_inst, inst_kind) = load_rack_preset_with_kind(&inst_adg).expect("load inst");
        assert_eq!(inst_kind, RackKind::Instrument);
        assert_eq!(loaded_inst.devices.len(), 2);

        // MIDI Effect rack inferred from Arpeggiator.
        let arp = Device::new(
            DeviceId(3),
            "Arp",
            DeviceType::Custom("Arpeggiator".to_owned()),
        );
        let midi_adg = dir.join("MidiRack.adg");
        save_rack_preset(
            &RackPreset {
                name: "Arp Chain".to_owned(),
                devices: vec![arp],
                macros: [0.0; 8],
            },
            &midi_adg,
        )
        .expect("save midi rack");
        let midi_xml = decompress_als_or_xml(&midi_adg).expect("xml midi");
        assert!(midi_xml.contains("<MidiEffectGroupDevice"));
        assert!(midi_xml.contains("<MidiEffectBranchPreset"));
        let (_, midi_kind) = load_rack_preset_with_kind(&midi_adg).expect("load midi");
        assert_eq!(midi_kind, RackKind::MidiEffect);

        // Explicit RackKind override for empty rack.
        let explicit_adg = dir.join("EmptyInst.adg");
        save_rack_preset_with_kind(
            &RackPreset {
                name: "Empty Inst".to_owned(),
                devices: Vec::new(),
                macros: [0.0; 8],
            },
            &explicit_adg,
            RackKind::Instrument,
        )
        .expect("save explicit inst");
        let (_, loaded_kind) = load_rack_preset_with_kind(&explicit_adg).expect("load explicit");
        assert_eq!(loaded_kind, RackKind::Instrument);

        // Device & Rack preset round-trip preserves <SampleRef><FileRef> for Simpler and Custom devices.
        let mut simpler = Device::new(DeviceId(20), "OneShot", DeviceType::Simpler);
        simpler.sample_refs.push(DeviceSampleRef {
            path: "C:/Proj/Samples/Imported/snare.wav".to_owned(),
            relative_path: Some("Samples/Imported/snare.wav".to_owned()),
        });
        let adv_out = dir.join("OneShot.adv");
        save_device_preset(&simpler, &adv_out).expect("save simpler .adv");
        let reloaded_simpler = load_device_preset(&adv_out).expect("load simpler .adv");
        assert_eq!(reloaded_simpler.sample_refs, simpler.sample_refs);

        let mut custom = Device::new(
            DeviceId(21),
            "CustomSampler",
            DeviceType::Custom("Sampler".to_owned()),
        );
        custom.sample_refs.push(DeviceSampleRef {
            path: "C:/Proj/Samples/Imported/pad.wav".to_owned(),
            relative_path: Some("Samples/Imported/pad.wav".to_owned()),
        });
        let rack_with_refs = dir.join("SamplerRack.adg");
        save_rack_preset(
            &RackPreset {
                name: "Sampler Rack".to_owned(),
                devices: vec![reloaded_simpler, custom],
                macros: [0.0; 8],
            },
            &rack_with_refs,
        )
        .expect("save rack with sample refs");
        let reloaded_rack = load_rack_preset(&rack_with_refs).expect("load rack with sample refs");
        assert_eq!(reloaded_rack.devices.len(), 2);
        assert_eq!(reloaded_rack.devices[0].sample_refs.len(), 1);
        assert_eq!(
            reloaded_rack.devices[0].sample_refs[0]
                .relative_path
                .as_deref(),
            Some("Samples/Imported/snare.wav")
        );
        assert_eq!(reloaded_rack.devices[1].sample_refs.len(), 1);
        assert_eq!(
            reloaded_rack.devices[1].sample_refs[0].path,
            "C:/Proj/Samples/Imported/pad.wav"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn arbitrary_bytes_never_panic_in_load_device_or_rack_preset(
            bytes in proptest::collection::vec(any::<u8>(), 0..1024)
        ) {
            let pid = std::process::id();
            let adv = std::env::temp_dir().join(format!("livecraft_preset_prop_{pid}.adv"));
            let adg = std::env::temp_dir().join(format!("livecraft_preset_prop_{pid}.adg"));
            let _ = fs::write(&adv, &bytes);
            let _ = fs::write(&adg, &bytes);
            let _ = load_device_preset(&adv);
            let _ = load_rack_preset(&adg);
            let _ = fs::remove_file(&adv);
            let _ = fs::remove_file(&adg);
        }
    }
}
