//! Musical diff summary (`LiveSetDiff`) between two Live Sets (B10).

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use livecraft_model::ids::{ClipId, DeviceId, LaneId, SceneId, TrackId};
use livecraft_model::model::{
    AutomationLane, Clip, ClipPayload, Device, MidiNote, Project, Quantization, ScaleConfig, Scene,
    Track, TrackType,
};
use livecraft_model::scale::note_name;

use super::AlsError;
use super::map::AlsDocument;

/// Location of a clip inside a track.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipLocation {
    SessionSlot(usize),
    ArrangementBeat(f64),
}

/// Optional before/after time signature change pair on a scene.
pub type SceneTimeSignatureChange = (Option<(u8, u8)>, Option<(u8, u8)>);

/// Differences found on a single [`Scene`].
#[derive(Debug, Clone, PartialEq)]
pub struct SceneDiff {
    pub id: SceneId,
    pub name: String,
    pub renamed: Option<(String, String)>,
    pub tempo_changed: Option<(Option<f32>, Option<f32>)>,
    pub time_signature_changed: Option<SceneTimeSignatureChange>,
    pub color_changed: Option<(String, String)>,
}

/// Differences found on a single [`Device`] parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDiff {
    pub key: String,
    pub before: Option<f32>,
    pub after: Option<f32>,
}

/// Differences found on a single [`Device`].
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceDiff {
    pub id: DeviceId,
    pub name: String,
    pub renamed: Option<(String, String)>,
    pub enabled_changed: Option<(bool, bool)>,
    pub params_changed: Vec<ParamDiff>,
}

/// Differences found on a single [`Clip`].
#[derive(Debug, Clone, PartialEq)]
pub struct ClipDiff {
    pub id: ClipId,
    pub name: String,
    pub location: ClipLocation,
    pub renamed: Option<(String, String)>,
    pub length_changed: Option<(f64, f64)>,
    pub loop_changed: Option<(bool, bool)>,
    pub notes_added: Vec<(u8, f64)>,
    pub notes_removed: Vec<(u8, f64)>,
    pub notes_modified: usize,
    pub audio_path_changed: Option<(String, String)>,
    pub audio_warped_changed: Option<(bool, bool)>,
    pub launch_changed: bool,
    pub follow_changed: bool,
    pub automation_changed: bool,
}

/// Differences found on a single [`Track`].
#[derive(Debug, Clone, PartialEq)]
pub struct TrackDiff {
    pub id: TrackId,
    pub name: String,
    pub track_type: TrackType,
    pub renamed: Option<(String, String)>,
    pub volume_db_changed: Option<(f32, f32)>,
    pub pan_changed: Option<(f32, f32)>,
    pub muted_changed: Option<(bool, bool)>,
    pub soloed_changed: Option<(bool, bool)>,
    pub armed_changed: Option<(bool, bool)>,
    pub sends_changed: Vec<(usize, f32, f32)>,
    pub clips_added: Vec<(ClipId, String, ClipLocation)>,
    pub clips_removed: Vec<(ClipId, String, ClipLocation)>,
    pub clips_modified: Vec<ClipDiff>,
    pub devices_added: Vec<(DeviceId, String)>,
    pub devices_removed: Vec<(DeviceId, String)>,
    pub devices_modified: Vec<DeviceDiff>,
    pub automation_lanes_added: usize,
    pub automation_lanes_removed: usize,
    pub automation_lanes_modified: usize,
}

/// Structured musical diff between two [`Project`] Live Sets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveSetDiff {
    pub bpm_changed: Option<(f32, f32)>,
    pub tempo_map_changed: bool,
    pub time_signature_changed: Option<((u8, u8), (u8, u8))>,
    pub time_signature_map_changed: bool,
    pub loop_enabled_changed: Option<(bool, bool)>,
    pub loop_range_changed: Option<((f64, f64), (f64, f64))>,
    pub global_quantization_changed: Option<(Quantization, Quantization)>,
    pub scale_changed: Option<(ScaleConfig, ScaleConfig)>,
    pub tuning_changed: bool,
    pub tuning_names: Option<(Option<String>, Option<String>)>,
    pub scenes_added: Vec<(SceneId, String)>,
    pub scenes_removed: Vec<(SceneId, String)>,
    pub scenes_modified: Vec<SceneDiff>,
    pub tracks_added: Vec<(TrackId, String, TrackType)>,
    pub tracks_removed: Vec<(TrackId, String, TrackType)>,
    pub tracks_modified: Vec<TrackDiff>,
}

impl LiveSetDiff {
    /// Returns `true` when no musical differences were detected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bpm_changed.is_none()
            && !self.tempo_map_changed
            && self.time_signature_changed.is_none()
            && !self.time_signature_map_changed
            && self.loop_enabled_changed.is_none()
            && self.loop_range_changed.is_none()
            && self.global_quantization_changed.is_none()
            && self.scale_changed.is_none()
            && !self.tuning_changed
            && self.scenes_added.is_empty()
            && self.scenes_removed.is_empty()
            && self.scenes_modified.is_empty()
            && self.tracks_added.is_empty()
            && self.tracks_removed.is_empty()
            && self.tracks_modified.is_empty()
    }
}

/// Loads two `.als` / `.xml` Live Sets and returns their [`LiveSetDiff`].
pub fn diff_als_files<P: AsRef<Path>>(before: P, after: P) -> Result<LiveSetDiff, AlsError> {
    let a = AlsDocument::open(before)?.into_project();
    let b = AlsDocument::open(after)?.into_project();
    Ok(diff_projects(&a, &b))
}

/// Computes a structured musical diff between `before` and `after`.
#[must_use]
pub fn diff_projects(before: &Project, after: &Project) -> LiveSetDiff {
    let mut diff = LiveSetDiff::default();

    let bpm_a = before.transport.bpm();
    let bpm_b = after.transport.bpm();
    if (bpm_a - bpm_b).abs() > 1e-4 {
        diff.bpm_changed = Some((bpm_a, bpm_b));
    }
    if before.transport.tempo != after.transport.tempo {
        diff.tempo_map_changed = true;
    }

    let sig_a = before.transport.time_signature();
    let sig_b = after.transport.time_signature();
    if sig_a != sig_b {
        diff.time_signature_changed = Some((sig_a, sig_b));
    }
    if before.transport.time_signature != after.transport.time_signature {
        diff.time_signature_map_changed = true;
    }

    if before.transport.loop_enabled != after.transport.loop_enabled {
        diff.loop_enabled_changed =
            Some((before.transport.loop_enabled, after.transport.loop_enabled));
    }
    if (before.transport.loop_start_beats - after.transport.loop_start_beats).abs() > 1e-6
        || (before.transport.loop_length_beats - after.transport.loop_length_beats).abs() > 1e-6
    {
        diff.loop_range_changed = Some((
            (
                before.transport.loop_start_beats,
                before.transport.loop_length_beats,
            ),
            (
                after.transport.loop_start_beats,
                after.transport.loop_length_beats,
            ),
        ));
    }
    if before.transport.global_quantization != after.transport.global_quantization {
        diff.global_quantization_changed = Some((
            before.transport.global_quantization,
            after.transport.global_quantization,
        ));
    }
    if before.scale != after.scale {
        diff.scale_changed = Some((before.scale, after.scale));
    }
    if before.tuning != after.tuning {
        diff.tuning_changed = true;
        diff.tuning_names = Some((
            before.tuning.as_ref().map(|t| t.name.clone()),
            after.tuning.as_ref().map(|t| t.name.clone()),
        ));
    }

    diff_scenes(&before.scenes, &after.scenes, &mut diff);
    diff_tracks(before, after, &mut diff);

    diff
}

fn diff_scenes(before: &[Scene], after: &[Scene], diff: &mut LiveSetDiff) {
    let before_map: HashMap<SceneId, &Scene> = before.iter().map(|s| (s.id, s)).collect();
    let after_map: HashMap<SceneId, &Scene> = after.iter().map(|s| (s.id, s)).collect();

    for s in before {
        if !after_map.contains_key(&s.id) {
            diff.scenes_removed.push((s.id, s.name.clone()));
        }
    }
    for s in after {
        let Some(prev) = before_map.get(&s.id) else {
            diff.scenes_added.push((s.id, s.name.clone()));
            continue;
        };
        let renamed = (prev.name != s.name).then(|| (prev.name.clone(), s.name.clone()));
        let tempo_changed = (prev.tempo != s.tempo).then_some((prev.tempo, s.tempo));
        let time_signature_changed = (prev.time_signature != s.time_signature)
            .then_some((prev.time_signature, s.time_signature));
        let color_changed = (prev.color != s.color).then(|| (prev.color.clone(), s.color.clone()));
        if renamed.is_some()
            || tempo_changed.is_some()
            || time_signature_changed.is_some()
            || color_changed.is_some()
        {
            diff.scenes_modified.push(SceneDiff {
                id: s.id,
                name: s.name.clone(),
                renamed,
                tempo_changed,
                time_signature_changed,
                color_changed,
            });
        }
    }
}

fn diff_tracks(before: &Project, after: &Project, diff: &mut LiveSetDiff) {
    let before_list: Vec<&Track> = before.all_tracks().collect();
    let after_list: Vec<&Track> = after.all_tracks().collect();
    let before_map: HashMap<TrackId, &Track> = before_list.iter().map(|t| (t.id, *t)).collect();
    let after_map: HashMap<TrackId, &Track> = after_list.iter().map(|t| (t.id, *t)).collect();

    for t in before_list {
        if t.track_type != TrackType::Main && !after_map.contains_key(&t.id) {
            diff.tracks_removed
                .push((t.id, t.name.clone(), t.track_type));
        }
    }

    for t in after_list {
        let prev_opt = before_map
            .get(&t.id)
            .copied()
            .or_else(|| (t.track_type == TrackType::Main).then_some(&before.main_track));
        let Some(prev) = prev_opt else {
            diff.tracks_added.push((t.id, t.name.clone(), t.track_type));
            continue;
        };
        if let Some(td) = diff_single_track(prev, t) {
            diff.tracks_modified.push(td);
        }
    }
}

fn diff_single_track(before: &Track, after: &Track) -> Option<TrackDiff> {
    let renamed = (before.name != after.name).then(|| (before.name.clone(), after.name.clone()));
    let volume_db_changed = ((before.volume_db - after.volume_db).abs() > 1e-4)
        .then_some((before.volume_db, after.volume_db));
    let pan_changed = ((before.pan - after.pan).abs() > 1e-4).then_some((before.pan, after.pan));
    let muted_changed =
        (before.is_muted != after.is_muted).then_some((before.is_muted, after.is_muted));
    let soloed_changed =
        (before.is_soloed != after.is_soloed).then_some((before.is_soloed, after.is_soloed));
    let armed_changed =
        (before.is_armed != after.is_armed).then_some((before.is_armed, after.is_armed));

    let mut sends_changed = Vec::new();
    let max_sends = before.sends.len().max(after.sends.len());
    for i in 0..max_sends {
        let a = before.sends.get(i).copied().unwrap_or(0.0);
        let b = after.sends.get(i).copied().unwrap_or(0.0);
        if (a - b).abs() > 1e-5 {
            sends_changed.push((i, a, b));
        }
    }

    let (clips_added, clips_removed, clips_modified) = diff_track_clips(before, after);
    let (devices_added, devices_removed, devices_modified) =
        diff_track_devices(&before.device_chain, &after.device_chain);
    let (auto_add, auto_rem, auto_mod) =
        diff_automation_lanes(&before.automation_lanes, &after.automation_lanes);

    let has_any = renamed.is_some()
        || volume_db_changed.is_some()
        || pan_changed.is_some()
        || muted_changed.is_some()
        || soloed_changed.is_some()
        || armed_changed.is_some()
        || !sends_changed.is_empty()
        || !clips_added.is_empty()
        || !clips_removed.is_empty()
        || !clips_modified.is_empty()
        || !devices_added.is_empty()
        || !devices_removed.is_empty()
        || !devices_modified.is_empty()
        || auto_add > 0
        || auto_rem > 0
        || auto_mod > 0;

    has_any.then(|| TrackDiff {
        id: after.id,
        name: after.name.clone(),
        track_type: after.track_type,
        renamed,
        volume_db_changed,
        pan_changed,
        muted_changed,
        soloed_changed,
        armed_changed,
        sends_changed,
        clips_added,
        clips_removed,
        clips_modified,
        devices_added,
        devices_removed,
        devices_modified,
        automation_lanes_added: auto_add,
        automation_lanes_removed: auto_rem,
        automation_lanes_modified: auto_mod,
    })
}

fn collect_track_clips(track: &Track) -> HashMap<ClipId, (&Clip, ClipLocation)> {
    let mut map = HashMap::new();
    for (idx, slot) in track.clip_slots.iter().enumerate() {
        if let Some(ref c) = slot.clip {
            map.insert(c.id, (c, ClipLocation::SessionSlot(idx)));
        }
    }
    for arr in &track.arrangement_clips {
        map.insert(
            arr.clip.id,
            (&arr.clip, ClipLocation::ArrangementBeat(arr.start_beat)),
        );
    }
    map
}

type ClipDiffParts = (
    Vec<(ClipId, String, ClipLocation)>,
    Vec<(ClipId, String, ClipLocation)>,
    Vec<ClipDiff>,
);

fn diff_track_clips(before: &Track, after: &Track) -> ClipDiffParts {
    let b_clips = collect_track_clips(before);
    let a_clips = collect_track_clips(after);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (id, (c, loc)) in &b_clips {
        if !a_clips.contains_key(id) {
            removed.push((*id, c.name.clone(), loc.clone()));
        }
    }
    for (id, (c, loc)) in &a_clips {
        let Some((prev, _)) = b_clips.get(id) else {
            added.push((*id, c.name.clone(), loc.clone()));
            continue;
        };
        if let Some(cd) = diff_single_clip(prev, c, loc.clone()) {
            modified.push(cd);
        }
    }
    added.sort_by_key(|x| x.0);
    removed.sort_by_key(|x| x.0);
    modified.sort_by_key(|x| x.id);
    (added, removed, modified)
}

fn diff_single_clip(before: &Clip, after: &Clip, location: ClipLocation) -> Option<ClipDiff> {
    let renamed = (before.name != after.name).then(|| (before.name.clone(), after.name.clone()));
    let length_changed = ((before.length_beats - after.length_beats).abs() > 1e-6)
        .then_some((before.length_beats, after.length_beats));
    let loop_changed =
        (before.is_looping != after.is_looping).then_some((before.is_looping, after.is_looping));
    let launch_changed = before.launch != after.launch;
    let follow_changed = before.follow != after.follow;
    let automation_changed = before.automation_lanes != after.automation_lanes;

    let mut notes_added = Vec::new();
    let mut notes_removed = Vec::new();
    let mut notes_modified = 0usize;
    let mut audio_path_changed = None;
    let mut audio_warped_changed = None;

    match (&before.payload, &after.payload) {
        (ClipPayload::Midi(mb), ClipPayload::Midi(ma)) => {
            diff_midi_notes(
                &mb.notes,
                &ma.notes,
                &mut notes_added,
                &mut notes_removed,
                &mut notes_modified,
            );
        }
        (ClipPayload::Audio(ab), ClipPayload::Audio(aa)) => {
            if ab.file_path != aa.file_path {
                audio_path_changed = Some((ab.file_path.clone(), aa.file_path.clone()));
            }
            if ab.is_warped != aa.is_warped {
                audio_warped_changed = Some((ab.is_warped, aa.is_warped));
            }
        }
        _ => {}
    }

    let any = renamed.is_some()
        || length_changed.is_some()
        || loop_changed.is_some()
        || !notes_added.is_empty()
        || !notes_removed.is_empty()
        || notes_modified > 0
        || audio_path_changed.is_some()
        || audio_warped_changed.is_some()
        || launch_changed
        || follow_changed
        || automation_changed;

    any.then(|| ClipDiff {
        id: after.id,
        name: after.name.clone(),
        location,
        renamed,
        length_changed,
        loop_changed,
        notes_added,
        notes_removed,
        notes_modified,
        audio_path_changed,
        audio_warped_changed,
        launch_changed,
        follow_changed,
        automation_changed,
    })
}

fn note_key(n: &MidiNote) -> (u8, i64) {
    (n.pitch, (n.start_beat * 10_000.0).round() as i64)
}

fn notes_approx_equal(a: &MidiNote, b: &MidiNote) -> bool {
    a.pitch == b.pitch
        && (a.start_beat - b.start_beat).abs() < 1e-6
        && (a.duration_beats - b.duration_beats).abs() < 1e-6
        && (a.velocity - b.velocity).abs() < 1e-4
        && (a.release_velocity - b.release_velocity).abs() < 1e-4
        && (a.probability - b.probability).abs() < 1e-4
        && (a.velocity_deviation - b.velocity_deviation).abs() < 1e-4
        && a.mute == b.mute
}

fn diff_midi_notes(
    before: &[MidiNote],
    after: &[MidiNote],
    added: &mut Vec<(u8, f64)>,
    removed: &mut Vec<(u8, f64)>,
    modified: &mut usize,
) {
    let mut b_map: BTreeMap<(u8, i64), Vec<&MidiNote>> = BTreeMap::new();
    let mut a_map: BTreeMap<(u8, i64), Vec<&MidiNote>> = BTreeMap::new();
    for n in before {
        b_map.entry(note_key(n)).or_default().push(n);
    }
    for n in after {
        a_map.entry(note_key(n)).or_default().push(n);
    }

    for (k, b_list) in &b_map {
        match a_map.get(k) {
            None => {
                for n in b_list {
                    removed.push((n.pitch, n.start_beat));
                }
            }
            Some(a_list) => {
                let common = b_list.len().min(a_list.len());
                for i in 0..common {
                    if !notes_approx_equal(b_list[i], a_list[i]) {
                        *modified += 1;
                    }
                }
                for n in &b_list[common..] {
                    removed.push((n.pitch, n.start_beat));
                }
            }
        }
    }
    for (k, a_list) in &a_map {
        let b_len = b_map.get(k).map_or(0, Vec::len);
        for n in a_list.iter().skip(b_len) {
            added.push((n.pitch, n.start_beat));
        }
    }
}

type DeviceDiffParts = (
    Vec<(DeviceId, String)>,
    Vec<(DeviceId, String)>,
    Vec<DeviceDiff>,
);

fn diff_track_devices(before: &[Device], after: &[Device]) -> DeviceDiffParts {
    let b_map: HashMap<DeviceId, &Device> = before.iter().map(|d| (d.id, d)).collect();
    let a_map: HashMap<DeviceId, &Device> = after.iter().map(|d| (d.id, d)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for d in before {
        if !a_map.contains_key(&d.id) {
            removed.push((d.id, d.name.clone()));
        }
    }
    for d in after {
        let Some(prev) = b_map.get(&d.id) else {
            added.push((d.id, d.name.clone()));
            continue;
        };
        let renamed = (prev.name != d.name).then(|| (prev.name.clone(), d.name.clone()));
        let enabled_changed =
            (prev.is_enabled != d.is_enabled).then_some((prev.is_enabled, d.is_enabled));

        let b_params: BTreeMap<&str, f32> = prev.named_params().collect();
        let a_params: BTreeMap<&str, f32> = d.named_params().collect();
        let mut params_changed = Vec::new();
        for (&k, &bv) in &b_params {
            match a_params.get(k) {
                Some(&av) if (av - bv).abs() <= 1e-5 => {}
                other => params_changed.push(ParamDiff {
                    key: k.to_owned(),
                    before: Some(bv),
                    after: other.copied(),
                }),
            }
        }
        for (&k, &av) in &a_params {
            if !b_params.contains_key(k) {
                params_changed.push(ParamDiff {
                    key: k.to_owned(),
                    before: None,
                    after: Some(av),
                });
            }
        }
        if renamed.is_some() || enabled_changed.is_some() || !params_changed.is_empty() {
            modified.push(DeviceDiff {
                id: d.id,
                name: d.name.clone(),
                renamed,
                enabled_changed,
                params_changed,
            });
        }
    }
    (added, removed, modified)
}

fn diff_automation_lanes(
    before: &[AutomationLane],
    after: &[AutomationLane],
) -> (usize, usize, usize) {
    let b_map: HashMap<LaneId, &AutomationLane> = before.iter().map(|l| (l.id, l)).collect();
    let a_map: HashMap<LaneId, &AutomationLane> = after.iter().map(|l| (l.id, l)).collect();
    let removed = before.iter().filter(|l| !a_map.contains_key(&l.id)).count();
    let mut added = 0usize;
    let mut modified = 0usize;
    for l in after {
        match b_map.get(&l.id) {
            None => added += 1,
            Some(prev) if *prev != l => modified += 1,
            Some(_) => {}
        }
    }
    (added, removed, modified)
}

/// Formats a human-readable musical summary of a [`LiveSetDiff`].
#[must_use]
pub fn format_diff_summary(diff: &LiveSetDiff) -> String {
    if diff.is_empty() {
        return "No musical changes.".to_owned();
    }
    let mut out = String::new();

    if let Some((a, b)) = diff.bpm_changed {
        let _ = writeln!(out, "Tempo: {a:.2} BPM -> {b:.2} BPM");
    } else if diff.tempo_map_changed {
        let _ = writeln!(out, "Tempo automation map changed");
    }
    if let Some(((n1, d1), (n2, d2))) = diff.time_signature_changed {
        let _ = writeln!(out, "Time signature: {n1}/{d1} -> {n2}/{d2}");
    } else if diff.time_signature_map_changed {
        let _ = writeln!(out, "Time signature map changed");
    }
    if let Some((a, b)) = diff.loop_enabled_changed {
        let _ = writeln!(out, "Loop: {a} -> {b}");
    }
    if let Some(((s1, l1), (s2, l2))) = diff.loop_range_changed {
        let _ = writeln!(out, "Loop range: ({s1}, {l1}) -> ({s2}, {l2})");
    }
    if let Some((a, b)) = diff.global_quantization_changed {
        let _ = writeln!(out, "Global Quantization: {a:?} -> {b:?}");
    }
    if let Some((a, b)) = &diff.scale_changed {
        let _ = writeln!(
            out,
            "Scale: {} {} (active={}) -> {} {} (active={})",
            a.root_note,
            a.scale_type.name(),
            a.is_active,
            b.root_note,
            b.scale_type.name(),
            b.is_active
        );
    }
    if diff.tuning_changed {
        let _ = writeln!(out, "Tuning system changed: {:?}", diff.tuning_names);
    }
    for (id, name) in &diff.scenes_added {
        let _ = writeln!(out, "+ Scene {}: {name}", id.0);
    }
    for (id, name) in &diff.scenes_removed {
        let _ = writeln!(out, "- Scene {}: {name}", id.0);
    }
    for sc in &diff.scenes_modified {
        let _ = writeln!(out, "~ Scene {} ({})", sc.id.0, sc.name);
    }
    for (id, name, tt) in &diff.tracks_added {
        let _ = writeln!(out, "+ Track {} ({tt:?}): {name}", id.0);
    }
    for (id, name, tt) in &diff.tracks_removed {
        let _ = writeln!(out, "- Track {} ({tt:?}): {name}", id.0);
    }
    for trk in &diff.tracks_modified {
        let _ = writeln!(out, "~ Track {} ({})", trk.id.0, trk.name);
        if let Some((a, b)) = &trk.renamed {
            let _ = writeln!(out, "  name: {a} -> {b}");
        }
        if let Some((a, b)) = trk.volume_db_changed {
            let _ = writeln!(out, "  volume: {a:.2} dB -> {b:.2} dB");
        }
        if let Some((a, b)) = trk.pan_changed {
            let _ = writeln!(out, "  pan: {a:.3} -> {b:.3}");
        }
        if let Some((a, b)) = trk.muted_changed {
            let _ = writeln!(out, "  muted: {a} -> {b}");
        }
        if let Some((a, b)) = trk.soloed_changed {
            let _ = writeln!(out, "  soloed: {a} -> {b}");
        }
        if let Some((a, b)) = trk.armed_changed {
            let _ = writeln!(out, "  armed: {a} -> {b}");
        }
        for (idx, a, b) in &trk.sends_changed {
            let _ = writeln!(out, "  send[{idx}]: {a:.4} -> {b:.4}");
        }
        for (cid, cname, loc) in &trk.clips_added {
            let _ = writeln!(out, "  + clip {} ({cname}) at {loc:?}", cid.0);
        }
        for (cid, cname, loc) in &trk.clips_removed {
            let _ = writeln!(out, "  - clip {} ({cname}) at {loc:?}", cid.0);
        }
        for cd in &trk.clips_modified {
            let _ = writeln!(out, "  ~ clip {} ({})", cd.id.0, cd.name);
            for (pitch, beat) in &cd.notes_added {
                let _ = writeln!(out, "    + note {} @ {beat}", note_name(*pitch));
            }
            for (pitch, beat) in &cd.notes_removed {
                let _ = writeln!(out, "    - note {} @ {beat}", note_name(*pitch));
            }
            if cd.notes_modified > 0 {
                let _ = writeln!(out, "    ~ {} note(s) modified", cd.notes_modified);
            }
        }
        for (did, dname) in &trk.devices_added {
            let _ = writeln!(out, "  + device {} ({dname})", did.0);
        }
        for (did, dname) in &trk.devices_removed {
            let _ = writeln!(out, "  - device {} ({dname})", did.0);
        }
        for dd in &trk.devices_modified {
            let _ = writeln!(out, "  ~ device {} ({})", dd.id.0, dd.name);
            if let Some((a, b)) = dd.enabled_changed {
                let _ = writeln!(out, "    enabled: {a} -> {b}");
            }
            for pd in &dd.params_changed {
                let _ = writeln!(
                    out,
                    "    param {}: {:?} -> {:?}",
                    pd.key, pd.before, pd.after
                );
            }
        }
        if trk.automation_lanes_added + trk.automation_lanes_removed + trk.automation_lanes_modified
            > 0
        {
            let _ = writeln!(
                out,
                "  automation: +{} -{} ~{}",
                trk.automation_lanes_added,
                trk.automation_lanes_removed,
                trk.automation_lanes_modified
            );
        }
    }

    out.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::model::{Clip, ClipPayload, MidiNote};

    #[test]
    fn identical_projects_produce_empty_diff() {
        let p = Project::new_default("Identical");
        let diff = diff_projects(&p, &p);
        assert!(diff.is_empty());
        assert_eq!(format_diff_summary(&diff), "No musical changes.");
    }

    #[test]
    fn diff_captures_tempo_tracks_notes_and_device_params() {
        let before = Project::new_default("Before");
        let mut after = before.clone();
        after.transport.set_bpm(134.0);
        after.tracks[0].name = "Bass Lead".to_owned();
        after.tracks[0].volume_db = -4.5;
        after.tracks[0].device_chain[0].set_param("filter_cutoff", 980.0);

        let mut clip = Clip::new_midi(901, "Bass Line", 4.0);
        if let ClipPayload::Midi(ref mut md) = clip.payload {
            md.notes.push(MidiNote::new(60, 0.0, 0.5, 110));
        }
        after.tracks[0].clip_slots[0].clip = Some(clip);

        let diff = diff_projects(&before, &after);
        assert!(!diff.is_empty());
        assert_eq!(diff.bpm_changed, Some((120.0, 134.0)));
        assert_eq!(diff.tracks_modified.len(), 1);
        let td = &diff.tracks_modified[0];
        assert_eq!(
            td.renamed,
            Some(("1-MIDI".to_owned(), "Bass Lead".to_owned()))
        );
        assert_eq!(td.clips_added.len(), 1);
        assert_eq!(td.devices_modified.len(), 1);
        assert_eq!(
            td.devices_modified[0].params_changed[0].key,
            "filter_cutoff"
        );

        let summary = format_diff_summary(&diff);
        assert!(summary.contains("120.00 BPM -> 134.00 BPM"));
        assert!(summary.contains("Bass Lead"));
        assert!(summary.contains("filter_cutoff"));
    }
}
