//! Mappers for `<Scenes>`, `<ScaleInformation>`, `<InKey>`, `<TuningSystems>`,
//! and Live's global quantization / scale enum lookup tables.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use livecraft_model::color::{ABLETON_PALETTE, parse_hex_color};
use livecraft_model::ids::SceneId;
use livecraft_model::model::{
    Metronome, MetronomeRhythm, MetronomeSound, Quantization, ScaleConfig, ScaleType, Scene,
    Transport, TuningSystem,
};

use super::super::AlsError;
use super::super::tree::{XmlAttr, XmlNodeId, XmlTree};
use super::transport::{decode_time_signature, encode_time_signature};
use super::{
    AlsIdAllocator, EntityMap, bool_str, child_value_str, f32_eq, format_f32, format_f64,
    instantiate_template_cached, parse_bool_str, parse_child_bool, parse_child_f32,
    parse_child_f64, parse_child_i64, parse_child_u64, scenes_equal, set_child_value,
    sync_ordered_children,
};

const SCENE_TEMPLATE_XML: &str = include_str!("../../../templates/scene.xml");
static SCENE_TEMPLATE_CACHE: OnceLock<Option<XmlTree>> = OnceLock::new();
const DEFAULT_SCENE_COLOR: &str = "#464646";

/// Converts an XML `<Color Value="..." />` string into a hex color string.
#[must_use]
pub(crate) fn color_from_xml(val: &str, default_hex: &str) -> String {
    let trimmed = val.trim();
    if trimmed.starts_with('#') {
        return trimmed.to_owned();
    }
    if let Ok(idx) = trimmed.parse::<i64>()
        && idx >= 0
        && (idx as usize) < ABLETON_PALETTE.len()
    {
        return ABLETON_PALETTE[idx as usize].to_owned();
    }
    default_hex.to_owned()
}

/// Converts a hex color string into Live's `0..70` palette index (or `"-1"` when matching default).
#[must_use]
pub(crate) fn color_to_xml(hex: &str, default_hex: &str) -> String {
    if let Some(idx) = ABLETON_PALETTE
        .iter()
        .position(|p| p.eq_ignore_ascii_case(hex))
    {
        return idx.to_string();
    }
    if hex.eq_ignore_ascii_case(default_hex) {
        return "-1".to_owned();
    }
    let [r0, g0, b0, _] = parse_hex_color(hex);
    let mut best_idx = 0usize;
    let mut best_dist = u32::MAX;
    for (idx, &pal) in ABLETON_PALETTE.iter().enumerate() {
        let [r1, g1, b1, _] = parse_hex_color(pal);
        let dr = i32::from(r0) - i32::from(r1);
        let dg = i32::from(g0) - i32::from(g1);
        let db = i32::from(b0) - i32::from(b1);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
        }
    }
    best_idx.to_string()
}

const GLOBAL_QUANT_TABLE: [Quantization; 14] = [
    Quantization::None,
    Quantization::Bars(8),
    Quantization::Bars(4),
    Quantization::Bars(2),
    Quantization::Bars(1),
    Quantization::Note {
        num: 1,
        den: 2,
        triplet: false,
    },
    Quantization::Note {
        num: 1,
        den: 2,
        triplet: true,
    },
    Quantization::Note {
        num: 1,
        den: 4,
        triplet: false,
    },
    Quantization::Note {
        num: 1,
        den: 4,
        triplet: true,
    },
    Quantization::Note {
        num: 1,
        den: 8,
        triplet: false,
    },
    Quantization::Note {
        num: 1,
        den: 8,
        triplet: true,
    },
    Quantization::Note {
        num: 1,
        den: 16,
        triplet: false,
    },
    Quantization::Note {
        num: 1,
        den: 16,
        triplet: true,
    },
    Quantization::Note {
        num: 1,
        den: 32,
        triplet: false,
    },
];

const SCALE_CODES: [(ScaleType, usize); 12] = [
    (ScaleType::Major, 0),
    (ScaleType::Minor, 1),
    (ScaleType::Dorian, 2),
    (ScaleType::Mixolydian, 3),
    (ScaleType::Lydian, 4),
    (ScaleType::Phrygian, 5),
    (ScaleType::Locrian, 6),
    (ScaleType::Blues, 10),
    (ScaleType::MinorPentatonic, 11),
    (ScaleType::MajorPentatonic, 12),
    (ScaleType::HarmonicMinor, 13),
    (ScaleType::MelodicMinor, 14),
];

/// Maps `GlobalQuantisation` integer index (`0..=13`) into `Quantization`.
#[must_use]
pub fn global_index_to_quantization(idx: usize) -> Option<Quantization> {
    GLOBAL_QUANT_TABLE.get(idx).copied()
}

/// Maps `Quantization` to Live's `GlobalQuantisation` integer index (`0..=13`).
#[must_use]
pub fn quantization_to_global_index(q: Quantization) -> Option<usize> {
    GLOBAL_QUANT_TABLE.iter().position(|&item| item == q)
}

#[must_use]
pub(crate) fn scale_type_to_live_code(st: ScaleType) -> usize {
    SCALE_CODES
        .iter()
        .find(|&&(t, _)| t == st)
        .map_or(0, |&(_, code)| code)
}

#[must_use]
pub(crate) fn live_code_to_scale_type(code: usize) -> Option<ScaleType> {
    SCALE_CODES
        .iter()
        .find(|&&(_, c)| c == code)
        .map(|&(t, _)| t)
}

pub(crate) fn read_scale_config(tree: &XmlTree, live_set: XmlNodeId) -> ScaleConfig {
    let mut scale = ScaleConfig::default();
    let in_key = parse_child_bool(tree, live_set, "InKey");
    let Some(si) = tree.child(live_set, "ScaleInformation") else {
        if let Some(active) = in_key {
            scale.is_active = active;
        }
        return scale;
    };

    if let Some(root_val) =
        parse_child_u64(tree, si, "Root").or_else(|| parse_child_u64(tree, si, "RootNote"))
    {
        scale.root_note = (root_val % 12) as u8;
    }

    let mut parsed_string_scale = false;
    if let Some(name_str) =
        child_value_str(tree, si, "Name").or_else(|| child_value_str(tree, si, "ScaleType"))
    {
        if let Ok(code) = name_str.parse::<usize>() {
            if let Some(st) = live_code_to_scale_type(code) {
                scale.scale_type = st;
            }
        } else {
            for st in ScaleType::ALL {
                if st.name().eq_ignore_ascii_case(name_str) {
                    scale.scale_type = *st;
                    parsed_string_scale = true;
                    break;
                }
            }
        }
    }

    scale.is_active = in_key
        .or_else(|| parse_child_bool(tree, si, "IsActive"))
        .unwrap_or(parsed_string_scale);
    scale
}

pub(crate) fn patch_scale_config(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    base: &ScaleConfig,
    cur: &ScaleConfig,
) {
    if cur == base {
        return;
    }
    if cur.is_active != base.is_active || tree.child(live_set, "InKey").is_some() {
        set_child_value(tree, live_set, "InKey", bool_str(cur.is_active));
    }
    if let Some(si) = tree.ensure_child(live_set, "ScaleInformation") {
        if cur.root_note != base.root_note || tree.child(si, "Root").is_none() {
            if tree.child(si, "RootNote").is_some() && tree.child(si, "Root").is_none() {
                set_child_value(tree, si, "RootNote", (cur.root_note % 12).to_string());
            } else {
                set_child_value(tree, si, "Root", (cur.root_note % 12).to_string());
            }
        }
        if cur.scale_type != base.scale_type || tree.child(si, "Name").is_none() {
            set_child_value(
                tree,
                si,
                "Name",
                scale_type_to_live_code(cur.scale_type).to_string(),
            );
        }
    }
}

pub(crate) fn read_tuning(tree: &XmlTree, live_set: XmlNodeId) -> Option<TuningSystem> {
    let ts = tree.child(live_set, "TuningSystems")?;
    let lct = tree.child(ts, "LiveCraftTuning")?;
    if let Some(scala) = tree.child(lct, "ScalaData") {
        let text = tree.text_content(scala);
        if let Ok(parsed) = TuningSystem::from_scala(&text) {
            return Some(parsed);
        }
    }
    let name = child_value_str(tree, lct, "Name")
        .unwrap_or("Custom Tuning")
        .to_owned();
    let description = child_value_str(tree, lct, "Description")
        .unwrap_or(&name)
        .to_owned();
    let base_freq_hz = parse_child_f64(tree, lct, "BaseFreqHz").unwrap_or(440.0);
    let base_midi_note = parse_child_u64(tree, lct, "BaseMidiNote").unwrap_or(69) as u8;
    let mut degrees_cents = Vec::new();
    if let Some(dc) = tree.child(lct, "DegreesCents") {
        for deg in tree.children_named(dc, "Degree") {
            if let Some(v) = tree.value_attr(deg).and_then(|s| s.parse::<f64>().ok()) {
                degrees_cents.push(v);
            }
        }
    }
    Some(TuningSystem {
        name,
        description,
        degrees_cents,
        base_freq_hz,
        base_midi_note,
    })
}

pub(crate) fn patch_tuning(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    base: &Option<TuningSystem>,
    cur: &Option<TuningSystem>,
) {
    if cur == base {
        return;
    }
    let Some(ts) = tree.ensure_child(live_set, "TuningSystems") else {
        return;
    };
    if let Some(old) = tree.child(ts, "LiveCraftTuning") {
        tree.remove_child(ts, old);
    }
    if let Some(tuning) = cur
        && let Some(lct) = tree.ensure_child(ts, "LiveCraftTuning")
    {
        set_child_value(tree, lct, "Name", &tuning.name);
        set_child_value(tree, lct, "Description", &tuning.description);
        set_child_value(tree, lct, "BaseFreqHz", format_f64(tuning.base_freq_hz));
        set_child_value(tree, lct, "BaseMidiNote", tuning.base_midi_note.to_string());
        if let Some(dc) = tree.ensure_child(lct, "DegreesCents") {
            for &cents in &tuning.degrees_cents {
                let deg = tree.alloc_element_with(
                    "Degree",
                    vec![XmlAttr::new("Value", format_f64(cents))],
                    Some(true),
                );
                tree.append_child(dc, deg);
            }
        }
    }
}

pub(crate) fn read_scenes(
    tree: &XmlTree,
    live_set: XmlNodeId,
    entities: &mut EntityMap,
    ids: &mut AlsIdAllocator,
) -> Vec<Scene> {
    let Some(scenes_node) = tree.child(live_set, "Scenes") else {
        return Vec::new();
    };
    let mut used = HashSet::new();
    let mut out = Vec::new();
    for (idx, sc_node) in tree
        .children_named(scenes_node, "Scene")
        .into_iter()
        .enumerate()
    {
        let parsed_id = tree.attr(sc_node, "Id").and_then(|s| s.parse::<u64>().ok());
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
        let scene_id = SceneId(id);
        entities.scenes.insert(scene_id, sc_node);

        let name = child_value_str(tree, sc_node, "Name")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}", idx + 1));
        let color = child_value_str(tree, sc_node, "Color")
            .map(|v| color_from_xml(v, DEFAULT_SCENE_COLOR))
            .unwrap_or_else(|| DEFAULT_SCENE_COLOR.to_owned());
        let tempo = if parse_child_bool(tree, sc_node, "IsTempoEnabled").unwrap_or(false) {
            parse_child_f32(tree, sc_node, "Tempo")
        } else {
            None
        };
        let time_signature =
            if parse_child_bool(tree, sc_node, "IsTimeSignatureEnabled").unwrap_or(false) {
                parse_child_i64(tree, sc_node, "TimeSignatureId").and_then(decode_time_signature)
            } else {
                None
            };

        out.push(Scene {
            id: scene_id,
            name,
            tempo,
            time_signature,
            color,
        });
    }
    out
}

pub(crate) fn patch_scenes(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    base: &[Scene],
    cur: &[Scene],
    entities: &mut EntityMap,
) -> Result<(), AlsError> {
    if base.len() == cur.len() && base.iter().zip(cur).all(|(a, b)| scenes_equal(a, b)) {
        return Ok(());
    }
    let Some(scenes_node) = tree.ensure_child(live_set, "Scenes") else {
        return Ok(());
    };
    let base_by_id: HashMap<SceneId, &Scene> = base.iter().map(|s| (s.id, s)).collect();
    let mut new_map = HashMap::new();
    let mut desired_nodes = Vec::with_capacity(cur.len());

    for scene in cur {
        let node_id = if let Some(&existing) = entities.scenes.get(&scene.id) {
            if let Some(prev) = base_by_id.get(&scene.id) {
                if !scenes_equal(prev, scene) {
                    patch_scene_node(tree, existing, Some(prev), scene);
                }
            } else {
                patch_scene_node(tree, existing, None, scene);
            }
            existing
        } else {
            let fresh =
                instantiate_template_cached(tree, &SCENE_TEMPLATE_CACHE, SCENE_TEMPLATE_XML)?;
            tree.set_attr(fresh, "Id", scene.id.0.to_string());
            patch_scene_node(tree, fresh, None, scene);
            fresh
        };
        new_map.insert(scene.id, node_id);
        desired_nodes.push(node_id);
    }

    sync_ordered_children(
        tree,
        scenes_node,
        |n| n.element_name() == Some("Scene"),
        desired_nodes,
    );
    entities.scenes = new_map;
    Ok(())
}

fn patch_scene_node(tree: &mut XmlTree, sc_node: XmlNodeId, prev: Option<&Scene>, cur: &Scene) {
    if prev.is_none_or(|p| p.name != cur.name) {
        set_child_value(tree, sc_node, "Name", &cur.name);
    }
    if prev.is_none_or(|p| p.color != cur.color) {
        set_child_value(
            tree,
            sc_node,
            "Color",
            color_to_xml(&cur.color, DEFAULT_SCENE_COLOR),
        );
    }
    let tempo_changed = prev.is_none_or(|p| match (p.tempo, cur.tempo) {
        (None, None) => false,
        (Some(a), Some(b)) => !f32_eq(a, b),
        _ => true,
    });
    if tempo_changed {
        match cur.tempo {
            Some(t) => {
                set_child_value(tree, sc_node, "Tempo", format_f32(t));
                set_child_value(tree, sc_node, "IsTempoEnabled", "true");
            }
            None => {
                set_child_value(tree, sc_node, "IsTempoEnabled", "false");
            }
        }
    }
    if prev.is_none_or(|p| p.time_signature != cur.time_signature) {
        match cur.time_signature {
            Some((num, den)) => {
                set_child_value(
                    tree,
                    sc_node,
                    "TimeSignatureId",
                    encode_time_signature(num, den).to_string(),
                );
                set_child_value(tree, sc_node, "IsTimeSignatureEnabled", "true");
            }
            None => {
                set_child_value(tree, sc_node, "IsTimeSignatureEnabled", "false");
            }
        }
    }
}

pub(super) fn read_livecraft_transport(
    tree: &XmlTree,
    live_set: XmlNodeId,
    transport: &mut Transport,
) {
    let Some(lc) = tree.child(live_set, "LiveCraftTransport") else {
        return;
    };
    if let Some(v) = parse_child_bool(tree, lc, "MetronomeEnabled") {
        transport.metronome_enabled = v;
    }
    if let Some(v) = parse_child_bool(tree, lc, "FollowActionsEnabled") {
        transport.follow_actions_enabled = v;
    }
    if let Some(v) = parse_child_u64(tree, lc, "FollowSeed") {
        transport.follow_seed = v;
    }
    if let Some(m) = tree.child(lc, "Metronome") {
        if let Some(v) = parse_child_u64(tree, m, "CountInBars") {
            transport.metronome.count_in_bars = v as u8;
        }
        if let Some(v) = parse_child_bool(tree, m, "OnlyWhileRecording") {
            transport.metronome.only_while_recording = v;
        }
        if let Some(v) = parse_child_f32(tree, m, "VolumeDb") {
            transport.metronome.volume_db = v;
        }
        if let Some(s) = child_value_str(tree, m, "Sound") {
            transport.metronome.sound = match s {
                "Click" => MetronomeSound::Click,
                "Wood" => MetronomeSound::Wood,
                _ => MetronomeSound::Classic,
            };
        }
        if let Some(r) = tree.child(m, "Rhythm") {
            let kind = tree.attr(r, "Kind").unwrap_or("Auto");
            if kind == "Division" {
                let den = tree
                    .attr(r, "Denominator")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(4);
                let triplet = tree
                    .attr(r, "Triplet")
                    .and_then(parse_bool_str)
                    .unwrap_or(false);
                transport.metronome.rhythm = MetronomeRhythm::Division { den, triplet };
            } else {
                transport.metronome.rhythm = MetronomeRhythm::Auto;
            }
        }
    }
}

pub(super) fn patch_livecraft_transport(
    tree: &mut XmlTree,
    live_set: XmlNodeId,
    base: &Transport,
    cur: &Transport,
) {
    let needs_meta = cur.metronome_enabled
        || !cur.follow_actions_enabled
        || cur.follow_seed != 0
        || cur.metronome != Metronome::default();
    let base_had_meta = base.metronome_enabled
        || !base.follow_actions_enabled
        || base.follow_seed != 0
        || base.metronome != Metronome::default();
    if !(needs_meta != base_had_meta
        || (needs_meta
            && (cur.metronome_enabled != base.metronome_enabled
                || cur.follow_actions_enabled != base.follow_actions_enabled
                || cur.follow_seed != base.follow_seed
                || cur.metronome != base.metronome)))
    {
        return;
    }
    if !needs_meta {
        if let Some(existing) = tree.child(live_set, "LiveCraftTransport") {
            tree.remove_child(live_set, existing);
        }
        return;
    }
    let Some(lc) = tree.ensure_child(live_set, "LiveCraftTransport") else {
        return;
    };
    set_child_value(
        tree,
        lc,
        "MetronomeEnabled",
        bool_str(cur.metronome_enabled),
    );
    set_child_value(
        tree,
        lc,
        "FollowActionsEnabled",
        bool_str(cur.follow_actions_enabled),
    );
    set_child_value(tree, lc, "FollowSeed", cur.follow_seed.to_string());
    if let Some(m) = tree.ensure_child(lc, "Metronome") {
        set_child_value(
            tree,
            m,
            "CountInBars",
            cur.metronome.count_in_bars.to_string(),
        );
        set_child_value(
            tree,
            m,
            "OnlyWhileRecording",
            bool_str(cur.metronome.only_while_recording),
        );
        set_child_value(tree, m, "VolumeDb", format_f32(cur.metronome.volume_db));
        let sound = match cur.metronome.sound {
            MetronomeSound::Classic => "Classic",
            MetronomeSound::Click => "Click",
            MetronomeSound::Wood => "Wood",
        };
        set_child_value(tree, m, "Sound", sound);
        if let Some(r) = tree.ensure_child(m, "Rhythm") {
            match cur.metronome.rhythm {
                MetronomeRhythm::Auto => {
                    tree.set_attr(r, "Kind", "Auto");
                    tree.remove_attr(r, "Denominator");
                    tree.remove_attr(r, "Triplet");
                }
                MetronomeRhythm::Division { den, triplet } => {
                    tree.set_attr(r, "Kind", "Division");
                    tree.set_attr(r, "Denominator", den.to_string());
                    tree.set_attr(r, "Triplet", bool_str(triplet));
                }
            }
        }
    }
}
