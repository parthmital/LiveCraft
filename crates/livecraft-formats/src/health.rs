//! Project health report (B11): missing files, unused files, missing plug-ins, sample-rate mismatches.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use livecraft_model::model::{Clip, ClipPayload, DeviceType, Project};
use serde::{Deserialize, Serialize};

use crate::als::AlsError;
use crate::als::map::AlsDocument;
use crate::als::tree::{XmlNodeId, XmlTree};
use crate::project_dir::{
    BACKUP_DIR, ClipLocationRef, MissingFile, PROJECT_INFO_DIR, ProjectContext, ProjectDirError,
    SAMPLES_DIR, SAMPLES_IMPORTED_DIR, SampleLocation, SampleReference, SampleResolution,
    UnusedFileEntry, detect_project_root, find_unused_files, is_project_root,
    resolve_sample_reference,
};

/// Errors raised while probing audio headers or computing a [`ProjectHealthReport`].
#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ableton XML error: {0}")]
    Als(#[from] AlsError),
    #[error("project directory error: {0}")]
    ProjectDir(#[from] ProjectDirError),
    #[error("failed to probe audio sample rate for {path}: {reason}")]
    AudioProbe { path: PathBuf, reason: String },
}

/// A `DeviceType::Custom` plug-in or unavailable device detected on a track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingPlugin {
    pub track_id: u64,
    pub track_name: String,
    pub device_id: u64,
    pub device_name: String,
    pub plugin_type: String,
}

/// An audio file whose on-disk sample rate differs from the expected Set/clip sample rate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleRateMismatch {
    pub file_path: PathBuf,
    pub reference: String,
    pub expected_sample_rate: u32,
    pub actual_sample_rate: u32,
    pub usages: Vec<ClipLocationRef>,
}

/// A referenced sample residing outside the enclosing Live Project folder (manual 5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalFile {
    pub file_path: PathBuf,
    pub reference: String,
    pub location: SampleLocation,
    pub size_bytes: u64,
    pub usages: Vec<ClipLocationRef>,
}

/// Configuration for running a one-pass [`ProjectHealthReport`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HealthCheckOptions {
    pub set_path: Option<PathBuf>,
    pub project_root: Option<PathBuf>,
    pub expected_sample_rate: Option<u32>,
    pub user_library: Option<PathBuf>,
    pub factory_packs_roots: Vec<PathBuf>,
    pub available_plugins: Option<Vec<String>>,
}

/// One-pass Project Health Report (B11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectHealthReport {
    pub set_name: String,
    pub set_path: Option<PathBuf>,
    pub project_root: Option<PathBuf>,
    pub missing_files: Vec<MissingFile>,
    pub unused_files: Vec<UnusedFileEntry>,
    pub missing_plugins: Vec<MissingPlugin>,
    pub sample_rate_mismatches: Vec<SampleRateMismatch>,
    pub external_files: Vec<ExternalFile>,
}

impl ProjectHealthReport {
    /// Returns `true` when all five diagnostic lists are empty.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.missing_files.is_empty()
            && self.unused_files.is_empty()
            && self.missing_plugins.is_empty()
            && self.sample_rate_mismatches.is_empty()
            && self.external_files.is_empty()
    }

    /// Total count of reported findings across all five categories.
    #[must_use]
    pub fn issue_count(&self) -> usize {
        self.missing_files.len()
            + self.unused_files.len()
            + self.missing_plugins.len()
            + self.sample_rate_mismatches.len()
            + self.external_files.len()
    }

    /// Formats a human-readable multi-section summary of this health report.
    #[must_use]
    pub fn format_health_report(&self) -> String {
        format_health_report(self)
    }
}

/// Opens a `.als` Set on disk and produces its one-pass [`ProjectHealthReport`].
pub fn inspect_set_health(
    set_path: &Path,
    options: &HealthCheckOptions,
) -> Result<ProjectHealthReport, HealthError> {
    let doc = AlsDocument::open(set_path)?;
    let mut opts = options.clone();
    if opts.set_path.is_none() {
        opts.set_path = Some(set_path.to_path_buf());
    }
    check_document_health(&doc, &opts)
}

/// Inspects an already-loaded [`AlsDocument`] (model + XML `<FileRef>` nodes) in one pass.
pub fn check_document_health(
    doc: &AlsDocument,
    options: &HealthCheckOptions,
) -> Result<ProjectHealthReport, HealthError> {
    run_health_check(doc.project(), Some(doc.tree()), options)
}

/// Inspects an in-memory [`Project`] in one pass.
pub fn check_project_health(
    project: &Project,
    options: &HealthCheckOptions,
) -> Result<ProjectHealthReport, HealthError> {
    run_health_check(project, None, options)
}

/// Formats a [`ProjectHealthReport`] as a concise human-readable summary.
#[must_use]
pub fn format_health_report(report: &ProjectHealthReport) -> String {
    let mut out = String::new();
    let status = if report.is_healthy() {
        "HEALTHY"
    } else {
        "ISSUES FOUND"
    };
    let _ = writeln!(out, "Project Health Report: {} [{status}]", report.set_name);
    if let Some(ref root) = report.project_root {
        let _ = writeln!(out, "Project root: {}", root.display());
    }
    if report.is_healthy() {
        out.push_str("All checks passed (0 missing files, 0 unused files, 0 missing plug-ins, 0 sample-rate mismatches, 0 external files).");
        return out;
    }
    let _ = writeln!(out, "Missing files ({}):", report.missing_files.len());
    for m in &report.missing_files {
        let _ = writeln!(
            out,
            "  - {} ({} usage(s))",
            m.reference.display_ref(),
            m.usages.len()
        );
    }
    let _ = writeln!(out, "Unused files ({}):", report.unused_files.len());
    for u in &report.unused_files {
        let _ = writeln!(
            out,
            "  - {} ({:?}, {} bytes)",
            u.relative_path, u.category, u.size_bytes
        );
    }
    let _ = writeln!(out, "Missing plug-ins ({}):", report.missing_plugins.len());
    for p in &report.missing_plugins {
        let _ = writeln!(
            out,
            "  - [{}] {} ({})",
            p.track_name, p.device_name, p.plugin_type
        );
    }
    let _ = writeln!(
        out,
        "Sample-rate mismatches ({}):",
        report.sample_rate_mismatches.len()
    );
    for s in &report.sample_rate_mismatches {
        let _ = writeln!(
            out,
            "  - {}: file {} Hz != expected {} Hz",
            s.reference, s.actual_sample_rate, s.expected_sample_rate
        );
    }
    let _ = writeln!(out, "External files ({}):", report.external_files.len());
    for e in &report.external_files {
        let _ = writeln!(
            out,
            "  - {} ({:?}, {} bytes)",
            e.file_path.display(),
            e.location,
            e.size_bytes
        );
    }
    out.trim_end().to_owned()
}

/// Strictly read-only sample-rate probe supporting WAV (`hound`), AIFF/AIFF-C, FLAC, and MP3 headers.
pub fn probe_audio_sample_rate(path: &Path) -> Result<u32, HealthError> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut hdr = [0u8; 12];
    if file.read(&mut hdr)? < 4 {
        return Err(HealthError::AudioProbe {
            path: path.to_path_buf(),
            reason: "file too short".into(),
        });
    }
    if &hdr[0..4] == b"RIFF" && &hdr[8..12] == b"WAVE" {
        let reader = hound::WavReader::open(path).map_err(|e| HealthError::AudioProbe {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        let sr = reader.spec().sample_rate;
        return if sr > 0 {
            Ok(sr)
        } else {
            Err(HealthError::AudioProbe {
                path: path.to_path_buf(),
                reason: "zero WAV sample rate".into(),
            })
        };
    }
    if &hdr[0..4] == b"FORM" && (&hdr[8..12] == b"AIFF" || &hdr[8..12] == b"AIFC") {
        return probe_aiff_rate(&mut file, file_len, path);
    }
    if &hdr[0..4] == b"fLaC" {
        return probe_flac_rate(&mut file, path);
    }
    let is_mp3 = &hdr[0..3] == b"ID3"
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    if is_mp3 {
        return probe_mp3_rate(&mut file, &hdr, path);
    }
    Err(HealthError::AudioProbe {
        path: path.to_path_buf(),
        reason: "unrecognized audio container".into(),
    })
}

fn run_health_check(
    project: &Project,
    tree: Option<&XmlTree>,
    options: &HealthCheckOptions,
) -> Result<ProjectHealthReport, HealthError> {
    let set_dir = options
        .set_path
        .as_deref()
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    let project_root = options
        .project_root
        .clone()
        .or_else(|| options.set_path.as_deref().and_then(detect_project_root));
    let ctx = ProjectContext {
        project_root: project_root.clone(),
        set_dir,
        user_library: options.user_library.clone(),
        factory_packs_roots: options.factory_packs_roots.clone(),
    };

    let mut missing_plugins = Vec::new();
    for track in project.all_tracks() {
        for dev in &track.device_chain {
            if let DeviceType::Custom(ref tag) = dev.device_type
                && !is_plugin_available(tag, &dev.name, options.available_plugins.as_deref())
            {
                missing_plugins.push(MissingPlugin {
                    track_id: track.id.0,
                    track_name: track.name.clone(),
                    device_id: dev.id.0,
                    device_name: dev.name.clone(),
                    plugin_type: tag.clone(),
                });
            }
        }
    }

    let xml_rel = tree.map(build_xml_fileref_map).unwrap_or_default();
    let mut acc = HealthAcc::default();
    for (mut r, loc, sr) in collect_project_clip_refs_with_rate(project) {
        r.relative_path = xml_rel.get(&r.path).and_then(|x| x.relative_path.clone());
        acc.check(&r, loc, options.expected_sample_rate.unwrap_or(sr), &ctx);
    }
    if let Some(t) = tree {
        for (r, tag, sr_opt) in extract_xml_file_refs_with_rate(t) {
            if tag != "AudioClip" {
                let exp_sr = options.expected_sample_rate.or(sr_opt).unwrap_or(0);
                acc.check(&r, ClipLocationRef::XmlFileRef(tag), exp_sr, &ctx);
            }
        }
    }

    let unused_files = match project_root.as_deref().filter(|p| is_project_root(p)) {
        Some(root) => find_unused_files(root)?
            .unused_files
            .into_iter()
            .filter(|u| !acc.in_project_refs.contains(&canonical_or_self(&u.path)))
            .collect(),
        None => Vec::new(),
    };

    Ok(ProjectHealthReport {
        set_name: project.name.clone(),
        set_path: options.set_path.clone(),
        project_root,
        missing_files: acc.missing.into_values().collect(),
        unused_files,
        missing_plugins,
        sample_rate_mismatches: acc.mismatches.into_values().collect(),
        external_files: acc.external.into_values().collect(),
    })
}

#[derive(Default)]
struct HealthAcc {
    missing: BTreeMap<String, MissingFile>,
    external: BTreeMap<PathBuf, ExternalFile>,
    mismatches: BTreeMap<(PathBuf, u32), SampleRateMismatch>,
    probe_cache: HashMap<PathBuf, (Option<u32>, u64)>,
    in_project_refs: HashSet<PathBuf>,
}

impl HealthAcc {
    fn check(
        &mut self,
        r: &SampleReference,
        usage: ClipLocationRef,
        expected_sr: u32,
        ctx: &ProjectContext,
    ) {
        let SampleResolution::Resolved {
            resolved_path,
            location,
            ..
        } = resolve_sample_reference(r, ctx)
        else {
            self.missing
                .entry(r.display_ref().to_owned())
                .or_insert_with(|| MissingFile {
                    file_name: r.file_name().unwrap_or_default(),
                    reference: r.clone(),
                    usages: Vec::new(),
                })
                .usages
                .push(usage);
            return;
        };
        let canon = canonical_or_self(&resolved_path);
        let (probed_sr, size_bytes) = *self.probe_cache.entry(canon.clone()).or_insert_with(|| {
            let size = fs::metadata(&resolved_path).map(|m| m.len()).unwrap_or(0);
            (probe_audio_sample_rate(&resolved_path).ok(), size)
        });
        if location == SampleLocation::InProject {
            self.in_project_refs.insert(canon.clone());
        } else if ctx.project_root.is_some() {
            self.external
                .entry(canon.clone())
                .or_insert_with(|| ExternalFile {
                    file_path: resolved_path.clone(),
                    reference: r.display_ref().to_owned(),
                    location,
                    size_bytes,
                    usages: Vec::new(),
                })
                .usages
                .push(usage.clone());
        }
        if expected_sr > 0
            && let Some(actual_sr) = probed_sr
            && actual_sr != expected_sr
        {
            self.mismatches
                .entry((canon, expected_sr))
                .or_insert_with(|| SampleRateMismatch {
                    file_path: resolved_path,
                    reference: r.display_ref().to_owned(),
                    expected_sample_rate: expected_sr,
                    actual_sample_rate: actual_sr,
                    usages: Vec::new(),
                })
                .usages
                .push(usage);
        }
    }
}

fn is_plugin_available(tag: &str, name: &str, available: Option<&[String]>) -> bool {
    let Some(list) = available else { return false };
    list.iter()
        .any(|p| p.eq_ignore_ascii_case(tag) || p.eq_ignore_ascii_case(name))
}

fn probe_aiff_rate(file: &mut File, file_len: u64, path: &Path) -> Result<u32, HealthError> {
    file.seek(SeekFrom::Start(12))?;
    let mut pos = 12u64;
    while pos + 8 <= file_len {
        let mut ch = [0u8; 8];
        if file.read_exact(&mut ch).is_err() {
            break;
        }
        let size = u64::from(u32::from_be_bytes([ch[4], ch[5], ch[6], ch[7]]));
        pos += 8;
        if &ch[0..4] == b"COMM" && size >= 18 && pos + 18 <= file_len {
            let mut comm = [0u8; 18];
            file.read_exact(&mut comm)?;
            let ext: [u8; 10] = comm[8..18]
                .try_into()
                .map_err(|_| HealthError::AudioProbe {
                    path: path.to_path_buf(),
                    reason: "invalid AIFF COMM header".into(),
                })?;
            return decode_ieee_extended_80_be(ext).ok_or_else(|| HealthError::AudioProbe {
                path: path.to_path_buf(),
                reason: "invalid AIFF COMM sample rate".into(),
            });
        }
        let step = size.saturating_add(size & 1);
        pos = pos.saturating_add(step);
        if pos > file_len {
            break;
        }
        file.seek(SeekFrom::Start(pos))?;
    }
    Err(HealthError::AudioProbe {
        path: path.to_path_buf(),
        reason: "AIFF COMM chunk not found".into(),
    })
}

fn decode_ieee_extended_80_be(b: [u8; 10]) -> Option<u32> {
    let se = u16::from_be_bytes([b[0], b[1]]);
    if (se & 0x8000) != 0 {
        return None;
    }
    let exp = i32::from(se & 0x7FFF);
    let mant = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    if exp == 0 || exp == 0x7FFF || mant == 0 {
        return None;
    }
    let val = (mant as f64) * 2.0_f64.powi(exp - 16383 - 63);
    (val.is_finite() && val >= 1.0 && val <= f64::from(u32::MAX)).then_some(val.round() as u32)
}

fn probe_flac_rate(file: &mut File, path: &Path) -> Result<u32, HealthError> {
    file.seek(SeekFrom::Start(4))?;
    let mut bh = [0u8; 4];
    file.read_exact(&mut bh)?;
    let btype = bh[0] & 0x7F;
    let blen = (u32::from(bh[1]) << 16) | (u32::from(bh[2]) << 8) | u32::from(bh[3]);
    if btype != 0 || blen < 34 {
        return Err(HealthError::AudioProbe {
            path: path.to_path_buf(),
            reason: "missing FLAC STREAMINFO".into(),
        });
    }
    let mut si = [0u8; 34];
    file.read_exact(&mut si)?;
    let sr = (u32::from(si[10]) << 12) | (u32::from(si[11]) << 4) | (u32::from(si[12]) >> 4);
    (sr > 0)
        .then_some(sr)
        .ok_or_else(|| HealthError::AudioProbe {
            path: path.to_path_buf(),
            reason: "zero FLAC sample rate".into(),
        })
}

fn probe_mp3_rate(file: &mut File, hdr: &[u8; 12], path: &Path) -> Result<u32, HealthError> {
    let start = if &hdr[0..3] == b"ID3" {
        let sz = ((u64::from(hdr[6] & 0x7F)) << 21)
            | ((u64::from(hdr[7] & 0x7F)) << 14)
            | ((u64::from(hdr[8] & 0x7F)) << 7)
            | u64::from(hdr[9] & 0x7F);
        10 + sz + if (hdr[5] & 0x10) != 0 { 10 } else { 0 }
    } else {
        0
    };
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; 65_536];
    let n = file.read(&mut buf)?;
    for w in buf[..n].windows(4) {
        if w[0] == 0xFF && (w[1] & 0xE0) == 0xE0 {
            let (ver, layer, br, sr_idx) = (
                (w[1] >> 3) & 3,
                (w[1] >> 1) & 3,
                (w[2] >> 4) & 15,
                ((w[2] >> 2) & 3) as usize,
            );
            if ver != 1 && layer != 0 && br != 15 && sr_idx < 3 {
                let rates = match ver {
                    3 => [44100, 48000, 32000],
                    2 => [22050, 24000, 16000],
                    _ => [11025, 12000, 8000],
                };
                return Ok(rates[sr_idx]);
            }
        }
    }
    Err(HealthError::AudioProbe {
        path: path.to_path_buf(),
        reason: "MP3 frame header not found".into(),
    })
}

// ---------------------------------------------------------------------------
// Shared crate-internal helpers (used by both `project_dir` and `health`)
// ---------------------------------------------------------------------------

pub(crate) fn collect_project_clip_refs(
    project: &Project,
) -> Vec<(SampleReference, ClipLocationRef)> {
    collect_project_clip_refs_with_rate(project)
        .into_iter()
        .map(|(r, loc, _)| (r, loc))
        .collect()
}

fn collect_project_clip_refs_with_rate(
    project: &Project,
) -> Vec<(SampleReference, ClipLocationRef, u32)> {
    let mut out = Vec::new();
    for t in project.all_tracks() {
        for (i, s) in t.clip_slots.iter().enumerate() {
            if let Some(ref c) = s.clip
                && let ClipPayload::Audio(ref a) = c.payload
                && !a.file_path.trim().is_empty()
            {
                let loc = ClipLocationRef::SessionSlot(t.id.0, i, c.name.clone());
                out.push((SampleReference::from_path(&a.file_path), loc, a.sample_rate));
            }
        }
        for arr in &t.arrangement_clips {
            if let ClipPayload::Audio(ref a) = arr.clip.payload
                && !a.file_path.trim().is_empty()
            {
                let loc =
                    ClipLocationRef::Arrangement(t.id.0, arr.clip.id.0, arr.clip.name.clone());
                out.push((SampleReference::from_path(&a.file_path), loc, a.sample_rate));
            }
        }
    }
    out
}

pub(crate) fn extract_xml_file_refs(tree: &XmlTree) -> Vec<(SampleReference, String)> {
    extract_xml_file_refs_with_rate(tree)
        .into_iter()
        .map(|(r, t, _)| (r, t))
        .collect()
}

pub(crate) fn extract_xml_file_refs_with_rate(
    tree: &XmlTree,
) -> Vec<(SampleReference, String, Option<u32>)> {
    let (mut out, mut stack) = (Vec::new(), vec![(tree.root(), "", None::<XmlNodeId>)]);
    while let Some((nid, ptag, sref_id)) = stack.pop() {
        let Some(node) = tree.node(nid) else { continue };
        let Some(tag) = node.element_name() else {
            continue;
        };
        if tag == "FileRef" {
            let r = SampleReference {
                path: read_val(tree, nid, "Path").unwrap_or_default(),
                relative_path: read_val(tree, nid, "RelativePath").filter(|s| !s.trim().is_empty()),
            };
            if !r.path.trim().is_empty() || r.relative_path.is_some() {
                let sr = sref_id
                    .and_then(|s| read_val(tree, s, "DefaultSampleRate"))
                    .and_then(|s| s.parse::<u32>().ok());
                let ctx_tag = if ptag.is_empty() { "FileRef" } else { ptag }.to_owned();
                out.push((r, ctx_tag, sr));
            }
            continue;
        }
        let next_sref = if tag == "SampleRef" {
            Some(nid)
        } else {
            sref_id
        };
        let next_tag = if matches!(tag, "SampleRef" | "FileRef") {
            ptag
        } else {
            tag
        };
        for &c in node.children().iter().rev() {
            stack.push((c, next_tag, next_sref));
        }
    }
    out
}

pub(crate) fn build_xml_fileref_map(tree: &XmlTree) -> HashMap<String, SampleReference> {
    let mut map = HashMap::new();
    for (r, _) in extract_xml_file_refs(tree) {
        if !r.path.trim().is_empty() && r.relative_path.is_some() {
            map.entry(r.path.clone()).or_insert(r);
        }
    }
    map
}

pub(crate) fn path_is_inside(candidate: &Path, dir: &Path) -> bool {
    canonical_or_self(candidate).starts_with(canonical_or_self(dir))
}

pub(crate) fn canonical_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        path.components().fold(PathBuf::new(), |mut out, c| {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    let _ = out.pop();
                }
                _ => out.push(c),
            }
            out
        })
    })
}

pub(crate) fn all_project_clips_mut(project: &mut Project) -> impl Iterator<Item = &mut Clip> {
    project.all_tracks_mut().flat_map(|t| {
        t.clip_slots
            .iter_mut()
            .filter_map(|s| s.clip.as_mut())
            .chain(t.arrangement_clips.iter_mut().map(|a| &mut a.clip))
    })
}

pub(crate) fn read_val(tree: &XmlTree, parent: XmlNodeId, child: &str) -> Option<String> {
    let cid = tree.child(parent, child)?;
    tree.value_attr(cid).map(str::to_owned).or_else(|| {
        let t = tree.text_content(cid);
        (!t.trim().is_empty()).then(|| t.trim().to_owned())
    })
}

pub(crate) fn resolve_candidate(raw: &str, ctx: &ProjectContext) -> Option<PathBuf> {
    let b = raw.as_bytes();
    let win =
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    if Path::new(raw).is_absolute() || raw.starts_with('/') || raw.starts_with("\\\\") || win {
        return Path::new(raw).is_file().then(|| PathBuf::from(raw));
    }
    let join = |base: &Path| {
        raw.split(['/', '\\'])
            .filter(|s| !s.is_empty() && *s != ".")
            .fold(base.to_path_buf(), |p, s| p.join(s))
    };
    for base in [ctx.project_root.as_deref(), ctx.set_dir.as_deref()]
        .into_iter()
        .flatten()
    {
        let c = join(base);
        if c.is_file() {
            return Some(c);
        }
    }
    (ctx.project_root.is_none() && ctx.set_dir.is_none())
        .then(|| join(Path::new(".")))
        .filter(|c| c.is_file())
}

pub(crate) fn file_name_from_ref(raw: &str) -> Option<String> {
    let n = raw.trim().rsplit(['/', '\\']).next()?.trim();
    (!n.is_empty() && !matches!(n, "." | "..")).then(|| n.to_owned())
}

pub(crate) fn walk_files(
    root: &Path,
    skip_info: bool,
    skip_backup: bool,
) -> Result<Vec<PathBuf>, ProjectDirError> {
    let (mut visited, mut stack, mut out) = (HashSet::new(), vec![root.to_path_buf()], Vec::new());
    while let Some(dir) = stack.pop() {
        if !visited.insert(canonical_or_self(&dir)) {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let (path, ft) = (entry.path(), entry.file_type()?);
            if ft.is_dir() || (ft.is_symlink() && path.is_dir()) {
                if let Some(n) = entry.file_name().to_str()
                    && ((skip_info && n == PROJECT_INFO_DIR) || (skip_backup && n == BACKUP_DIR))
                {
                    continue;
                }
                stack.push(path);
            } else if ft.is_file() || (ft.is_symlink() && path.is_file()) {
                out.push(path);
            }
        }
    }
    Ok(out)
}

pub(crate) fn relative_forward_slash(target: &Path, base: &Path) -> Option<String> {
    let rel = canonical_or_self(target)
        .strip_prefix(canonical_or_self(base))
        .ok()?
        .to_path_buf();
    let s = rel.to_string_lossy().replace('\\', "/");
    (!s.is_empty()).then_some(s)
}

pub(crate) fn copy_dedup(
    src: &Path,
    imported: &Path,
    root: &Path,
) -> Result<(PathBuf, String, bool), ProjectDirError> {
    let name0 = src
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && !matches!(*s, "." | "..") && !s.contains(['/', '\\']))
        .ok_or_else(|| ProjectDirError::InvalidPath(format!("invalid name: {}", src.display())))?;
    let size = fs::metadata(src)?.len();
    let stem = Path::new(name0)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name0);
    let ext = Path::new(name0).extension().and_then(|s| s.to_str());
    for suffix in 0usize.. {
        let name = match (suffix, ext) {
            (0, _) => name0.to_owned(),
            (n, Some(e)) => format!("{stem}_{n}.{e}"),
            (n, None) => format!("{stem}_{n}"),
        };
        let dest = imported.join(&name);
        if dest.exists() {
            if fs::metadata(&dest).is_ok_and(|m| m.len() == size) && files_equal(src, &dest)? {
                let rel = relative_forward_slash(&dest, root)
                    .unwrap_or_else(|| format!("{SAMPLES_DIR}/{SAMPLES_IMPORTED_DIR}/{name}"));
                return Ok((dest, rel, true));
            }
            continue;
        }
        fs::copy(src, &dest)?;
        let rel = relative_forward_slash(&dest, root)
            .unwrap_or_else(|| format!("{SAMPLES_DIR}/{SAMPLES_IMPORTED_DIR}/{name}"));
        return Ok((dest, rel, false));
    }
    Err(ProjectDirError::InvalidPath("collision limit".into()))
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, ProjectDirError> {
    let (mut fa, mut fb) = (File::open(a)?, File::open(b)?);
    let (mut ba, mut bb) = ([0u8; 8192], [0u8; 8192]);
    loop {
        let na = fa.read(&mut ba)?;
        if na == 0 {
            return Ok(fb.read(&mut bb[..1])? == 0);
        }
        if fb.read_exact(&mut bb[..na]).is_err() || ba[..na] != bb[..na] {
            return Ok(false);
        }
    }
}

pub(crate) fn has_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
}
