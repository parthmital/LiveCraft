//! Mappers for MIDI `KeyTracks`/`MidiNoteEvent`s, `ClipLaunch`, and `FollowAction` settings.

use std::collections::BTreeMap;

use livecraft_model::model::{
    ClipLaunch, FollowAction, FollowActions, LaunchMode, MidiClipData, MidiNote, Quantization,
};

use super::super::tree::{XmlAttr, XmlNodeId, XmlTree};
use super::scenes::{global_index_to_quantization, quantization_to_global_index};
use super::{
    AlsIdAllocator, append_value_child, bool_str, f64_eq, format_f32, format_f64, parse_bool_str,
    parse_child_bool, parse_child_f32, parse_child_f64, parse_child_u64, set_child_value,
};

pub(crate) fn read_clip_launch(tree: &XmlTree, node: XmlNodeId) -> ClipLaunch {
    let mut launch = ClipLaunch::default();
    if let Some(m) = parse_child_u64(tree, node, "LaunchMode") {
        launch.mode = match m {
            1 => LaunchMode::Gate,
            2 => LaunchMode::Toggle,
            3 => LaunchMode::Repeat,
            _ => LaunchMode::Trigger,
        };
    }
    if let Some(q) = parse_child_u64(tree, node, "LaunchQuantisation") {
        launch.quantization = match q {
            0 => Quantization::Global,
            idx => global_index_to_quantization(idx as usize - 1).unwrap_or(Quantization::Global),
        };
    }
    if let Some(leg) = parse_child_bool(tree, node, "Legato") {
        launch.legato = leg;
    }
    if let Some(vel) = parse_child_f32(tree, node, "VelocityAmount") {
        launch.velocity_amount = vel.clamp(0.0, 1.0);
    }
    launch
}

pub(crate) fn patch_clip_launch(tree: &mut XmlTree, node: XmlNodeId, launch: &ClipLaunch) {
    let mode_code = match launch.mode {
        LaunchMode::Trigger => "0",
        LaunchMode::Gate => "1",
        LaunchMode::Toggle => "2",
        LaunchMode::Repeat => "3",
    };
    set_child_value(tree, node, "LaunchMode", mode_code);
    let q_code = match launch.quantization {
        Quantization::Global => 0,
        other => quantization_to_global_index(other).map_or(0, |i| i + 1),
    };
    set_child_value(tree, node, "LaunchQuantisation", q_code.to_string());
    set_child_value(tree, node, "Legato", bool_str(launch.legato));
    set_child_value(
        tree,
        node,
        "VelocityAmount",
        format_f32(launch.velocity_amount),
    );
}

fn decode_follow_action(code: u64, jump_index: u32) -> FollowAction {
    match code {
        0 => FollowAction::NoAction,
        1 => FollowAction::Stop,
        2 => FollowAction::PlayAgain,
        3 => FollowAction::Previous,
        4 => FollowAction::Next,
        5 => FollowAction::First,
        6 => FollowAction::Last,
        7 => FollowAction::Any,
        8 => FollowAction::Other,
        9 => FollowAction::Jump(jump_index),
        _ => FollowAction::NoAction,
    }
}

fn encode_follow_action(action: FollowAction) -> (u64, Option<u32>) {
    match action {
        FollowAction::NoAction => (0, None),
        FollowAction::Stop => (1, None),
        FollowAction::PlayAgain => (2, None),
        FollowAction::Previous => (3, None),
        FollowAction::Next => (4, None),
        FollowAction::First => (5, None),
        FollowAction::Last => (6, None),
        FollowAction::Any => (7, None),
        FollowAction::Other => (8, None),
        FollowAction::Jump(row) => (9, Some(row)),
    }
}

pub(crate) fn read_clip_follow_actions(tree: &XmlTree, node: XmlNodeId) -> FollowActions {
    let mut fa = FollowActions::default();
    let Some(fa_node) = tree.child(node, "FollowAction") else {
        return fa;
    };
    if let Some(v) = parse_child_bool(tree, fa_node, "FollowActionEnabled") {
        fa.enabled = v;
    }
    let jump_a = parse_child_u64(tree, fa_node, "JumpIndexA").unwrap_or(0) as u32;
    let jump_b = parse_child_u64(tree, fa_node, "JumpIndexB").unwrap_or(0) as u32;
    if let Some(code) = parse_child_u64(tree, fa_node, "FollowActionA") {
        fa.action_a = decode_follow_action(code, jump_a);
    }
    if let Some(code) = parse_child_u64(tree, fa_node, "FollowActionB") {
        fa.action_b = decode_follow_action(code, jump_b);
    }
    if let Some(ca) = parse_child_f32(tree, fa_node, "FollowChanceA") {
        fa.chance_a = (ca / 100.0).clamp(0.0, 1.0);
    }
    if let Some(cb) = parse_child_f32(tree, fa_node, "FollowChanceB") {
        fa.chance_b = (cb / 100.0).clamp(0.0, 1.0);
    }
    if let Some(linked) = parse_child_bool(tree, fa_node, "IsLinked") {
        fa.linked = linked;
    }
    if let Some(mult) = parse_child_u64(tree, fa_node, "LoopIterations") {
        fa.multiplier = (mult as u32).max(1);
    }
    if let Some(time) = parse_child_f64(tree, fa_node, "FollowTime") {
        fa.time_beats = time;
    }
    fa
}

pub(crate) fn patch_clip_follow_actions(
    tree: &mut XmlTree,
    node: XmlNodeId,
    follow: &FollowActions,
) {
    let Some(fa_node) = tree.ensure_child(node, "FollowAction") else {
        return;
    };
    let (code_a, jump_a) = encode_follow_action(follow.action_a);
    let (code_b, jump_b) = encode_follow_action(follow.action_b);
    set_child_value(tree, fa_node, "FollowTime", format_f64(follow.time_beats));
    set_child_value(tree, fa_node, "IsLinked", bool_str(follow.linked));
    set_child_value(
        tree,
        fa_node,
        "LoopIterations",
        follow.multiplier.to_string(),
    );
    set_child_value(tree, fa_node, "FollowActionA", code_a.to_string());
    set_child_value(tree, fa_node, "FollowActionB", code_b.to_string());
    set_child_value(
        tree,
        fa_node,
        "FollowChanceA",
        format_f32(follow.chance_a * 100.0),
    );
    set_child_value(
        tree,
        fa_node,
        "FollowChanceB",
        format_f32(follow.chance_b * 100.0),
    );
    set_child_value(tree, fa_node, "JumpIndexA", jump_a.unwrap_or(0).to_string());
    set_child_value(tree, fa_node, "JumpIndexB", jump_b.unwrap_or(0).to_string());
    set_child_value(
        tree,
        fa_node,
        "FollowActionEnabled",
        bool_str(follow.enabled),
    );
}

pub(crate) fn read_midi_notes(tree: &XmlTree, clip_node: XmlNodeId) -> Vec<MidiNote> {
    let mut notes = Vec::new();
    let key_tracks_node = tree
        .child(clip_node, "Notes")
        .and_then(|n| tree.child(n, "KeyTracks"))
        .or_else(|| tree.child(clip_node, "KeyTracks"));

    let Some(kts) = key_tracks_node else {
        return notes;
    };

    for kt in tree.children_named(kts, "KeyTrack") {
        let pitch = parse_child_u64(tree, kt, "MidiKey")
            .map(|v| v.min(127) as u8)
            .unwrap_or(60);
        if let Some(notes_node) = tree.child(kt, "Notes") {
            for ev in tree.children_named(notes_node, "MidiNoteEvent") {
                let start_beat = tree
                    .attr(ev, "Time")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let duration_beats = tree
                    .attr(ev, "Duration")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);
                let velocity = tree
                    .attr(ev, "Velocity")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(100.0);
                let release_velocity = tree
                    .attr(ev, "OffVelocity")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(64.0);
                let probability = tree
                    .attr(ev, "Probability")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(1.0);
                let velocity_deviation = tree
                    .attr(ev, "VelocityDeviation")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let mute = tree
                    .attr(ev, "IsEnabled")
                    .and_then(parse_bool_str)
                    .is_some_and(|enabled| !enabled);
                notes.push(MidiNote {
                    pitch,
                    start_beat,
                    duration_beats,
                    velocity,
                    release_velocity,
                    probability,
                    velocity_deviation,
                    mute,
                });
            }
        }
    }
    notes
}

pub(crate) fn patch_midi_notes(
    tree: &mut XmlTree,
    clip_node: XmlNodeId,
    prev: Option<&MidiClipData>,
    cur: &MidiClipData,
    ids: &mut AlsIdAllocator,
) {
    let notes_unchanged = prev.is_some_and(|p| {
        p.notes.len() == cur.notes.len()
            && p.notes.iter().zip(&cur.notes).all(|(x, y)| {
                x.pitch == y.pitch
                    && f64_eq(x.start_beat, y.start_beat)
                    && f64_eq(x.duration_beats, y.duration_beats)
                    && x.velocity == y.velocity
                    && x.release_velocity == y.release_velocity
                    && x.probability == y.probability
                    && x.velocity_deviation == y.velocity_deviation
                    && x.mute == y.mute
            })
    });
    if notes_unchanged {
        return;
    }

    let notes_container = tree.ensure_child(clip_node, "Notes");
    let Some(kts) = notes_container.and_then(|nc| tree.ensure_child(nc, "KeyTracks")) else {
        return;
    };
    tree.clear_children(kts);

    let mut by_pitch: BTreeMap<u8, Vec<&MidiNote>> = BTreeMap::new();
    for note in &cur.notes {
        by_pitch.entry(note.pitch.min(127)).or_default().push(note);
    }

    let mut next_note_id = 1u64;
    for (pitch, list) in by_pitch {
        let kt = tree.alloc_element_with(
            "KeyTrack",
            vec![XmlAttr::new("Id", ids.alloc().to_string())],
            Some(false),
        );
        let notes_el = tree.alloc_element("Notes");
        tree.append_child(kt, notes_el);
        for note in list {
            let ev = tree.alloc_element_with(
                "MidiNoteEvent",
                vec![
                    XmlAttr::new("Time", format_f64(note.start_beat)),
                    XmlAttr::new("Duration", format_f64(note.duration_beats)),
                    XmlAttr::new("Velocity", format_f32(note.velocity)),
                    XmlAttr::new("VelocityDeviation", format_f32(note.velocity_deviation)),
                    XmlAttr::new("OffVelocity", format_f32(note.release_velocity)),
                    XmlAttr::new("Probability", format_f32(note.probability)),
                    XmlAttr::new("IsEnabled", bool_str(!note.mute)),
                    XmlAttr::new("NoteId", next_note_id.to_string()),
                ],
                Some(true),
            );
            next_note_id += 1;
            tree.append_child(notes_el, ev);
        }
        append_value_child(tree, kt, "MidiKey", pitch.to_string());
        tree.append_child(kts, kt);
    }

    if let Some(nc) = notes_container
        && let Some(gen_node) = tree.ensure_child(nc, "NoteIdGenerator")
    {
        set_child_value(tree, gen_node, "NextId", next_note_id.to_string());
    }
}
