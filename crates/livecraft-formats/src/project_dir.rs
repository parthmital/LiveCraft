//! Live Project folder detection, sample resolution, Collect All and Save, and Unused Files.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use livecraft_model::model::{ClipPayload, Project};
use serde::{Deserialize, Serialize};

use crate::als::AlsError;
use crate::als::map::AlsDocument;
use crate::als::read::{decompress_als_or_xml, read_xml_tree};
use crate::als::tree::{XmlNodeId, XmlTree};
use crate::health::{
    all_project_clips_mut, build_xml_fileref_map, canonical_or_self, collect_project_clip_refs,
    copy_dedup, extract_xml_file_refs, file_name_from_ref, has_ext, path_is_inside, read_val,
    relative_forward_slash, resolve_candidate, walk_files,
};

pub const PROJECT_INFO_DIR: &str = "Ableton Project Info";
pub const SAMPLES_DIR: &str = "Samples";
pub const SAMPLES_RECORDED_DIR: &str = "Recorded";
pub const SAMPLES_IMPORTED_DIR: &str = "Imported";
pub const SAMPLES_PROCESSED_DIR: &str = "Processed";
pub const BACKUP_DIR: &str = "Backup";

const AUDIO_EXTS: &[&str] = &[
    "wav", "aif", "aiff", "aifc", "flac", "mp3", "ogg", "oga", "opus", "m4a", "aac",
];
const DOC_EXTS: &[&str] = &["als", "alc", "adv", "adg"];

#[derive(Debug, thiserror::Error)]
pub enum ProjectDirError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ableton XML error: {0}")]
    Als(#[from] AlsError),
    #[error("invalid Live Set or Project name: {0}")]
    InvalidProjectName(String),
    #[error("not a Live Project folder (missing `{PROJECT_INFO_DIR}`): {0}")]
    NotAProject(PathBuf),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("write policy denied (LC-006): {0}")]
    WritePolicyDenied(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDir {
    pub root: PathBuf,
}

impl ProjectDir {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ProjectDirError> {
        let r = root.as_ref();
        if !is_project_root(r) {
            return Err(ProjectDirError::NotAProject(r.to_path_buf()));
        }
        Ok(Self {
            root: r.to_path_buf(),
        })
    }
    #[must_use]
    pub fn samples_imported_dir(&self) -> PathBuf {
        self.root.join(SAMPLES_DIR).join(SAMPLES_IMPORTED_DIR)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSetLocation {
    pub project_dir: ProjectDir,
    pub set_path: PathBuf,
    pub created_new_project: bool,
}

#[must_use]
pub fn is_project_root(dir: &Path) -> bool {
    dir.is_dir() && dir.join(PROJECT_INFO_DIR).is_dir()
}

#[must_use]
pub fn detect_project_root(path: &Path) -> Option<PathBuf> {
    let anc = if path.is_relative() {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |c| c.join(path))
    } else {
        path.to_path_buf()
    };
    anc.ancestors()
        .find(|a| !a.as_os_str().is_empty() && is_project_root(a))
        .map(Path::to_path_buf)
}

pub fn create_project_layout(root: &Path) -> Result<ProjectDir, ProjectDirError> {
    if root.as_os_str().is_empty() {
        return Err(ProjectDirError::InvalidPath("empty root".into()));
    }
    fs::create_dir_all(root.join(PROJECT_INFO_DIR))?;
    for sub in [
        SAMPLES_RECORDED_DIR,
        SAMPLES_IMPORTED_DIR,
        SAMPLES_PROCESSED_DIR,
    ] {
        fs::create_dir_all(root.join(SAMPLES_DIR).join(sub))?;
    }
    Ok(ProjectDir {
        root: root.to_path_buf(),
    })
}

/// Saves `doc` into an enclosing Live Project or creates `<Name> Project/` when outside (5.5.1, LC-006).
pub fn save_set_into_project(
    doc: &AlsDocument,
    target: &Path,
) -> Result<SavedSetLocation, ProjectDirError> {
    if target.is_dir()
        || target
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(ProjectDirError::InvalidPath(
            "target must be a safe file path".into(),
        ));
    }
    let raw = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim();
    let stem = raw
        .strip_suffix(".als")
        .or_else(|| raw.strip_suffix(".ALS"))
        .unwrap_or(raw);
    if stem.is_empty() || raw.contains(['/', '\\', '\0']) || stem.contains("..") || stem == "." {
        return Err(ProjectDirError::InvalidProjectName(format!(
            "invalid `{raw}`"
        )));
    }
    let als_name = format!("{stem}.als");
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (project_dir, set_path, created_new_project) =
        if let Some(root) = detect_project_root(target) {
            let abs_parent = if parent.is_relative() {
                std::env::current_dir().map_or_else(|_| parent.to_path_buf(), |c| c.join(parent))
            } else {
                parent.to_path_buf()
            };
            if !path_is_inside(&abs_parent, &root) {
                return Err(ProjectDirError::WritePolicyDenied("outside root".into()));
            }
            fs::create_dir_all(parent)?;
            if !path_is_inside(parent, &root) {
                return Err(ProjectDirError::WritePolicyDenied("outside root".into()));
            }
            let dest_set = parent.join(&als_name);
            if fs::symlink_metadata(&dest_set).is_ok_and(|m| m.is_symlink())
                || !path_is_inside(&dest_set, &root)
            {
                return Err(ProjectDirError::WritePolicyDenied(
                    "destination escapes project root".into(),
                ));
            }
            (ProjectDir { root }, dest_set, false)
        } else {
            let candidate_root = parent.join(format!("{stem} Project"));
            if fs::symlink_metadata(&candidate_root).is_ok() {
                if is_project_root(&candidate_root) {
                    let set = candidate_root.join(&als_name);
                    if fs::symlink_metadata(&set).is_ok_and(|m| m.is_symlink()) {
                        return Err(ProjectDirError::WritePolicyDenied(
                            "destination is a symbolic link".into(),
                        ));
                    }
                    (
                        ProjectDir {
                            root: candidate_root,
                        },
                        set,
                        false,
                    )
                } else {
                    return Err(ProjectDirError::WritePolicyDenied(format!(
                        "destination `{}` exists and is not a Live Project",
                        candidate_root.display()
                    )));
                }
            } else {
                let dir = create_project_layout(&candidate_root)?;
                let set = dir.root.join(&als_name);
                (dir, set, true)
            }
        };
    doc.save_als(&set_path)?;
    Ok(SavedSetLocation {
        project_dir,
        set_path,
        created_new_project,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_root: Option<PathBuf>,
    pub set_dir: Option<PathBuf>,
    pub user_library: Option<PathBuf>,
    pub factory_packs_roots: Vec<PathBuf>,
}

impl ProjectContext {
    #[must_use]
    pub fn for_project(root: impl Into<PathBuf>) -> Self {
        let r = root.into();
        Self {
            set_dir: Some(r.clone()),
            project_root: Some(r),
            ..Self::default()
        }
    }
    #[must_use]
    pub fn from_set_path(set_path: &Path) -> Self {
        let dir = set_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        Self {
            project_root: detect_project_root(set_path),
            set_dir: dir,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SampleReference {
    pub path: String,
    pub relative_path: Option<String>,
}

impl SampleReference {
    #[must_use]
    pub fn from_path(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            relative_path: None,
        }
    }
    #[must_use]
    pub fn display_ref(&self) -> &str {
        if !self.path.trim().is_empty() {
            &self.path
        } else {
            self.relative_path.as_deref().unwrap_or("")
        }
    }
    #[must_use]
    pub fn file_name(&self) -> Option<String> {
        file_name_from_ref(&self.path)
            .or_else(|| self.relative_path.as_deref().and_then(file_name_from_ref))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipLocationRef {
    SessionSlot(u64, usize, String),
    Arrangement(u64, u64, String),
    XmlFileRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleLocation {
    InProject,
    UserLibrary,
    FactoryPacks,
    OtherProject(PathBuf),
    Elsewhere,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleResolution {
    Resolved {
        resolved_path: PathBuf,
        via_relative_path: bool,
        location: SampleLocation,
    },
    Missing {
        reference: SampleReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingFile {
    pub reference: SampleReference,
    pub file_name: String,
    pub usages: Vec<ClipLocationRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AutomaticSearchOptions {
    pub search_folder: Option<PathBuf>,
    pub search_project: bool,
    pub search_library: bool,
    pub library_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchCandidateOutcome {
    NoCandidate,
    OneCandidate(PathBuf),
    MultipleCandidates(Vec<PathBuf>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AutoRepairReport {
    pub repaired: Vec<(String, PathBuf)>,
    pub unresolved: Vec<(MissingFile, SearchCandidateOutcome)>,
}

/// Resolves a [`SampleReference`] strictly read-only (`Path` first, then `RelativePath`).
#[must_use]
pub fn resolve_sample_reference(
    reference: &SampleReference,
    ctx: &ProjectContext,
) -> SampleResolution {
    let pairs = [
        (&reference.path as &str, false),
        (reference.relative_path.as_deref().unwrap_or(""), true),
    ];
    for (raw, via_relative_path) in pairs {
        if !raw.trim().is_empty()
            && let Some(resolved_path) = resolve_candidate(raw, ctx)
        {
            let location = classify_sample_location(&resolved_path, ctx);
            return SampleResolution::Resolved {
                resolved_path,
                via_relative_path,
                location,
            };
        }
    }
    SampleResolution::Missing {
        reference: reference.clone(),
    }
}

#[must_use]
pub fn classify_sample_location(resolved: &Path, ctx: &ProjectContext) -> SampleLocation {
    if !resolved.is_file() {
        return SampleLocation::Missing;
    }
    if ctx
        .project_root
        .as_ref()
        .is_some_and(|r| path_is_inside(resolved, r))
    {
        return SampleLocation::InProject;
    }
    if ctx
        .factory_packs_roots
        .iter()
        .any(|p| path_is_inside(resolved, p))
    {
        return SampleLocation::FactoryPacks;
    }
    if let Some(other) = detect_project_root(resolved)
        && ctx
            .project_root
            .as_ref()
            .is_none_or(|c| canonical_or_self(c) != canonical_or_self(&other))
    {
        return SampleLocation::OtherProject(other);
    }
    if ctx
        .user_library
        .as_ref()
        .is_some_and(|l| path_is_inside(resolved, l))
    {
        return SampleLocation::UserLibrary;
    }
    SampleLocation::Elsewhere
}

#[must_use]
pub fn detect_missing_files(project: &Project, ctx: &ProjectContext) -> Vec<MissingFile> {
    collect_missing(project, None, ctx)
}

#[must_use]
pub fn detect_missing_files_in_document(
    doc: &AlsDocument,
    ctx: &ProjectContext,
) -> Vec<MissingFile> {
    collect_missing(doc.project(), Some(doc.tree()), ctx)
}

fn collect_missing(
    project: &Project,
    tree: Option<&XmlTree>,
    ctx: &ProjectContext,
) -> Vec<MissingFile> {
    let mut by_key: BTreeMap<String, MissingFile> = BTreeMap::new();
    let xml_rel = tree.map(build_xml_fileref_map).unwrap_or_default();
    let mut push = |r: SampleReference, u: ClipLocationRef| {
        if let SampleResolution::Missing { reference } = resolve_sample_reference(&r, ctx) {
            let name = reference.file_name().unwrap_or_default();
            by_key
                .entry(reference.display_ref().to_owned())
                .or_insert_with(|| MissingFile {
                    file_name: name,
                    reference,
                    usages: Vec::new(),
                })
                .usages
                .push(u);
        }
    };
    for (mut r, u) in collect_project_clip_refs(project) {
        if r.relative_path.is_none() {
            r.relative_path = xml_rel.get(&r.path).and_then(|x| x.relative_path.clone());
        }
        push(r, u);
    }
    if let Some(t) = tree {
        for (r, tag) in extract_xml_file_refs(t) {
            if tag != "AudioClip" {
                push(r, ClipLocationRef::XmlFileRef(tag));
            }
        }
    }
    by_key.into_values().collect()
}

pub fn automatic_search_file(
    missing: &MissingFile,
    opts: &AutomaticSearchOptions,
    ctx: &ProjectContext,
) -> Result<SearchCandidateOutcome, ProjectDirError> {
    let res = automatic_search(std::slice::from_ref(missing), opts, ctx)?;
    Ok(res
        .into_iter()
        .next()
        .map_or(SearchCandidateOutcome::NoCandidate, |(_, o)| o))
}

/// Searches enabled locations (`Search Folder`, `Search Project`, `Search Library`) for `missing`.
pub fn automatic_search(
    missing: &[MissingFile],
    opts: &AutomaticSearchOptions,
    ctx: &ProjectContext,
) -> Result<Vec<(MissingFile, SearchCandidateOutcome)>, ProjectDirError> {
    let mut roots: Vec<PathBuf> = opts.search_folder.iter().cloned().collect();
    if opts.search_project {
        roots.extend(ctx.project_root.iter().cloned());
    }
    if opts.search_library {
        let libs = if opts.library_roots.is_empty() {
            ctx.user_library.as_slice()
        } else {
            &opts.library_roots
        };
        roots.extend(libs.iter().cloned());
    }
    let (mut by_lower, mut seen) = (HashMap::<String, Vec<PathBuf>>::new(), HashSet::new());
    for root in roots.into_iter().filter(|r| r.is_dir()) {
        for f in walk_files(&root, true, false)? {
            if seen.insert(canonical_or_self(&f))
                && let Some(n) = f.file_name().and_then(|s| s.to_str())
            {
                by_lower.entry(n.to_ascii_lowercase()).or_default().push(f);
            }
        }
    }
    let mut out = Vec::with_capacity(missing.len());
    for item in missing {
        let mut cands = by_lower
            .get(&item.file_name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default();
        let exact: Vec<_> = cands
            .iter()
            .filter(|p| p.file_name().and_then(|s| s.to_str()) == Some(&item.file_name))
            .cloned()
            .collect();
        if !exact.is_empty() {
            cands = exact;
        }
        cands.sort();
        cands.dedup_by(|a, b| canonical_or_self(a) == canonical_or_self(b));
        let outcome = match cands.len() {
            0 => SearchCandidateOutcome::NoCandidate,
            1 => SearchCandidateOutcome::OneCandidate(cands.swap_remove(0)),
            _ => SearchCandidateOutcome::MultipleCandidates(cands),
        };
        out.push((item.clone(), outcome));
    }
    Ok(out)
}

fn normalize_stored_path(path: &Path) -> String {
    let raw = path.to_string_lossy().into_owned();
    if raw.starts_with(r"\\?\") {
        raw
    } else {
        raw.replace('\\', "/")
    }
}

fn set_fileref_child(tree: &mut XmlTree, fileref: XmlNodeId, tag: &str, val: &str) {
    if let Some(cid) = tree.ensure_child(fileref, tag) {
        if !tree.children(cid).is_empty() {
            tree.clear_children(cid);
        }
        tree.set_value_attr(cid, val);
    }
}

pub fn replace_sample_reference(project: &mut Project, old_path: &str, new_path: &str) -> usize {
    replace_sample_reference_with_rel(project, old_path, new_path, None)
}

pub fn replace_sample_reference_with_rel(
    project: &mut Project,
    old_ref: &str,
    new_path: &str,
    new_rel: Option<&str>,
) -> usize {
    if old_ref.trim().is_empty() {
        return 0;
    }
    let mut n = 0;
    for c in all_project_clips_mut(project) {
        if let ClipPayload::Audio(ref mut a) = c.payload
            && (a.file_path == old_ref || a.relative_path.as_deref() == Some(old_ref))
        {
            a.file_path = new_path.to_owned();
            a.relative_path = new_rel.map(str::to_owned);
            n += 1;
        }
    }
    n
}

pub fn replace_xml_sample_reference(
    tree: &mut XmlTree,
    old_ref: &str,
    new_path: &str,
    new_rel: Option<&str>,
) -> usize {
    if old_ref.trim().is_empty() {
        return 0;
    }
    let mut matching = Vec::new();
    let mut stack = vec![tree.root()];
    while let Some(nid) = stack.pop() {
        let Some(node) = tree.node(nid) else {
            continue;
        };
        if node.element_name() == Some("FileRef") {
            let path_val = read_val(tree, nid, "Path").unwrap_or_default();
            let rel_val = read_val(tree, nid, "RelativePath").unwrap_or_default();
            if path_val == old_ref || rel_val == old_ref {
                matching.push(nid);
            }
            continue;
        }
        for &c in node.children().iter().rev() {
            stack.push(c);
        }
    }
    for &nid in &matching {
        set_fileref_child(tree, nid, "Path", new_path);
        match new_rel {
            Some(rel) => {
                set_fileref_child(tree, nid, "RelativePathType", "3");
                set_fileref_child(tree, nid, "RelativePath", rel);
            }
            None => {
                if tree.child(nid, "RelativePath").is_some() {
                    set_fileref_child(tree, nid, "RelativePath", "");
                }
                if tree.child(nid, "RelativePathType").is_some() {
                    set_fileref_child(tree, nid, "RelativePathType", "0");
                }
            }
        }
    }
    matching.len()
}

pub fn repair_missing_files_auto(
    project: &mut Project,
    doc: Option<&mut AlsDocument>,
    ctx: &ProjectContext,
    opts: &AutomaticSearchOptions,
) -> Result<AutoRepairReport, ProjectDirError> {
    let missing = doc.as_deref().map_or_else(
        || detect_missing_files(project, ctx),
        |d| detect_missing_files_in_document(d, ctx),
    );
    let mut report = AutoRepairReport::default();
    let mut repairs = Vec::new();
    for (item, outcome) in automatic_search(&missing, opts, ctx)? {
        if let SearchCandidateOutcome::OneCandidate(found) = outcome {
            let rel_opt = ctx
                .project_root
                .as_deref()
                .filter(|r| path_is_inside(&found, r))
                .and_then(|r| relative_forward_slash(&found, r));
            let stored = normalize_stored_path(&found);
            replace_sample_reference_with_rel(
                project,
                &item.reference.path,
                &stored,
                rel_opt.as_deref(),
            );
            if let Some(ref rel) = item.reference.relative_path {
                replace_sample_reference_with_rel(project, rel, &stored, rel_opt.as_deref());
            }
            repairs.push((
                item.reference.path.clone(),
                item.reference.relative_path.clone(),
                stored,
                rel_opt,
            ));
            report
                .repaired
                .push((item.reference.display_ref().to_owned(), found));
        } else {
            report.unresolved.push((item, outcome));
        }
    }
    if let Some(d) = doc
        && !repairs.is_empty()
    {
        d.apply_project(project)?;
        let tree = d.tree_mut();
        for (old_path, old_rel, new_path, new_rel) in repairs {
            if !old_path.trim().is_empty() {
                replace_xml_sample_reference(tree, &old_path, &new_path, new_rel.as_deref());
            }
            if let Some(rel) = old_rel
                && rel != old_path
            {
                replace_xml_sample_reference(tree, &rel, &new_path, new_rel.as_deref());
            }
        }
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectAllOptions {
    pub collect_elsewhere: bool,
    pub collect_other_projects: bool,
    pub collect_user_library: bool,
    pub collect_factory_packs: bool,
}

impl Default for CollectAllOptions {
    fn default() -> Self {
        Self {
            collect_elsewhere: true,
            collect_other_projects: true,
            collect_user_library: false,
            collect_factory_packs: false,
        }
    }
}

impl CollectAllOptions {
    #[must_use]
    pub fn all() -> Self {
        Self {
            collect_elsewhere: true,
            collect_other_projects: true,
            collect_user_library: true,
            collect_factory_packs: true,
        }
    }
    #[must_use]
    pub fn should_collect(&self, loc: &SampleLocation) -> bool {
        match loc {
            SampleLocation::Elsewhere => self.collect_elsewhere,
            SampleLocation::OtherProject(_) => self.collect_other_projects,
            SampleLocation::UserLibrary => self.collect_user_library,
            SampleLocation::FactoryPacks => self.collect_factory_packs,
            SampleLocation::InProject | SampleLocation::Missing => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CollectFilesOnExport {
    #[default]
    Always,
    Ask,
    Never,
}

impl CollectFilesOnExport {
    #[must_use]
    pub fn should_copy(self, user_choice: Option<bool>) -> Option<bool> {
        match self {
            Self::Always => Some(true),
            Self::Never => Some(false),
            Self::Ask => user_choice,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectedFile {
    pub original_path: PathBuf,
    pub destination_path: PathBuf,
    pub project_relative_path: String,
    pub location: SampleLocation,
    pub reused_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CollectReport {
    pub collected: Vec<CollectedFile>,
    pub skipped_by_policy: Vec<(PathBuf, SampleLocation)>,
    pub missing: Vec<String>,
    pub updated_references: usize,
}

/// Copies selected external samples into `<project_root>/Samples/Imported/` without moving/deleting originals (5.7, LC-006).
pub fn collect_all_and_save(
    project: &mut Project,
    doc: Option<&mut AlsDocument>,
    ctx: &ProjectContext,
    opts: &CollectAllOptions,
) -> Result<CollectReport, ProjectDirError> {
    let root = ctx
        .project_root
        .as_deref()
        .filter(|p| is_project_root(p))
        .ok_or_else(|| {
            ProjectDirError::NotAProject(ctx.project_root.clone().unwrap_or_default())
        })?;
    let imported = root.join(SAMPLES_DIR).join(SAMPLES_IMPORTED_DIR);
    fs::create_dir_all(&imported)?;
    let xml_rel = doc
        .as_deref()
        .map(|d| build_xml_fileref_map(d.tree()))
        .unwrap_or_default();
    let (mut report, mut copied, mut skipped) = (
        CollectReport::default(),
        HashMap::<PathBuf, (String, String)>::new(),
        HashSet::new(),
    );

    for clip in all_project_clips_mut(project) {
        let ClipPayload::Audio(ref mut audio) = clip.payload else {
            continue;
        };
        if audio.file_path.trim().is_empty()
            && audio
                .relative_path
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
        {
            continue;
        }
        let mut r = SampleReference {
            path: audio.file_path.clone(),
            relative_path: audio.relative_path.clone(),
        };
        if r.relative_path.is_none() {
            r.relative_path = xml_rel
                .get(&audio.file_path)
                .and_then(|x| x.relative_path.clone());
        }
        let SampleResolution::Resolved {
            resolved_path,
            location,
            ..
        } = resolve_sample_reference(&r, ctx)
        else {
            let k = r.display_ref().to_owned();
            if !report.missing.contains(&k) {
                report.missing.push(k);
            }
            continue;
        };
        if location == SampleLocation::InProject {
            continue;
        }
        let canon = canonical_or_self(&resolved_path);
        if !opts.should_collect(&location) {
            if skipped.insert(canon) {
                report.skipped_by_policy.push((resolved_path, location));
            }
            continue;
        }
        let (dest_str, rel_str) = if let Some(pair) = copied.get(&canon) {
            pair.clone()
        } else {
            let (dest, rel, reused) = copy_dedup(&resolved_path, &imported, root)?;
            let dest_str = normalize_stored_path(&dest);
            copied.insert(canon, (dest_str.clone(), rel.clone()));
            report.collected.push(CollectedFile {
                original_path: resolved_path,
                destination_path: dest,
                project_relative_path: rel.clone(),
                location,
                reused_existing: reused,
            });
            (dest_str, rel)
        };
        if audio.file_path != dest_str || audio.relative_path.as_deref() != Some(rel_str.as_str()) {
            audio.file_path = dest_str;
            audio.relative_path = Some(rel_str);
            report.updated_references += 1;
        }
    }
    if let Some(d) = doc {
        d.apply_project(project)?;
        let xml_refs = extract_xml_file_refs(d.tree());
        for (r, tag) in xml_refs {
            if tag == "AudioClip" {
                continue;
            }
            let SampleResolution::Resolved {
                resolved_path,
                location,
                ..
            } = resolve_sample_reference(&r, ctx)
            else {
                let k = r.display_ref().to_owned();
                if !k.is_empty() && !report.missing.contains(&k) {
                    report.missing.push(k);
                }
                continue;
            };
            if location == SampleLocation::InProject {
                continue;
            }
            let canon = canonical_or_self(&resolved_path);
            if !opts.should_collect(&location) {
                if skipped.insert(canon) {
                    report.skipped_by_policy.push((resolved_path, location));
                }
                continue;
            }
            let (dest_str, rel_str) = if let Some(pair) = copied.get(&canon) {
                pair.clone()
            } else {
                let (dest, rel, reused) = copy_dedup(&resolved_path, &imported, root)?;
                let dest_str = normalize_stored_path(&dest);
                copied.insert(canon, (dest_str.clone(), rel.clone()));
                report.collected.push(CollectedFile {
                    original_path: resolved_path,
                    destination_path: dest,
                    project_relative_path: rel.clone(),
                    location,
                    reused_existing: reused,
                });
                (dest_str, rel)
            };
            let old_ref = r.display_ref();
            let patched =
                replace_xml_sample_reference(d.tree_mut(), old_ref, &dest_str, Some(&rel_str));
            report.updated_references += patched;
        }
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UnusedFileCategory {
    Recorded,
    Imported,
    Processed,
    Freeze,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedFileEntry {
    pub path: PathBuf,
    pub relative_path: String,
    pub category: UnusedFileCategory,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnusedFilesReport {
    pub project_root: PathBuf,
    pub scanned_documents: Vec<PathBuf>,
    pub unused_files: Vec<UnusedFileEntry>,
}

/// Scans `.als`, `.alc`, `.adv`, and `.adg` inside `project_root` for `<FileRef><Path|RelativePath>` (manual 5.8).
pub fn find_unused_files(project_root: &Path) -> Result<UnusedFilesReport, ProjectDirError> {
    if !is_project_root(project_root) {
        return Err(ProjectDirError::NotAProject(project_root.to_path_buf()));
    }
    let mut files = walk_files(project_root, true, true)?;
    files.sort();
    let (mut scanned_documents, mut samples, mut referenced) =
        (Vec::new(), Vec::new(), HashSet::new());
    for f in files {
        if has_ext(&f, DOC_EXTS) {
            scanned_documents.push(f);
        } else if has_ext(&f, AUDIO_EXTS) {
            samples.push(f);
        }
    }
    for doc_path in &scanned_documents {
        let tree = read_xml_tree(&decompress_als_or_xml(doc_path)?)?;
        let dir = doc_path.parent().map(Path::to_path_buf);
        let ctx = ProjectContext {
            project_root: Some(project_root.to_path_buf()),
            set_dir: dir,
            ..ProjectContext::default()
        };
        for (r, _) in extract_xml_file_refs(&tree) {
            if let SampleResolution::Resolved { resolved_path, .. } =
                resolve_sample_reference(&r, &ctx)
                && path_is_inside(&resolved_path, project_root)
            {
                referenced.insert(canonical_or_self(&resolved_path));
            }
        }
    }
    let mut unused_files = Vec::new();
    for path in samples {
        if referenced.contains(&canonical_or_self(&path)) {
            continue;
        }
        let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let rel = relative_forward_slash(&path, project_root)
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let l = rel.to_ascii_lowercase();
        let category = if l.starts_with("samples/recorded/") {
            UnusedFileCategory::Recorded
        } else if l.starts_with("samples/imported/") {
            UnusedFileCategory::Imported
        } else if l.starts_with("samples/freeze/") || l.starts_with("samples/processed/freeze/") {
            UnusedFileCategory::Freeze
        } else if l.starts_with("samples/processed/") {
            UnusedFileCategory::Processed
        } else {
            UnusedFileCategory::Other
        };
        unused_files.push(UnusedFileEntry {
            category,
            path,
            relative_path: rel,
            size_bytes,
        });
    }
    unused_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(UnusedFilesReport {
        project_root: project_root.to_path_buf(),
        scanned_documents,
        unused_files,
    })
}
