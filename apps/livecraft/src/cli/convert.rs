use std::fs;
use std::path::Path;

use livecraft_formats::als::diff::{
    ClipDiff, ClipLocation, DeviceDiff, LiveSetDiff, ParamDiff, SceneDiff, TrackDiff,
};
use livecraft_formats::als::{AlsDocument, diff_projects, format_diff_summary};
use livecraft_model::model::Project;

/// Returns `true` when `path` has a `.json` extension (case-insensitive).
pub fn is_json_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

/// Loads a [`Project`] from a `.als`, `.xml`, or `.json` Live Set path.
pub fn load_project_from_path(path: &Path) -> anyhow::Result<Project> {
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    if is_json_path(path) {
        let data = fs::read_to_string(path)?;
        let proj: Project = serde_json::from_str(&data)?;
        return Ok(proj);
    }
    Ok(AlsDocument::open(path)
        .map_err(|e| anyhow::anyhow!("ALS load error: {e}"))?
        .into_project())
}

/// Loads a lossless [`AlsDocument`] from a `.als`, `.xml`, or `.json` Live Set path.
pub fn load_document_from_path(path: &Path) -> anyhow::Result<AlsDocument> {
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    if is_json_path(path) {
        let data = fs::read_to_string(path)?;
        let proj: Project = serde_json::from_str(&data)?;
        return AlsDocument::from_project(&proj)
            .map_err(|e| anyhow::anyhow!("ALS build error: {e}"));
    }
    AlsDocument::open(path).map_err(|e| anyhow::anyhow!("ALS load error: {e}"))
}

/// Converts between `.als`, canonical pretty `.xml`, and `.json` Live Sets,
/// preserving unmodeled XML nodes and devices across `.als` / `.xml` conversions.
pub fn run_convert(input: &Path, output: &Path, pretty_xml: bool) -> anyhow::Result<()> {
    let doc = load_document_from_path(input)?;
    let out_ext = output.extension().and_then(|s| s.to_str()).unwrap_or("");

    if pretty_xml || out_ext.eq_ignore_ascii_case("xml") {
        let xml = doc.to_pretty_xml()?;
        fs::write(output, xml)?;
        println!(
            "Converted '{}' -> XML: '{}'",
            input.display(),
            output.display()
        );
    } else if out_ext.eq_ignore_ascii_case("json") {
        let json = serde_json::to_string_pretty(doc.project())?;
        fs::write(output, json)?;
        println!(
            "Converted '{}' -> JSON: '{}'",
            input.display(),
            output.display()
        );
    } else {
        doc.save_als(output)?;
        println!(
            "Converted '{}' -> Ableton .als: '{}'",
            input.display(),
            output.display()
        );
    }

    Ok(())
}

/// Compares two `.als`, `.xml`, or `.json` Live Sets and prints either a musical
/// summary or a structured JSON diff.
pub fn run_diff(a: &Path, b: &Path, json: bool) -> anyhow::Result<()> {
    let proj_a = load_project_from_path(a)?;
    let proj_b = load_project_from_path(b)?;
    let diff = diff_projects(&proj_a, &proj_b);

    if json {
        let payload = diff_to_json(&diff);
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("{}", format_diff_summary(&diff));
    }

    Ok(())
}

fn diff_to_json(diff: &LiveSetDiff) -> serde_json::Value {
    let scenes_modified: Vec<serde_json::Value> = diff
        .scenes_modified
        .iter()
        .map(scene_diff_to_json)
        .collect();
    let tracks_modified: Vec<serde_json::Value> = diff
        .tracks_modified
        .iter()
        .map(track_diff_to_json)
        .collect();

    serde_json::json!({
        "bpm_changed": diff.bpm_changed,
        "tempo_map_changed": diff.tempo_map_changed,
        "time_signature_changed": diff.time_signature_changed,
        "time_signature_map_changed": diff.time_signature_map_changed,
        "loop_enabled_changed": diff.loop_enabled_changed,
        "loop_range_changed": diff.loop_range_changed,
        "global_quantization_changed": diff.global_quantization_changed,
        "scale_changed": diff.scale_changed,
        "tuning_changed": diff.tuning_changed,
        "tuning_names": diff.tuning_names,
        "scenes_added": diff.scenes_added,
        "scenes_removed": diff.scenes_removed,
        "scenes_modified": scenes_modified,
        "tracks_added": diff.tracks_added,
        "tracks_removed": diff.tracks_removed,
        "tracks_modified": tracks_modified,
    })
}

fn scene_diff_to_json(scene: &SceneDiff) -> serde_json::Value {
    serde_json::json!({
        "id": scene.id,
        "name": scene.name,
        "renamed": scene.renamed,
        "tempo_changed": scene.tempo_changed,
        "time_signature_changed": scene.time_signature_changed,
        "color_changed": scene.color_changed,
    })
}

fn track_diff_to_json(track: &TrackDiff) -> serde_json::Value {
    let clips_added: Vec<serde_json::Value> = track
        .clips_added
        .iter()
        .map(|(id, name, loc)| serde_json::json!([id, name, clip_location_to_json(loc)]))
        .collect();
    let clips_removed: Vec<serde_json::Value> = track
        .clips_removed
        .iter()
        .map(|(id, name, loc)| serde_json::json!([id, name, clip_location_to_json(loc)]))
        .collect();
    let clips_modified: Vec<serde_json::Value> =
        track.clips_modified.iter().map(clip_diff_to_json).collect();
    let devices_modified: Vec<serde_json::Value> = track
        .devices_modified
        .iter()
        .map(device_diff_to_json)
        .collect();

    serde_json::json!({
        "id": track.id,
        "name": track.name,
        "track_type": track.track_type,
        "renamed": track.renamed,
        "volume_db_changed": track.volume_db_changed,
        "pan_changed": track.pan_changed,
        "muted_changed": track.muted_changed,
        "soloed_changed": track.soloed_changed,
        "armed_changed": track.armed_changed,
        "sends_changed": track.sends_changed,
        "clips_added": clips_added,
        "clips_removed": clips_removed,
        "clips_modified": clips_modified,
        "devices_added": track.devices_added,
        "devices_removed": track.devices_removed,
        "devices_modified": devices_modified,
        "automation_lanes_added": track.automation_lanes_added,
        "automation_lanes_removed": track.automation_lanes_removed,
        "automation_lanes_modified": track.automation_lanes_modified,
    })
}

fn clip_location_to_json(loc: &ClipLocation) -> serde_json::Value {
    match loc {
        ClipLocation::SessionSlot(slot) => serde_json::json!({ "SessionSlot": slot }),
        ClipLocation::ArrangementBeat(beat) => serde_json::json!({ "ArrangementBeat": beat }),
    }
}

fn clip_diff_to_json(clip: &ClipDiff) -> serde_json::Value {
    serde_json::json!({
        "id": clip.id,
        "name": clip.name,
        "location": clip_location_to_json(&clip.location),
        "renamed": clip.renamed,
        "length_changed": clip.length_changed,
        "loop_changed": clip.loop_changed,
        "notes_added": clip.notes_added,
        "notes_removed": clip.notes_removed,
        "notes_modified": clip.notes_modified,
        "audio_path_changed": clip.audio_path_changed,
        "audio_warped_changed": clip.audio_warped_changed,
        "launch_changed": clip.launch_changed,
        "follow_changed": clip.follow_changed,
        "automation_changed": clip.automation_changed,
    })
}

fn device_diff_to_json(device: &DeviceDiff) -> serde_json::Value {
    let params_changed: Vec<serde_json::Value> = device
        .params_changed
        .iter()
        .map(param_diff_to_json)
        .collect();
    serde_json::json!({
        "id": device.id,
        "name": device.name,
        "renamed": device.renamed,
        "enabled_changed": device.enabled_changed,
        "params_changed": params_changed,
    })
}

fn param_diff_to_json(param: &ParamDiff) -> serde_json::Value {
    serde_json::json!({
        "key": param.key,
        "before": param.before,
        "after": param.after,
    })
}
