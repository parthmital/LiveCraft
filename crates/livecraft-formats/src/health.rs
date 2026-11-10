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
        return probe_mp3_rate(&mut file, &hdr, file_len, path);
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
    let xml_rates = tree
        .map(extract_audio_clip_explicit_rates)
        .unwrap_or_default();
    let mut acc = HealthAcc::default();
    for (clip_id, mut r, loc) in collect_project_clip_refs_with_id(project) {
        if r.relative_path.is_none() {
            r.relative_path = xml_rel.get(&r.path).and_then(|x| x.relative_path.clone());
        }
        let exp_sr = if let Some(exp) = options.expected_sample_rate {
            exp
        } else if tree.is_some() {
            xml_rates
                .by_id
                .get(&clip_id)
                .copied()
                .or_else(|| {
                    xml_rates
                        .by_ref
                        .get(&(r.path.clone(), r.relative_path.clone()))
                        .copied()
                })
                .unwrap_or(0)
        } else {
            0
        };
        acc.check(&r, loc, exp_sr, &ctx);
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

fn is_external_plugin_tag(tag: &str) -> bool {
    tag.contains("Plugin") || tag.contains("MxDevice")
}

fn is_plugin_available(tag: &str, name: &str, available: Option<&[String]>) -> bool {
    let Some(list) = available else {
        return !is_external_plugin_tag(tag);
    };
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

#[derive(Clone, Copy)]
struct Mp3FrameHeader {
    version: u8,
    layer: u8,
    sample_rate: u32,
    frame_len: usize,
}

fn parse_mp3_frame_header(w: &[u8]) -> Option<Mp3FrameHeader> {
    if w.len() < 4 || w[0] != 0xFF || (w[1] & 0xE0) != 0xE0 {
        return None;
    }
    let version = (w[1] >> 3) & 3;
    let layer = (w[1] >> 1) & 3;
    let br_idx = ((w[2] >> 4) & 0x0F) as usize;
    let sr_idx = ((w[2] >> 2) & 0x03) as usize;
    let padding = usize::from((w[2] >> 1) & 1);
    if version == 1 || layer == 0 || br_idx == 0 || br_idx == 15 || sr_idx >= 3 {
        return None;
    }
    let sample_rate = match version {
        3 => [44100, 48000, 32000][sr_idx],
        2 => [22050, 24000, 16000][sr_idx],
        _ => [11025, 12000, 8000][sr_idx],
    };
    let kbps: usize = match (version, layer) {
        (3, 3) => [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ][br_idx],
        (3, 2) => [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ][br_idx],
        (3, 1) => [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ][br_idx],
        (_, 3) => [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ][br_idx],
        _ => [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160][br_idx],
    };
    let bitrate_bps = kbps * 1000;
    let sr = sample_rate as usize;
    let frame_len = match (version, layer) {
        (_, 3) => ((12 * bitrate_bps) / sr + padding) * 4,
        (3, _) | (_, 2) => (144 * bitrate_bps) / sr + padding,
        _ => (72 * bitrate_bps) / sr + padding,
    };
    (frame_len >= 4).then_some(Mp3FrameHeader {
        version,
        layer,
        sample_rate,
        frame_len,
    })
}

fn probe_mp3_rate(
    file: &mut File,
    hdr: &[u8; 12],
    file_len: u64,
    path: &Path,
) -> Result<u32, HealthError> {
    let start = if &hdr[0..3] == b"ID3" {
        let sz = ((u64::from(hdr[6] & 0x7F)) << 21)
            | ((u64::from(hdr[7] & 0x7F)) << 14)
            | ((u64::from(hdr[8] & 0x7F)) << 7)
            | u64::from(hdr[9] & 0x7F);
        10 + sz + if (hdr[5] & 0x10) != 0 { 10 } else { 0 }
    } else {
        0
    };
    if start >= file_len {
        return Err(HealthError::AudioProbe {
            path: path.to_path_buf(),
            reason: "MP3 frame header not found".into(),
        });
    }
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; 65_536];
    let n = file.read(&mut buf)?;
    let mut fallback = None;
    for i in 0..n.saturating_sub(3) {
        let Some(h1) = parse_mp3_frame_header(&buf[i..i + 4]) else {
            continue;
        };
        let next_file_pos = start
            .saturating_add(i as u64)
            .saturating_add(h1.frame_len as u64);
        if next_file_pos.saturating_add(4) <= file_len {
            let next_buf_pos = i.saturating_add(h1.frame_len);
            let next_bytes = if next_buf_pos.saturating_add(4) <= n {
                Some([
                    buf[next_buf_pos],
                    buf[next_buf_pos + 1],
                    buf[next_buf_pos + 2],
                    buf[next_buf_pos + 3],
                ])
            } else if file.seek(SeekFrom::Start(next_file_pos)).is_ok() {
                let mut tmp = [0u8; 4];
                file.read_exact(&mut tmp).ok().map(|()| tmp)
            } else {
                None
            };
            if let Some(nb) = next_bytes
                && let Some(h2) = parse_mp3_frame_header(&nb)
                && h2.version == h1.version
                && h2.layer == h1.layer
                && h2.sample_rate == h1.sample_rate
            {
                return Ok(h1.sample_rate);
            }
        } else if file_len.saturating_sub(start) < (h1.frame_len as u64).saturating_add(4) {
            fallback.get_or_insert(h1.sample_rate);
        }
    }
    fallback.ok_or_else(|| HealthError::AudioProbe {
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
    collect_project_clip_refs_with_id(project)
        .into_iter()
        .map(|(_, r, loc)| (r, loc))
        .collect()
}

fn collect_project_clip_refs_with_id(
    project: &Project,
) -> Vec<(u64, SampleReference, ClipLocationRef)> {
    let mut out = Vec::new();
    for t in project.all_tracks() {
        for (i, s) in t.clip_slots.iter().enumerate() {
            if let Some(ref c) = s.clip
                && let ClipPayload::Audio(ref a) = c.payload
                && (!a.file_path.trim().is_empty()
                    || a.relative_path
                        .as_deref()
                        .is_some_and(|rel| !rel.trim().is_empty()))
            {
                let loc = ClipLocationRef::SessionSlot(t.id.0, i, c.name.clone());
                let sr = SampleReference {
                    path: a.file_path.clone(),
                    relative_path: a.relative_path.clone(),
                };
                out.push((c.id.0, sr, loc));
            }
        }
        for arr in &t.arrangement_clips {
            if let ClipPayload::Audio(ref a) = arr.clip.payload
                && (!a.file_path.trim().is_empty()
                    || a.relative_path
                        .as_deref()
                        .is_some_and(|rel| !rel.trim().is_empty()))
            {
                let loc =
                    ClipLocationRef::Arrangement(t.id.0, arr.clip.id.0, arr.clip.name.clone());
                let sr = SampleReference {
                    path: a.file_path.clone(),
                    relative_path: a.relative_path.clone(),
                };
                out.push((arr.clip.id.0, sr, loc));
            }
        }
    }
    out
}

#[derive(Default)]
struct ExplicitClipRates {
    by_id: HashMap<u64, u32>,
    by_ref: HashMap<(String, Option<String>), u32>,
}

fn extract_audio_clip_explicit_rates(tree: &XmlTree) -> ExplicitClipRates {
    let mut out = ExplicitClipRates::default();
    let mut stack = vec![tree.root()];
    while let Some(nid) = stack.pop() {
        let Some(node) = tree.node(nid) else {
            continue;
        };
        let Some(tag) = node.element_name() else {
            continue;
        };
        if tag == "AudioClip" {
            let sr_opt = tree
                .child(nid, "SampleRef")
                .and_then(|sr| read_val(tree, sr, "DefaultSampleRate"))
                .and_then(|s| s.parse::<u32>().ok());
            if let Some(sr) = sr_opt {
                if let Some(cid) = tree.attr(nid, "Id").and_then(|s| s.parse::<u64>().ok()) {
                    out.by_id.insert(cid, sr);
                }
                if let Some(fr) = tree
                    .child(nid, "SampleRef")
                    .and_then(|sr_node| tree.child(sr_node, "FileRef"))
                    .or_else(|| tree.child(nid, "FileRef"))
                {
                    let path = read_val(tree, fr, "Path").unwrap_or_default();
                    let rel = read_val(tree, fr, "RelativePath").filter(|s| !s.trim().is_empty());
                    let eff_path = if path.trim().is_empty() {
                        rel.clone().unwrap_or_default()
                    } else {
                        path
                    };
                    if !eff_path.trim().is_empty() || rel.is_some() {
                        out.by_ref.entry((eff_path, rel)).or_insert(sr);
                    }
                }
            }
            continue;
        }
        for &c in node.children().iter().rev() {
            stack.push(c);
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
    if let Ok(canon) = fs::canonicalize(path) {
        return canon;
    }
    let mut tail: Vec<Component<'_>> = Vec::new();
    let mut cur = path;
    while !cur.as_os_str().is_empty() {
        if let Ok(canon) = fs::canonicalize(cur) {
            let mut out = canon;
            for comp in tail.into_iter().rev() {
                match comp {
                    Component::CurDir => {}
                    Component::ParentDir => {
                        let _ = out.pop();
                    }
                    other => out.push(other),
                }
            }
            return out;
        }
        let Some(parent) = cur.parent() else {
            break;
        };
        if let Some(last) = cur.components().next_back() {
            tail.push(last);
        }
        cur = parent;
    }
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
    if raw.contains('\0') {
        return None;
    }
    let b = raw.as_bytes();
    let win =
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    if Path::new(raw).is_absolute() || raw.starts_with('/') || raw.starts_with("\\\\") || win {
        return Path::new(raw).is_file().then(|| PathBuf::from(raw));
    }
    let has_parent_dir = raw.split(['/', '\\']).any(|s| s == "..")
        || Path::new(raw)
            .components()
            .any(|c| matches!(c, Component::ParentDir));
    if ctx.project_root.is_none() && has_parent_dir {
        return None;
    }
    let join = |base: &Path| {
        raw.split(['/', '\\'])
            .filter(|s| !s.is_empty() && *s != ".")
            .fold(base.to_path_buf(), |p, s| p.join(s))
    };
    let is_allowed = |candidate: &Path| -> bool {
        candidate.is_file()
            && ctx
                .project_root
                .as_deref()
                .is_none_or(|root| path_is_inside(candidate, root))
    };
    for base in [ctx.project_root.as_deref(), ctx.set_dir.as_deref()]
        .into_iter()
        .flatten()
    {
        let c = join(base);
        if is_allowed(&c) {
            return Some(c);
        }
    }
    (ctx.project_root.is_none() && ctx.set_dir.is_none())
        .then(|| join(Path::new(".")))
        .filter(|c| is_allowed(c))
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
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if let Some(n) = entry.file_name().to_str()
                    && ((skip_info && n == PROJECT_INFO_DIR) || (skip_backup && n == BACKUP_DIR))
                {
                    continue;
                }
                stack.push(path);
            } else if ft.is_file() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_encode::{AudioFileFormat, BitDepth, write_mp3_320, write_pcm_file};
    use crate::project_dir::create_project_layout;
    use livecraft_model::ids::{ClipId, DeviceId};
    use livecraft_model::model::{Device, TrackType};
    use proptest::prelude::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "livecraft_health_unit_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn resolve_candidate_blocks_parent_traversal_and_allows_intra_project_relative() {
        let dir = temp_dir("rel_escape");
        let root = dir.join("Proj Project");
        create_project_layout(&root).expect("layout");
        let clips_dir = root.join("Clips");
        fs::create_dir_all(&clips_dir).expect("clips dir");
        let secret = dir.join("secret.wav");
        let intra = root.join("Samples/Imported/used_alc.wav");
        fs::write(&secret, b"RIFFsecret").expect("write secret");
        fs::write(&intra, b"RIFFintra").expect("write intra");

        let no_root_ctx = ProjectContext {
            project_root: None,
            set_dir: Some(clips_dir.clone()),
            ..ProjectContext::default()
        };
        assert_eq!(resolve_candidate("../../secret.wav", &no_root_ctx), None);
        assert_eq!(resolve_candidate(r"..\..\secret.wav", &no_root_ctx), None);

        let proj_ctx = ProjectContext {
            project_root: Some(root.clone()),
            set_dir: Some(clips_dir),
            ..ProjectContext::default()
        };
        assert_eq!(resolve_candidate("../../secret.wav", &proj_ctx), None);
        assert_eq!(resolve_candidate(r"..\..\secret.wav", &proj_ctx), None);
        assert!(resolve_candidate("../Samples/Imported/used_alc.wav", &proj_ctx).is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn walk_files_skips_symlinks() {
        let dir = temp_dir("walk_symlinks");
        let root = dir.join("Proj Project");
        create_project_layout(&root).expect("layout");
        let real_wav = root.join("Samples/Imported/real.wav");
        fs::write(&real_wav, b"RIFFreal").expect("write real");

        let outside_dir = dir.join("Outside");
        fs::create_dir_all(&outside_dir).expect("outside dir");
        let secret_wav = outside_dir.join("secret.wav");
        fs::write(&secret_wav, b"RIFFsecret").expect("write secret");

        let link_file = root.join("Samples/Imported/link_secret.wav");
        let link_dir = root.join("Samples/LinkDir");

        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&secret_wav, &link_file).is_ok()
            && std::os::unix::fs::symlink(&outside_dir, &link_dir).is_ok();
        #[cfg(windows)]
        let linked = {
            let sym_ok = std::os::windows::fs::symlink_file(&secret_wav, &link_file).is_ok()
                && std::os::windows::fs::symlink_dir(&outside_dir, &link_dir).is_ok();
            if sym_ok {
                true
            } else {
                std::process::Command::new("cmd")
                    .args(["/C", "mklink", "/J"])
                    .arg(&link_dir)
                    .arg(&outside_dir)
                    .output()
                    .is_ok_and(|o| o.status.success())
            }
        };

        assert!(
            linked,
            "expected symlink or Windows NTFS junction creation to succeed in temp dir"
        );
        let walked = walk_files(&root, true, true).expect("walk_files");
        assert_eq!(walked.len(), 1);
        assert_eq!(walked[0], real_wav);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn health_check_defaults_avoid_false_positive_rate_and_native_custom_devices() {
        let dir = temp_dir("defaults");
        let root = dir.join("Clean48k Project");
        create_project_layout(&root).expect("layout");
        let wav48 = root.join("Samples/Recorded/take_48k.wav");
        write_pcm_file(
            &wav48,
            AudioFileFormat::Wav,
            BitDepth::Bits16,
            48000,
            1,
            &[0.1, -0.1],
        )
        .expect("write 48k wav");

        let mut project = Project::new_default("Clean48k");
        let track = project
            .tracks
            .iter_mut()
            .find(|t| t.track_type == TrackType::Audio)
            .expect("audio track");
        track.clip_slots[0].clip = Some(Clip::new_audio(
            ClipId(100),
            "Take",
            "Samples/Recorded/take_48k.wav",
            4.0,
        ));
        project.tracks[0].device_chain.push(Device::new(
            DeviceId(101),
            "Wavetable",
            DeviceType::Custom("Wavetable".to_owned()),
        ));
        project.tracks[0].device_chain.push(Device::new(
            DeviceId(102),
            "Effect Rack",
            DeviceType::Custom("AudioEffectGroupDevice".to_owned()),
        ));
        project.tracks[0].device_chain.push(Device::new(
            DeviceId(103),
            "FabPro",
            DeviceType::Custom("AuPluginDevice".to_owned()),
        ));
        project.tracks[0].device_chain.push(Device::new(
            DeviceId(104),
            "M4L LFO",
            DeviceType::Custom("MxDeviceAudioEffect".to_owned()),
        ));

        let doc = AlsDocument::from_project(&project).expect("doc");
        let opts = HealthCheckOptions {
            project_root: Some(root.clone()),
            ..HealthCheckOptions::default()
        };
        let rep = check_document_health(&doc, &opts).expect("check health");
        assert!(
            rep.sample_rate_mismatches.is_empty(),
            "defaulted 44100 without XML <DefaultSampleRate> must not mismatch: {:?}",
            rep.sample_rate_mismatches
        );
        let flagged: Vec<_> = rep
            .missing_plugins
            .iter()
            .map(|p| p.plugin_type.as_str())
            .collect();
        assert_eq!(flagged, vec!["AuPluginDevice", "MxDeviceAudioEffect"]);

        // Explicit <DefaultSampleRate Value="44100"/> in XML DOES flag 48 kHz file when expected_sample_rate is None.
        let xml = format!(
            "<?xml version=\"1.0\"?><Ableton><LiveSet><Tracks>\
             <AudioTrack Id=\"1\"><Name><EffectiveName Value=\"A\"/></Name>\
             <DeviceChain><MainSequencer><ClipSlotList><ClipSlot Id=\"0\"><ClipSlot><Value>\
             <AudioClip Id=\"77\"><Name Value=\"ExplicitSR\"/>\
             <SampleRef><DefaultSampleRate Value=\"44100\"/><FileRef>\
             <Path Value=\"{}\"/></FileRef></SampleRef>\
             </AudioClip></Value></ClipSlot></ClipSlot></ClipSlotList></MainSequencer></DeviceChain>\
             </AudioTrack></Tracks></LiveSet></Ableton>",
            wav48.to_string_lossy()
        );
        let explicit_doc = AlsDocument::from_xml_named(&xml, "ExplicitSR").expect("xml doc");
        let explicit_rep = check_document_health(&explicit_doc, &opts).expect("explicit health");
        assert_eq!(explicit_rep.sample_rate_mismatches.len(), 1);
        assert_eq!(
            explicit_rep.sample_rate_mismatches[0].expected_sample_rate,
            44100
        );
        assert_eq!(
            explicit_rep.sample_rate_mismatches[0].actual_sample_rate,
            48000
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_mp3_rate_verifies_consecutive_frame_and_keeps_short_fixture() {
        let dir = temp_dir("mp3_probe");

        let short_mp3 = dir.join("short.mp3");
        fs::write(&short_mp3, [0xFF_u8, 0xFB, 0x94, 0x00, 0x00, 0x00]).expect("write short");
        assert_eq!(probe_audio_sample_rate(&short_mp3).expect("short"), 48000);

        // False sync at offset 16 in a 2 KiB file with no valid second frame header at 16 + 384.
        let mut false_sync = vec![0x11_u8; 2048];
        false_sync[16..20].copy_from_slice(&[0xFF, 0xFB, 0x94, 0x00]);
        let bad_mp3 = dir.join("false_sync.mp3");
        fs::write(&bad_mp3, &false_sync).expect("write bad");
        assert!(probe_audio_sample_rate(&bad_mp3).is_err());

        // Two consecutive MPEG-1 Layer III 128 kbps 48 kHz frames (384 bytes per frame).
        let mut two_frames = vec![0u8; 384 * 2];
        two_frames[0..4].copy_from_slice(&[0xFF, 0xFB, 0x94, 0x00]);
        two_frames[384..388].copy_from_slice(&[0xFF, 0xFB, 0x94, 0x00]);
        let valid_synth = dir.join("two_frames.mp3");
        fs::write(&valid_synth, &two_frames).expect("write two frames");
        assert_eq!(
            probe_audio_sample_rate(&valid_synth).expect("two frames"),
            48000
        );

        let real_mp3 = dir.join("real_44k.mp3");
        write_mp3_320(&real_mp3, 44100, 1, &vec![0.0_f32; 4096]).expect("encode mp3");
        assert_eq!(probe_audio_sample_rate(&real_mp3).expect("real 44k"), 44100);

        let _ = fs::remove_dir_all(&dir);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn arbitrary_bytes_never_panic_in_probe_audio_sample_rate(
            ext_idx in 0usize..5,
            bytes in proptest::collection::vec(any::<u8>(), 0..1024)
        ) {
            let ext = ["wav", "aif", "flac", "mp3", "bin"][ext_idx];
            let path = std::env::temp_dir().join(format!(
                "livecraft_probe_prop_{}.{ext}",
                std::process::id()
            ));
            let _ = fs::write(&path, &bytes);
            let _ = probe_audio_sample_rate(&path);
            let _ = fs::remove_file(&path);
        }
    }
}
