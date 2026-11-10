//! M10 project/file management, missing-file repair, Collect All and Save, unused files,
//! `.alc`/`.adv`/`.adg`, SMF `.mid`, Pack `.alp`, and B11 health report methods on [`Session`].

use crate::command::CommandError;
use crate::commands::clip::{ReplaceClipSample, clip_mut};
use crate::commands::project::{
    InsertPreparedArrangementClip, InsertPreparedClip, InsertPreparedDevice, ReplaceSampleReference,
};
use crate::commands::track::{CreateAudioTrack, CreateMidiTrack};
use crate::preferences::{LibraryPreferences, PreferencesError};
use crate::session::Session;
use livecraft_formats::als::AlsError;
use livecraft_formats::als::map::AlsDocument;
use livecraft_formats::als::read::read_xml_tree;
use livecraft_formats::als::tree::XmlTree;
use livecraft_formats::als::write::{XmlFormatMode, write_xml_tree};
use livecraft_formats::analysis_cache::{
    CachedSampleAnalysis, analysis_cache_path, build_waveform_pyramid_streaming,
    load_or_build_analysis,
};
use livecraft_formats::audio_decode::{AudioDecodeError, probe_audio_info};
use livecraft_formats::health::{
    HealthCheckOptions, HealthError, ProjectHealthReport, check_document_health,
    check_project_health,
};
use livecraft_formats::live_clip::{LiveClipFile, load_live_clip, save_live_clip_in_context};
use livecraft_formats::midi_file::{SmfError, SmfFormat, export_clip_to_smf, read_smf_to_clip};
use livecraft_formats::pack::{PackError, PackInstallReport, install_pack as extract_pack};
use livecraft_formats::presets::{
    RackPreset, load_device_preset as read_device_preset, load_rack_preset as read_rack_preset,
    save_device_preset as write_device_preset, save_rack_preset as write_rack_preset,
};
use livecraft_formats::project_dir::{
    AutoRepairReport, AutomaticSearchOptions, BACKUP_DIR, CollectAllOptions, CollectReport,
    MissingFile, PROJECT_INFO_DIR, ProjectContext, ProjectDirError, SampleLocation,
    SampleReference, SampleResolution, SavedSetLocation, SearchCandidateOutcome, UnusedFilesReport,
    automatic_search, detect_missing_files, detect_missing_files_in_document, detect_project_root,
    find_unused_files as scan_unused_files, is_project_root, replace_xml_sample_reference,
    resolve_sample_reference, save_set_into_project,
};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, DeviceId, TrackId};
use livecraft_model::model::{
    ArrangementClip, AudioClipData, Clip, ClipPayload, Project, Track, TrackType,
};
use livecraft_time::Beats;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Outcome of exporting a Live Clip (`.alc`) under the `CollectFilesOnExport` policy (manual 5.7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportClipOutcome {
    Saved { copied_samples: bool },
    NeedsCopyConfirmation,
}

/// Errors returned by `.mid`, `.alc`, `.adv`, `.adg`, and `.alp` workflows on [`Session`].
#[derive(Debug, thiserror::Error)]
pub enum SessionFormatError {
    #[error(transparent)]
    Als(#[from] AlsError),
    #[error(transparent)]
    Smf(#[from] SmfError),
    #[error(transparent)]
    Pack(#[from] PackError),
    #[error(transparent)]
    ProjectDir(#[from] ProjectDirError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error(transparent)]
    AudioDecode(#[from] AudioDecodeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cannot place clip: {0}")]
    InvalidPlacement(String),
    #[error("track {track} ({actual:?}) cannot hold a {expected:?} clip")]
    TrackTypeMismatch {
        track: TrackId,
        expected: TrackType,
        actual: TrackType,
    },
    #[error("no User Library folder is configured or available")]
    MissingUserLibrary,
}

#[derive(Clone, Copy)]
enum ClipPlacement {
    Slot(usize),
    Arrangement(f64),
}

pub(crate) fn build_project_context(
    set_path: Option<&Path>,
    library: &LibraryPreferences,
) -> ProjectContext {
    let mut ctx = match set_path {
        Some(path) => ProjectContext::from_set_path(path),
        None => ProjectContext::default(),
    };
    ctx.user_library = library.effective_user_library();
    ctx.factory_packs_roots = library.factory_packs_roots.clone();
    ctx
}

pub(crate) fn has_sample_refs(project: &Project) -> bool {
    project.all_tracks().any(|track| {
        track.clip_slots.iter().any(|slot| {
            slot.clip
                .as_ref()
                .is_some_and(|c| matches!(c.payload, ClipPayload::Audio(_)))
        }) || track
            .arrangement_clips
            .iter()
            .any(|arr| matches!(arr.clip.payload, ClipPayload::Audio(_)))
            || track
                .device_chain
                .iter()
                .any(|dev| !dev.sample_refs.is_empty())
    })
}

fn normalize_stored_path(path: &Path) -> String {
    let raw = path.to_string_lossy().into_owned();
    if raw.starts_with(r"\\?\") {
        raw
    } else {
        raw.replace('\\', "/")
    }
}

fn canonical_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn project_relative_path(path: &Path, ctx: &ProjectContext) -> Option<String> {
    let root = ctx.project_root.as_deref()?;
    let canon_root = canonical_or_self(root);
    let canon_path = canonical_or_self(path);
    let rel = canon_path.strip_prefix(&canon_root).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    (!s.is_empty()).then_some(s)
}

fn xml_read_val(
    tree: &XmlTree,
    parent: livecraft_formats::als::tree::XmlNodeId,
    child: &str,
) -> Option<String> {
    let cid = tree.child(parent, child)?;
    tree.value_attr(cid).map(str::to_owned).or_else(|| {
        let t = tree.text_content(cid);
        (!t.trim().is_empty()).then(|| t.trim().to_owned())
    })
}

fn xml_tree_has_file_ref(tree: &XmlTree, target_ref: &str) -> bool {
    if target_ref.trim().is_empty() {
        return false;
    }
    let mut stack = vec![tree.root()];
    while let Some(nid) = stack.pop() {
        let Some(node) = tree.node(nid) else {
            continue;
        };
        if node.element_name() == Some("FileRef") {
            let path_val = xml_read_val(tree, nid, "Path").unwrap_or_default();
            let rel_val = xml_read_val(tree, nid, "RelativePath").unwrap_or_default();
            if path_val == target_ref || rel_val == target_ref {
                return true;
            }
            continue;
        }
        for &cid in node.children().iter().rev() {
            stack.push(cid);
        }
    }
    false
}

fn patch_doc_xml_sample_refs(
    doc: &mut AlsDocument,
    project_name: &str,
    patches: &[(String, String, Option<String>)],
) -> Result<usize, AlsError> {
    let needs_xml_patch = patches
        .iter()
        .any(|(old_ref, _, _)| xml_tree_has_file_ref(doc.tree(), old_ref));
    if !needs_xml_patch {
        return Ok(0);
    }
    let xml = doc.to_xml()?;
    let mut tree = read_xml_tree(&xml)?;
    let mut total = 0usize;
    for (old_ref, new_path, new_rel) in patches {
        total += replace_xml_sample_reference(&mut tree, old_ref, new_path, new_rel.as_deref());
    }
    if total > 0 {
        let updated_xml = write_xml_tree(&tree, XmlFormatMode::LiveExact)?;
        *doc = AlsDocument::from_xml_named(&updated_xml, project_name)?;
    }
    Ok(total)
}

fn count_project_sample_ref(project: &Project, target_ref: &str) -> usize {
    if target_ref.trim().is_empty() {
        return 0;
    }
    let mut count = 0usize;
    for track in project.all_tracks() {
        for slot in &track.clip_slots {
            if let Some(ref clip) = slot.clip
                && let ClipPayload::Audio(ref audio) = clip.payload
                && (audio.file_path == target_ref
                    || audio.relative_path.as_deref() == Some(target_ref))
            {
                count += 1;
            }
        }
        for arr in &track.arrangement_clips {
            if let ClipPayload::Audio(ref audio) = arr.clip.payload
                && (audio.file_path == target_ref
                    || audio.relative_path.as_deref() == Some(target_ref))
            {
                count += 1;
            }
        }
        for dev in &track.device_chain {
            for sref in &dev.sample_refs {
                if sref.path == target_ref || sref.relative_path.as_deref() == Some(target_ref) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn collect_audio_clips(project: &Project) -> Vec<AudioClipData> {
    let mut out = Vec::new();
    for track in project.all_tracks() {
        for slot in &track.clip_slots {
            if let Some(ref clip) = slot.clip
                && let ClipPayload::Audio(ref audio) = clip.payload
            {
                out.push(audio.clone());
            }
        }
        for arr in &track.arrangement_clips {
            if let ClipPayload::Audio(ref audio) = arr.clip.payload {
                out.push(audio.clone());
            }
        }
    }
    out
}

fn collect_in_project_refs(project: &Project, ctx: &ProjectContext) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    let mut record = |path: &str, rel: Option<&str>| {
        if path.trim().is_empty() && rel.is_none_or(|r| r.trim().is_empty()) {
            return;
        }
        let r = SampleReference {
            path: path.to_owned(),
            relative_path: rel.map(str::to_owned),
        };
        if let SampleResolution::Resolved {
            resolved_path,
            location: SampleLocation::InProject,
            ..
        } = resolve_sample_reference(&r, ctx)
        {
            out.insert(canonical_or_self(&resolved_path));
        }
    };
    for track in project.all_tracks() {
        for slot in &track.clip_slots {
            if let Some(ref clip) = slot.clip
                && let ClipPayload::Audio(ref audio) = clip.payload
            {
                record(&audio.file_path, audio.relative_path.as_deref());
            }
        }
        for arr in &track.arrangement_clips {
            if let ClipPayload::Audio(ref audio) = arr.clip.payload {
                record(&audio.file_path, audio.relative_path.as_deref());
            }
        }
        for dev in &track.device_chain {
            for sref in &dev.sample_refs {
                record(&sref.path, sref.relative_path.as_deref());
            }
        }
    }
    out
}

fn find_clip_on_track(track: &Track, clip_id: ClipId) -> Option<&Clip> {
    track
        .clip_slots
        .iter()
        .filter_map(|slot| slot.clip.as_ref())
        .chain(track.arrangement_clips.iter().map(|arr| &arr.clip))
        .find(|clip| clip.id == clip_id)
}

fn find_clip_in_project(project: &Project, clip_id: ClipId) -> Option<&Clip> {
    project
        .all_tracks()
        .find_map(|track| find_clip_on_track(track, clip_id))
}

fn resolve_clip_sample_path(audio: &AudioClipData, ctx: &ProjectContext) -> Option<PathBuf> {
    if audio.file_path.trim().is_empty()
        && audio
            .relative_path
            .as_deref()
            .is_none_or(|r| r.trim().is_empty())
    {
        return None;
    }
    let reference = SampleReference {
        path: audio.file_path.clone(),
        relative_path: audio.relative_path.clone(),
    };
    match resolve_sample_reference(&reference, ctx) {
        SampleResolution::Resolved { resolved_path, .. } => Some(resolved_path),
        SampleResolution::Missing { .. } => None,
    }
}

fn resolve_placement(
    project: &Project,
    track_id: Option<TrackId>,
    slot_idx: Option<usize>,
    start_beat: Option<f64>,
    expected_track_type: TrackType,
) -> Result<(ClipPlacement, bool), SessionFormatError> {
    if slot_idx.is_some() && start_beat.is_some() {
        return Err(SessionFormatError::InvalidPlacement(
            "cannot specify both slot_idx and start_beat".to_owned(),
        ));
    }
    if let Some(beat) = start_beat
        && (!beat.is_finite() || beat < 0.0)
    {
        return Err(SessionFormatError::InvalidPlacement(format!(
            "arrangement start_beat must be finite and non-negative, got {beat}"
        )));
    }

    let Some(tid) = track_id else {
        let placement = match (slot_idx, start_beat) {
            (Some(slot), None) => ClipPlacement::Slot(slot),
            (None, Some(beat)) => ClipPlacement::Arrangement(beat),
            (None, None) => ClipPlacement::Slot(0),
            (Some(_), Some(_)) => unreachable!("checked above"),
        };
        return Ok((placement, true));
    };

    let track = project.track(tid).ok_or(EditError::NoSuchTrack(tid))?;
    match track.track_type {
        TrackType::Group => return Err(EditError::GroupHoldsClips.into()),
        TrackType::Return | TrackType::Main => return Err(EditError::ReturnHoldsClips.into()),
        actual if actual != expected_track_type => {
            return Err(SessionFormatError::TrackTypeMismatch {
                track: tid,
                expected: expected_track_type,
                actual,
            });
        }
        _ => {}
    }

    let is_empty_track = track.device_chain.is_empty()
        && track.clip_slots.iter().all(|s| s.clip.is_none())
        && track.arrangement_clips.is_empty();

    let placement = match (slot_idx, start_beat) {
        (Some(slot), None) => {
            let slot_ref = track
                .clip_slots
                .get(slot)
                .ok_or(EditError::NoSuchSlot { track: tid, slot })?;
            if slot_ref.clip.is_some() {
                return Err(CommandError::SlotOccupied { track: tid, slot }.into());
            }
            ClipPlacement::Slot(slot)
        }
        (None, Some(beat)) => ClipPlacement::Arrangement(beat),
        (None, None) => {
            let empty_slot = track
                .clip_slots
                .iter()
                .position(|s| s.clip.is_none())
                .ok_or_else(|| {
                    SessionFormatError::InvalidPlacement(format!(
                        "track {tid} has no empty clip slot"
                    ))
                })?;
            ClipPlacement::Slot(empty_slot)
        }
        (Some(_), Some(_)) => unreachable!("checked above"),
    };

    Ok((placement, is_empty_track))
}

fn resolve_export_copy_dir(out_path: &Path, ctx: &ProjectContext) -> PathBuf {
    if let Some(root) = detect_project_root(out_path) {
        return root;
    }
    if let Some(parent) = out_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        return parent.to_path_buf();
    }
    ctx.project_root
        .clone()
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Session {
    /// Path of the open or last-saved `.als` file, if any.
    pub fn set_path(&self) -> Option<&Path> {
        self.set_path.as_deref()
    }

    /// Active [`ProjectContext`] used for relative sample resolution and project-folder checks.
    pub fn project_context(&self) -> &ProjectContext {
        &self.project_context
    }

    /// Missing audio/sample file references detected on the Set.
    pub fn missing_files(&self) -> &[MissingFile] {
        &self.missing_files
    }

    /// Recomputes [`Self::missing_files`] against the current Set and [`ProjectContext`].
    pub fn refresh_missing_files(&mut self) -> &[MissingFile] {
        self.missing_files = match self.als_doc.as_ref() {
            Some(doc) if !self.dirty => {
                detect_missing_files_in_document(doc, &self.project_context)
            }
            Some(doc) if has_sample_refs(&self.project) || has_sample_refs(doc.project()) => {
                let mut synced = doc.clone();
                match synced.apply_project(&self.project) {
                    Ok(()) => detect_missing_files_in_document(&synced, &self.project_context),
                    Err(err) => {
                        log::warn!("Failed to sync AlsDocument for missing-file check: {err}");
                        detect_missing_files(&self.project, &self.project_context)
                    }
                }
            }
            Some(doc) => detect_missing_files_in_document(doc, &self.project_context),
            None => detect_missing_files(&self.project, &self.project_context),
        };
        &self.missing_files
    }

    pub(crate) fn update_project_context(&mut self) {
        let next = build_project_context(self.set_path.as_deref(), &self.preferences.library);
        let changed = next != self.project_context;
        self.project_context = next;
        if changed && let Some(engine) = self.engine.as_mut() {
            engine.set_project_context(self.project_context.clone());
            if has_sample_refs(&self.project) {
                let graph = engine.compiler.compile(&self.project);
                if let Err(err) = engine.handle.submit_graph(graph) {
                    log::warn!("Failed to refresh audio graph after context change: {err}");
                }
            }
        }
        self.refresh_missing_files();
    }

    /// Updates [`LibraryPreferences`], refreshes the active [`ProjectContext`] and engine
    /// compiler context, and persists preferences when a preferences file is attached.
    pub fn set_library_preferences(
        &mut self,
        library: LibraryPreferences,
    ) -> Result<(), PreferencesError> {
        self.preferences.library = library;
        self.update_project_context();
        if let Some(path) = &self.prefs_path {
            self.preferences.save(path)?;
        }
        Ok(())
    }

    /// Saves the Set into an enclosing Live Project or creates `<Name> Project/` when outside
    /// (manual 5.5.1, LC-006).
    pub fn save_into_project(
        &mut self,
        target: &Path,
    ) -> Result<SavedSetLocation, ProjectDirError> {
        let saved = if let Some(ref mut doc) = self.als_doc {
            doc.apply_project(&self.project)?;
            save_set_into_project(doc, target)?
        } else {
            let doc = AlsDocument::from_project(&self.project)?;
            let saved = save_set_into_project(&doc, target)?;
            self.als_doc = Some(doc);
            saved
        };
        self.dirty = false;
        self.set_path = Some(saved.set_path.clone());
        self.update_project_context();
        Ok(saved)
    }

    /// Manually replaces `missing_ref` with any existing `replacement` file (manual 5.6, p. 147–149).
    /// Undoable via [`ReplaceSampleReference`].
    pub fn locate_missing_manual(
        &mut self,
        missing_ref: &str,
        replacement: &Path,
    ) -> Result<usize, ProjectDirError> {
        if replacement.as_os_str().is_empty() || replacement.to_string_lossy().contains('\0') {
            return Err(ProjectDirError::InvalidPath(format!(
                "invalid replacement path: {}",
                replacement.display()
            )));
        }
        if !replacement.exists() {
            return Err(ProjectDirError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("replacement file not found: {}", replacement.display()),
            )));
        }
        if !replacement.is_file() {
            return Err(ProjectDirError::InvalidPath(format!(
                "replacement path is not a file: {}",
                replacement.display()
            )));
        }
        if missing_ref.trim().is_empty() {
            return Ok(0);
        }

        let abs_replacement =
            std::path::absolute(replacement).unwrap_or_else(|_| replacement.to_path_buf());
        let stored_path = normalize_stored_path(&abs_replacement);
        let new_rel = project_relative_path(&abs_replacement, &self.project_context);

        let model_matches = count_project_sample_ref(&self.project, missing_ref);
        if model_matches > 0 {
            self.execute(ReplaceSampleReference::new(
                missing_ref,
                &stored_path,
                new_rel.clone(),
            ))
            .map_err(|e| ProjectDirError::InvalidPath(e.to_string()))?;
        }

        let mut xml_patched = 0usize;
        if let Some(ref mut doc) = self.als_doc {
            doc.apply_project(&self.project)?;
            let patches = vec![(missing_ref.to_owned(), stored_path, new_rel)];
            xml_patched = patch_doc_xml_sample_refs(doc, &self.project.name, &patches)?;
            if xml_patched > 0 {
                self.dirty = true;
            }
        }

        self.refresh_missing_files();
        Ok(model_matches + xml_patched)
    }

    /// Runs automatic search for missing files (manual 5.6, p. 147–149), repairing single-candidate
    /// matches in one `"Locate Missing Files"` undo transaction and leaving zero/multi-candidate
    /// matches unresolved.
    pub fn locate_missing_auto(
        &mut self,
        opts: &AutomaticSearchOptions,
    ) -> Result<AutoRepairReport, ProjectDirError> {
        let missing = self.refresh_missing_files().to_vec();
        let outcomes = automatic_search(&missing, opts, &self.project_context)?;

        let mut report = AutoRepairReport::default();
        let mut repairs: Vec<(SampleReference, String, Option<String>)> = Vec::new();
        for (item, outcome) in outcomes {
            if let SearchCandidateOutcome::OneCandidate(found) = outcome {
                let abs_found = if found.is_absolute() {
                    found.clone()
                } else {
                    std::path::absolute(&found).unwrap_or_else(|_| found.clone())
                };
                let stored = normalize_stored_path(&abs_found);
                let rel_opt = project_relative_path(&abs_found, &self.project_context);
                report
                    .repaired
                    .push((item.reference.display_ref().to_owned(), found));
                repairs.push((item.reference, stored, rel_opt));
            } else {
                report.unresolved.push((item, outcome));
            }
        }

        if !repairs.is_empty() {
            let mut xml_patches: Vec<(String, String, Option<String>)> = Vec::new();
            self.transaction("Locate Missing Files", |tx| {
                for (reference, stored, rel_opt) in &repairs {
                    let mut touched_primary = false;
                    if !reference.path.trim().is_empty() {
                        if count_project_sample_ref(tx.project(), &reference.path) > 0 {
                            tx.run(ReplaceSampleReference::new(
                                &reference.path,
                                stored,
                                rel_opt.clone(),
                            ))?;
                            touched_primary = true;
                        }
                        xml_patches.push((reference.path.clone(), stored.clone(), rel_opt.clone()));
                    }
                    if let Some(ref rel) = reference.relative_path
                        && (!touched_primary || rel != &reference.path)
                    {
                        if count_project_sample_ref(tx.project(), rel) > 0 {
                            tx.run(ReplaceSampleReference::new(rel, stored, rel_opt.clone()))?;
                        }
                        if rel != &reference.path {
                            xml_patches.push((rel.clone(), stored.clone(), rel_opt.clone()));
                        }
                    }
                }
                Ok(())
            })
            .map_err(|e| ProjectDirError::InvalidPath(e.to_string()))?;

            if let Some(ref mut doc) = self.als_doc {
                doc.apply_project(&self.project)?;
                let xml_patched = patch_doc_xml_sample_refs(doc, &self.project.name, &xml_patches)?;
                if xml_patched > 0 {
                    self.dirty = true;
                }
            }
        }

        self.refresh_missing_files();
        Ok(report)
    }

    /// Copies selected external samples into `<project_root>/Samples/Imported/` without moving or
    /// deleting originals (manual 5.7, LC-006), records clip updates in `"Collect All and Save"`,
    /// saves the `.als` if [`Self::set_path`] is `Some`, and refreshes [`Self::missing_files`].
    pub fn collect_all_and_save(
        &mut self,
        opts: &CollectAllOptions,
    ) -> Result<CollectReport, ProjectDirError> {
        let mut staged = self.project.clone();
        let report = livecraft_formats::project_dir::collect_all_and_save(
            &mut staged,
            self.als_doc.as_mut(),
            &self.project_context,
            opts,
        )?;

        let mut replacements: Vec<(String, String, Option<String>)> = Vec::new();
        for (before, after) in collect_audio_clips(&self.project)
            .into_iter()
            .zip(collect_audio_clips(&staged))
        {
            if before.file_path != after.file_path || before.relative_path != after.relative_path {
                let old_ref = if !before.file_path.trim().is_empty() {
                    before.file_path.clone()
                } else {
                    before.relative_path.clone().unwrap_or_default()
                };
                if !old_ref.is_empty()
                    && !replacements.iter().any(|(r, p, rel)| {
                        r == &old_ref && p == &after.file_path && rel == &after.relative_path
                    })
                {
                    replacements.push((
                        old_ref,
                        after.file_path.clone(),
                        after.relative_path.clone(),
                    ));
                }
            }
        }

        if !replacements.is_empty() {
            self.transaction("Collect All and Save", |tx| {
                for (old_ref, new_path, new_rel) in &replacements {
                    tx.run(ReplaceSampleReference::new(
                        old_ref,
                        new_path,
                        new_rel.clone(),
                    ))?;
                }
                Ok(())
            })
            .map_err(|e| ProjectDirError::InvalidPath(e.to_string()))?;
        } else if report.updated_references > 0 {
            self.dirty = true;
        }

        if let Some(set_path) = self.set_path.clone() {
            self.save(&set_path)?;
        } else {
            self.refresh_missing_files();
        }
        Ok(report)
    }

    /// Scans the enclosing Live Project for audio files unreferenced by any `.als`/`.alc`/`.adv`/`.adg`
    /// or the currently open Set (manual 5.8).
    pub fn find_unused_files(&self) -> Result<UnusedFilesReport, ProjectDirError> {
        let root = self
            .project_context
            .project_root
            .as_deref()
            .ok_or_else(|| {
                ProjectDirError::NotAProject(
                    self.project_context
                        .project_root
                        .clone()
                        .unwrap_or_default(),
                )
            })?;
        let mut report = scan_unused_files(root)?;
        let active_refs = collect_in_project_refs(&self.project, &self.project_context);
        if !active_refs.is_empty() {
            report
                .unused_files
                .retain(|entry| !active_refs.contains(&canonical_or_self(&entry.path)));
        }
        Ok(report)
    }

    /// Deletes a single verified-unused file inside the enclosing Live Project folder (manual 5.8,
    /// LC-006), rejecting `..`, symlinks, files outside `project_root`, `Ableton Project Info`,
    /// `Backup`, and any referenced file.
    pub fn delete_unused_file(&self, candidate: &Path) -> Result<(), ProjectDirError> {
        let root = self
            .project_context
            .project_root
            .as_deref()
            .filter(|p| is_project_root(p))
            .ok_or_else(|| {
                ProjectDirError::NotAProject(
                    self.project_context
                        .project_root
                        .clone()
                        .unwrap_or_default(),
                )
            })?;
        if candidate.as_os_str().is_empty()
            || candidate.to_string_lossy().contains('\0')
            || candidate
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(ProjectDirError::InvalidPath(format!(
                "unsafe candidate path: {}",
                candidate.display()
            )));
        }

        let target = if candidate.is_relative() {
            root.join(candidate)
        } else {
            candidate.to_path_buf()
        };
        let meta = fs::symlink_metadata(&target)?;
        if meta.is_symlink() || !meta.is_file() {
            return Err(ProjectDirError::WritePolicyDenied(format!(
                "refusing symbolic link or non-regular file: {}",
                target.display()
            )));
        }

        let canon_root = fs::canonicalize(root)?;
        let canon_target = fs::canonicalize(&target)?;
        if canon_target == canon_root || !canon_target.starts_with(&canon_root) {
            return Err(ProjectDirError::WritePolicyDenied(format!(
                "refusing to delete file outside project root: {}",
                candidate.display()
            )));
        }
        let rel = canon_target.strip_prefix(&canon_root).map_err(|_| {
            ProjectDirError::WritePolicyDenied(format!(
                "refusing to delete file outside project root: {}",
                candidate.display()
            ))
        })?;

        let mut walk = canon_root.clone();
        for (idx, comp) in rel.components().enumerate() {
            let Component::Normal(seg) = comp else {
                return Err(ProjectDirError::WritePolicyDenied(format!(
                    "unsafe path component in `{}`",
                    candidate.display()
                )));
            };
            if idx == 0 && (seg == PROJECT_INFO_DIR || seg == BACKUP_DIR) {
                return Err(ProjectDirError::WritePolicyDenied(format!(
                    "refusing to delete protected project directory file: {}",
                    candidate.display()
                )));
            }
            walk.push(seg);
            if fs::symlink_metadata(&walk).is_ok_and(|m| m.is_symlink()) {
                return Err(ProjectDirError::WritePolicyDenied(format!(
                    "refusing path traversing symbolic link: {}",
                    candidate.display()
                )));
            }
        }

        let report = self.find_unused_files()?;
        let is_verified_unused = report
            .unused_files
            .iter()
            .any(|entry| fs::canonicalize(&entry.path).ok().as_ref() == Some(&canon_target));
        if !is_verified_unused {
            return Err(ProjectDirError::WritePolicyDenied(format!(
                "refusing to delete referenced or non-unused file: {}",
                candidate.display()
            )));
        }

        fs::remove_file(&canon_target)?;
        Ok(())
    }

    /// Imports a Standard MIDI File (`.mid`) into a Session slot or Arrangement in one
    /// `"Import MIDI File"` undo transaction, creating a new MIDI track when `track_id` is `None`.
    pub fn import_midi_file(
        &mut self,
        track_id: Option<TrackId>,
        slot_idx: Option<usize>,
        start_beat: Option<f64>,
        mid_path: &Path,
    ) -> Result<ClipId, SessionFormatError> {
        let clip = read_smf_to_clip(mid_path)?;
        let (placement, _) = resolve_placement(
            &self.project,
            track_id,
            slot_idx,
            start_beat,
            TrackType::Midi,
        )?;

        let mut inserted_id: Option<ClipId> = None;
        self.transaction("Import MIDI File", |tx| {
            let target_track = match track_id {
                Some(tid) => tid,
                None => {
                    let name = if clip.name.trim().is_empty() {
                        "MIDI".to_owned()
                    } else {
                        clip.name.clone()
                    };
                    tx.run(CreateMidiTrack::new(name))?;
                    tx.project()
                        .tracks
                        .last()
                        .map(|t| t.id)
                        .ok_or(CommandError::Edit(EditError::NoSuchTrack(TrackId(0))))?
                }
            };
            inserted_id = Some(apply_clip_placement(tx, target_track, placement, clip)?);
            Ok(())
        })?;

        inserted_id.ok_or_else(|| {
            SessionFormatError::InvalidPlacement("imported MIDI clip id missing".to_owned())
        })
    }

    /// Exports a MIDI clip (in Session or Arrangement) as a Standard MIDI File (`.mid`).
    pub fn export_midi_clip(
        &self,
        clip_id: ClipId,
        out_path: &Path,
    ) -> Result<(), SessionFormatError> {
        let clip = find_clip_in_project(&self.project, clip_id)
            .ok_or(CommandError::NoSuchClip(clip_id))?;
        let bpm = self.project.transport.tempo.bpm_at(Beats(0.0));
        let time_sig = self.project.transport.time_signature();
        export_clip_to_smf(clip, out_path, bpm, time_sig, SmfFormat::Format0)?;
        Ok(())
    }

    /// Imports an Ableton Live Clip (`.alc`) into a Session slot or Arrangement in one
    /// `"Import Live Clip"` undo transaction.
    ///
    /// Obeys manual 5.3 (p. 134): onto an empty track (no clips and no devices) or empty Set
    /// space (`track_id` is `None`), restores the clip and its device chain; onto a non-empty
    /// track, restores the clip without its devices.
    pub fn import_live_clip(
        &mut self,
        track_id: Option<TrackId>,
        slot_idx: Option<usize>,
        start_beat: Option<f64>,
        alc_path: &Path,
    ) -> Result<ClipId, SessionFormatError> {
        let mut live_clip = load_live_clip(alc_path)?;
        let expected_type = match &live_clip.clip.payload {
            ClipPayload::Midi(_) => TrackType::Midi,
            ClipPayload::Audio(_) => TrackType::Audio,
        };
        if let ClipPayload::Audio(ref mut audio) = live_clip.clip.payload {
            let r = SampleReference {
                path: audio.file_path.clone(),
                relative_path: audio.relative_path.clone(),
            };
            if matches!(
                resolve_sample_reference(&r, &self.project_context),
                SampleResolution::Missing { .. }
            ) {
                let alc_ctx = ProjectContext::from_set_path(alc_path);
                if let SampleResolution::Resolved { resolved_path, .. } =
                    resolve_sample_reference(&r, &alc_ctx)
                {
                    let abs_resolved = std::path::absolute(&resolved_path).unwrap_or(resolved_path);
                    audio.file_path = normalize_stored_path(&abs_resolved);
                    audio.relative_path =
                        project_relative_path(&abs_resolved, &self.project_context);
                }
            }
        }

        let (placement, restore_devices) =
            resolve_placement(&self.project, track_id, slot_idx, start_beat, expected_type)?;

        let mut inserted_id: Option<ClipId> = None;
        let devices = live_clip.devices;
        let clip = live_clip.clip;
        self.transaction("Import Live Clip", |tx| {
            let target_track = match track_id {
                Some(tid) => tid,
                None => {
                    let name = if clip.name.trim().is_empty() {
                        "Clip".to_owned()
                    } else {
                        clip.name.clone()
                    };
                    match expected_type {
                        TrackType::Midi => tx.run(CreateMidiTrack::new(name))?,
                        TrackType::Audio => tx.run(CreateAudioTrack::new(name))?,
                        _ => {}
                    }
                    tx.project()
                        .tracks
                        .last()
                        .map(|t| t.id)
                        .ok_or(CommandError::Edit(EditError::NoSuchTrack(TrackId(0))))?
                }
            };
            inserted_id = Some(apply_clip_placement(tx, target_track, placement, clip)?);
            if restore_devices {
                for device in devices {
                    tx.run(InsertPreparedDevice::new(target_track, device))?;
                }
            }
            Ok(())
        })?;

        inserted_id.ok_or_else(|| {
            SessionFormatError::InvalidPlacement("imported Live Clip id missing".to_owned())
        })
    }

    /// Exports a clip and its track's device chain as an `.alc` Live Clip, honouring
    /// `preferences.library.collect_files_on_export` (manual 5.7.1, p. 152–153).
    pub fn export_live_clip(
        &self,
        track_id: TrackId,
        clip_id: ClipId,
        out_path: &Path,
        user_copy_choice: Option<bool>,
    ) -> Result<ExportClipOutcome, SessionFormatError> {
        let track = self
            .project
            .track(track_id)
            .ok_or(EditError::NoSuchTrack(track_id))?;
        let clip = find_clip_on_track(track, clip_id)
            .ok_or(CommandError::NoSuchClip(clip_id))?
            .clone();
        let live_clip = LiveClipFile {
            clip: clip.clone(),
            devices: track.device_chain.clone(),
        };
        let copy_audio = match &clip.payload {
            ClipPayload::Midi(_) => false,
            ClipPayload::Audio(_) => match self
                .preferences
                .library
                .collect_files_on_export
                .should_copy(user_copy_choice)
            {
                None => return Ok(ExportClipOutcome::NeedsCopyConfirmation),
                Some(choice) => choice,
            },
        };
        let dest_project_dir = if copy_audio {
            Some(resolve_export_copy_dir(out_path, &self.project_context))
        } else {
            None
        };
        let copied_samples = if let Some(ref copy_dir) = dest_project_dir
            && let ClipPayload::Audio(ref audio) = clip.payload
            && (!audio.file_path.trim().is_empty()
                || audio
                    .relative_path
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty()))
        {
            let r = SampleReference {
                path: audio.file_path.clone(),
                relative_path: audio.relative_path.clone(),
            };
            matches!(
                resolve_sample_reference(&r, &self.project_context),
                SampleResolution::Resolved { .. }
            ) || matches!(
                resolve_sample_reference(&r, &ProjectContext::for_project(copy_dir)),
                SampleResolution::Resolved { .. }
            ) || matches!(
                resolve_sample_reference(&r, &ProjectContext::default()),
                SampleResolution::Resolved { .. }
            )
        } else {
            false
        };
        save_live_clip_in_context(
            &live_clip,
            out_path,
            dest_project_dir.as_deref(),
            Some(&self.project_context),
        )?;
        Ok(ExportClipOutcome::Saved { copied_samples })
    }

    /// Saves one device on `track_id` as an Ableton Device Preset (`.adv`).
    pub fn save_device_preset(
        &self,
        track_id: TrackId,
        device_id: DeviceId,
        out_path: &Path,
    ) -> Result<(), SessionFormatError> {
        let track = self
            .project
            .track(track_id)
            .ok_or(EditError::NoSuchTrack(track_id))?;
        let device = track
            .device_chain
            .iter()
            .find(|d| d.id == device_id)
            .ok_or(EditError::NoSuchDevice {
                track: track_id,
                device: device_id,
            })?;
        write_device_preset(device, out_path)?;
        Ok(())
    }

    /// Loads an `.adv` device preset onto `track_id` in one undoable step.
    pub fn load_device_preset(
        &mut self,
        track_id: TrackId,
        adv_path: &Path,
    ) -> Result<DeviceId, SessionFormatError> {
        if self.project.track(track_id).is_none() {
            return Err(EditError::NoSuchTrack(track_id).into());
        }
        let device = read_device_preset(adv_path)?;
        let mut created_id: Option<DeviceId> = None;
        self.transaction("Load Device Preset", |tx| {
            tx.run(InsertPreparedDevice::new(track_id, device))?;
            created_id = tx
                .project()
                .track(track_id)
                .and_then(|t| t.device_chain.last())
                .map(|d| d.id);
            Ok(())
        })?;
        created_id.ok_or_else(|| EditError::NoSuchTrack(track_id).into())
    }

    /// Saves a track's device chain as an Ableton Rack Preset (`.adg`).
    pub fn save_rack_preset(
        &self,
        track_id: TrackId,
        name: &str,
        macros: [f32; 8],
        out_path: &Path,
    ) -> Result<(), SessionFormatError> {
        let track = self
            .project
            .track(track_id)
            .ok_or(EditError::NoSuchTrack(track_id))?;
        let preset = RackPreset {
            name: name.to_owned(),
            devices: track.device_chain.clone(),
            macros,
        };
        write_rack_preset(&preset, out_path)?;
        Ok(())
    }

    /// Loads all devices from an `.adg` rack preset onto `track_id` in one undoable transaction.
    pub fn load_rack_preset(
        &mut self,
        track_id: TrackId,
        adg_path: &Path,
    ) -> Result<Vec<DeviceId>, SessionFormatError> {
        if self.project.track(track_id).is_none() {
            return Err(EditError::NoSuchTrack(track_id).into());
        }
        let preset = read_rack_preset(adg_path)?;
        let mut inserted_ids = Vec::with_capacity(preset.devices.len());
        self.transaction("Load Rack Preset", |tx| {
            for device in preset.devices {
                tx.run(InsertPreparedDevice::new(track_id, device))?;
                if let Some(id) = tx
                    .project()
                    .track(track_id)
                    .and_then(|t| t.device_chain.last())
                    .map(|d| d.id)
                {
                    inserted_ids.push(id);
                }
            }
            Ok(())
        })?;
        Ok(inserted_ids)
    }

    /// Extracts an Ableton Live Pack (`.alp`) safely into `dest_dir` or the configured User Library.
    pub fn install_pack(
        &self,
        alp_path: &Path,
        dest_dir: Option<&Path>,
    ) -> Result<PackInstallReport, SessionFormatError> {
        let target = match dest_dir {
            Some(dir) => dir.to_path_buf(),
            None => self
                .preferences
                .library
                .effective_user_library()
                .ok_or(SessionFormatError::MissingUserLibrary)?,
        };
        Ok(extract_pack(alp_path, &target)?)
    }

    /// Runs a one-pass B11 health check across the open Set (missing files, unused files,
    /// missing plug-ins, sample-rate mismatches, and external files).
    pub fn health_report(
        &self,
        expected_sample_rate: Option<u32>,
    ) -> Result<ProjectHealthReport, HealthError> {
        let opts = HealthCheckOptions {
            set_path: self.set_path.clone(),
            project_root: self.project_context.project_root.clone(),
            expected_sample_rate,
            user_library: self.project_context.user_library.clone(),
            factory_packs_roots: self.project_context.factory_packs_roots.clone(),
            available_plugins: None,
        };
        match self.als_doc.as_ref() {
            Some(doc) if !self.dirty => check_document_health(doc, &opts),
            Some(doc) => {
                let mut synced = doc.clone();
                synced.apply_project(&self.project)?;
                check_document_health(&synced, &opts)
            }
            None => check_project_health(&self.project, &opts),
        }
    }

    /// Replaces the audio sample of `clip_id` with `new_sample` (manual 8.12).
    ///
    /// Preserves clip settings (`gain_db`, `pitch_cents`, `warp_mode`, `is_warped`, `ram_mode`,
    /// loop, and clip markers) via [`ReplaceClipSample`], retains warp markers only when the old
    /// sample exists on disk and has the exact same frame length as `new_sample`, builds
    /// `.lcanalysis` when `create_analysis_files` is enabled, and refreshes [`Self::missing_files`].
    pub fn replace_clip_sample(
        &mut self,
        clip_id: ClipId,
        new_sample: &Path,
    ) -> Result<(), SessionFormatError> {
        let old_frames = {
            let clip = find_clip_in_project(&self.project, clip_id)
                .ok_or(CommandError::NoSuchClip(clip_id))?;
            let ClipPayload::Audio(ref audio) = clip.payload else {
                return Err(CommandError::NotAudioClip(clip_id).into());
            };
            resolve_clip_sample_path(audio, &self.project_context)
                .and_then(|old_path| probe_audio_info(&old_path).ok())
                .and_then(|info| info.total_frames)
        };

        let new_info = probe_audio_info(new_sample)?;
        let keep_warp_markers = old_frames.is_some() && old_frames == new_info.total_frames;

        let abs_new = std::path::absolute(new_sample).unwrap_or_else(|_| new_sample.to_path_buf());
        let new_path = normalize_stored_path(&abs_new);
        let new_relative_path = project_relative_path(&abs_new, &self.project_context);

        self.execute(ReplaceClipSample::new(
            clip_id,
            new_path,
            new_relative_path,
            new_info.sample_rate,
            new_info.channels,
            keep_warp_markers,
        ))?;

        if self.preferences.library.create_analysis_files {
            let loc = self.preferences.library.analysis_cache_location();
            if let Err(err) = load_or_build_analysis(&abs_new, &loc, true) {
                log::warn!(
                    "Failed to build .lcanalysis cache for {}: {err}",
                    abs_new.display()
                );
            }
        }

        self.refresh_missing_files();
        Ok(())
    }

    /// Reloads an audio clip's sample after external modification on disk (manual 8.12):
    /// recomputes `.lcanalysis` (when enabled), updates `sample_rate` and `channels`, and refreshes
    /// the running audio engine graph.
    pub fn refresh_clip_sample(&mut self, clip_id: ClipId) -> Result<(), SessionFormatError> {
        let (resolved, is_ram_mode) = {
            let clip = find_clip_in_project(&self.project, clip_id)
                .ok_or(CommandError::NoSuchClip(clip_id))?;
            let ClipPayload::Audio(ref audio) = clip.payload else {
                return Err(CommandError::NotAudioClip(clip_id).into());
            };
            let path = resolve_clip_sample_path(audio, &self.project_context).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("sample file for clip {} could not be resolved", clip_id.0),
                )
            })?;
            (path, audio.ram_mode)
        };

        let info = probe_audio_info(&resolved)?;
        if self.preferences.library.create_analysis_files {
            let loc = self.preferences.library.analysis_cache_location();
            let cache_path = analysis_cache_path(&resolved, &loc);
            if cache_path.is_file() {
                let _ = fs::remove_file(&cache_path);
            }
            load_or_build_analysis(&resolved, &loc, true)?;
        } else {
            let _ = build_waveform_pyramid_streaming(&resolved)?;
        }

        let mut meta_changed = false;
        if let Ok(clip_ref) = clip_mut(&mut self.project, clip_id)
            && let ClipPayload::Audio(ref mut audio_mut) = clip_ref.payload
            && (audio_mut.sample_rate != info.sample_rate || audio_mut.channels != info.channels)
        {
            audio_mut.sample_rate = info.sample_rate;
            audio_mut.channels = info.channels;
            meta_changed = true;
        }
        if meta_changed {
            self.dirty = true;
        }

        if let Some(engine) = self.engine.as_mut() {
            if is_ram_mode {
                let target_rate = engine.sample_rate.max(1.0).round() as u32;
                engine
                    .compiler
                    .decode_queue()
                    .submit(&resolved, target_rate);
            } else {
                // GraphCompiler/ClipLane retains an existing StreamAudioPlayer when stream_key
                // (path + engine rate) matches, so submit an intermediate graph without this
                // clip's stream first so the audio thread drops the stale reader before opening
                // the updated file.
                let mut flush_project = self.project.clone();
                if let Ok(flush_clip) = clip_mut(&mut flush_project, clip_id)
                    && let ClipPayload::Audio(ref mut audio_mut) = flush_clip.payload
                {
                    audio_mut.file_path.clear();
                    audio_mut.relative_path = None;
                }
                let flush_graph = engine.compiler.compile(&flush_project);
                if let Err(err) = engine.handle.submit_graph(flush_graph) {
                    log::warn!("Failed to submit stream-flush graph on sample reload: {err}");
                }
            }
            let graph = engine.compiler.compile(&self.project);
            if let Err(err) = engine.handle.submit_graph(graph) {
                log::warn!("Failed to refresh audio graph after clip sample reload: {err}");
            }
        }
        self.sent = self.project.clone();
        self.refresh_missing_files();
        Ok(())
    }

    /// Resolves an audio clip's sample via [`ProjectContext`] and returns its cached or freshly
    /// computed `.lcanalysis` waveform pyramid (writing the sidecar only when
    /// `preferences.library.create_analysis_files` is `true`).
    #[must_use]
    pub fn sample_analysis(&self, clip: &Clip) -> Option<CachedSampleAnalysis> {
        let ClipPayload::Audio(ref audio) = clip.payload else {
            return None;
        };
        let resolved = resolve_clip_sample_path(audio, &self.project_context)?;
        let loc = self.preferences.library.analysis_cache_location();
        match load_or_build_analysis(
            &resolved,
            &loc,
            self.preferences.library.create_analysis_files,
        ) {
            Ok(analysis) => Some(analysis),
            Err(err) => {
                log::warn!(
                    "Failed to load or build .lcanalysis for {}: {err}",
                    resolved.display()
                );
                None
            }
        }
    }
}

fn apply_clip_placement(
    tx: &mut crate::history::Transaction<'_>,
    track: TrackId,
    placement: ClipPlacement,
    clip: Clip,
) -> Result<ClipId, CommandError> {
    match placement {
        ClipPlacement::Slot(slot) => {
            tx.run(InsertPreparedClip::new(track, slot, clip))?;
            tx.project()
                .track(track)
                .and_then(|t| t.clip_slots.get(slot))
                .and_then(|s| s.clip.as_ref())
                .map(|c| c.id)
                .ok_or(CommandError::SlotEmpty { track, slot })
        }
        ClipPlacement::Arrangement(start_beat) => {
            let length_beats = clip.length_beats;
            let arr = ArrangementClip::new(clip.id, clip, start_beat, length_beats);
            tx.run(InsertPreparedArrangementClip::new(track, arr))?;
            tx.project()
                .track(track)
                .and_then(|t| t.arrangement_clips.last())
                .map(|a| a.clip.id)
                .ok_or(CommandError::Edit(EditError::NoSuchTrack(track)))
        }
    }
}
