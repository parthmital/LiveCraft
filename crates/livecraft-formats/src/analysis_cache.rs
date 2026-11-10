//! Streaming 3-level waveform peak pyramid and `.lcanalysis` disk cache.

use std::fs::{self, Metadata};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::audio_decode::{AudioDecodeError, AudioInfo, AudioStreamReader};
use crate::health::canonical_or_self;

/// Version tag written into `.lcanalysis` files (aligned with M08 `AnalysisFile`).
pub const ANALYSIS_VERSION: u32 = 1;

/// Frame counts per peak block for each pyramid level.
pub const PYRAMID_BLOCK_256: usize = 256;
pub const PYRAMID_BLOCK_4096: usize = 4096;
pub const PYRAMID_BLOCK_65536: usize = 65536;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Min and max sample amplitude over one block of frames.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeakFrame {
    pub min: f32,
    pub max: f32,
}

/// 3-level waveform overview pyramid (256-, 4096-, and 65536-frame blocks).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WaveformPyramid {
    pub block_256: Vec<PeakFrame>,
    pub block_4096: Vec<PeakFrame>,
    pub block_65536: Vec<PeakFrame>,
}

/// Where `.lcanalysis` sidecar caches are stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisCacheLocation {
    BesideSample,
    Folder(PathBuf),
}

/// Persisted `.lcanalysis` JSON payload, backwards-compatible with M08 `AnalysisFile`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedSampleAnalysis {
    pub version: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub overview_block_frames: u32,
    pub peak_overview: Vec<PeakFrame>,
    #[serde(default)]
    pub overview_4096: Vec<PeakFrame>,
    #[serde(default)]
    pub overview_65536: Vec<PeakFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_len_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mtime_unix_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
}

impl CachedSampleAnalysis {
    /// Returns the 3-level [`WaveformPyramid`] represented by this cached entry.
    #[must_use]
    pub fn pyramid(&self) -> WaveformPyramid {
        WaveformPyramid {
            block_256: self.peak_overview.clone(),
            block_4096: self.overview_4096.clone(),
            block_65536: self.overview_65536.clone(),
        }
    }
}

struct PeakAccumulator {
    block_size: usize,
    count: usize,
    min: f32,
    max: f32,
    out: Vec<PeakFrame>,
}

impl PeakAccumulator {
    fn new(block_size: usize, reserve_blocks: usize) -> Self {
        Self {
            block_size,
            count: 0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            out: Vec::with_capacity(reserve_blocks),
        }
    }

    #[inline]
    fn push_frame(&mut self, l: f32, r: f32) {
        let vl = if l.is_finite() { l } else { 0.0 };
        let vr = if r.is_finite() { r } else { 0.0 };
        self.min = self.min.min(vl).min(vr);
        self.max = self.max.max(vl).max(vr);
        self.count += 1;
        if self.count == self.block_size {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.count > 0 {
            let min = if self.min.is_finite() { self.min } else { 0.0 };
            let max = if self.max.is_finite() { self.max } else { 0.0 };
            self.out.push(PeakFrame { min, max });
            self.count = 0;
            self.min = f32::INFINITY;
            self.max = f32::NEG_INFINITY;
        }
    }
}

/// Computes all 3 waveform peak pyramid levels in one streaming pass with O(1) working memory.
pub fn build_waveform_pyramid_streaming(
    path: &Path,
) -> Result<(WaveformPyramid, AudioInfo, u64), AudioDecodeError> {
    let mut reader = AudioStreamReader::open(path)?;
    let info = AudioInfo {
        sample_rate: reader.sample_rate(),
        channels: reader.channels(),
        total_frames: reader.total_frames(),
    };

    let cap_hint = info.total_frames.unwrap_or(0).min(1 << 22);
    let mut acc_256 = PeakAccumulator::new(
        PYRAMID_BLOCK_256,
        cap_hint.div_ceil(PYRAMID_BLOCK_256 as u64) as usize,
    );
    let mut acc_4096 = PeakAccumulator::new(
        PYRAMID_BLOCK_4096,
        cap_hint.div_ceil(PYRAMID_BLOCK_4096 as u64) as usize,
    );
    let mut acc_65536 = PeakAccumulator::new(
        PYRAMID_BLOCK_65536,
        cap_hint.div_ceil(PYRAMID_BLOCK_65536 as u64) as usize,
    );

    let mut chunk = Vec::with_capacity(4096);
    let mut actual_frames: u64 = 0;
    loop {
        let n = reader.read_frames(4096, &mut chunk)?;
        if n == 0 {
            break;
        }
        for &[l, r] in &chunk {
            acc_256.push_frame(l, r);
            acc_4096.push_frame(l, r);
            acc_65536.push_frame(l, r);
        }
        actual_frames = actual_frames.saturating_add(n as u64);
    }

    acc_256.flush();
    acc_4096.flush();
    acc_65536.flush();

    let pyramid = WaveformPyramid {
        block_256: acc_256.out,
        block_4096: acc_4096.out,
        block_65536: acc_65536.out,
    };
    Ok((pyramid, info, actual_frames))
}

fn fnv1a_64(bytes: &[u8], seed: u64) -> u64 {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut hash = seed;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn deterministic_path_digest(path: &Path) -> String {
    let canon = canonical_or_self(path);
    let normalized = canon.to_string_lossy().replace('\\', "/");
    let bytes = normalized.as_bytes();
    let h1 = fnv1a_64(bytes, 0xcbf2_9ce4_8422_2325);
    let h2 = fnv1a_64(bytes, 0xaf63_bd4c_8601_b7df);
    format!("{h1:016x}{h2:016x}")
}

/// Computes the `.lcanalysis` cache file path for `sample_path` under `loc`.
#[must_use]
pub fn analysis_cache_path(sample_path: &Path, loc: &AnalysisCacheLocation) -> PathBuf {
    match loc {
        AnalysisCacheLocation::BesideSample => {
            let mut os = sample_path.as_os_str().to_os_string();
            os.push(".lcanalysis");
            PathBuf::from(os)
        }
        AnalysisCacheLocation::Folder(dir) => {
            let raw_name = sample_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("sample");
            let mut sanitized: String = raw_name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                        c
                    } else {
                        '_'
                    }
                })
                .take(40)
                .collect();
            if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
                sanitized = "sample".to_owned();
            }
            let digest = deterministic_path_digest(sample_path);
            dir.join(format!("{sanitized}_{digest}.lcanalysis"))
        }
    }
}

fn mtime_unix_secs(meta: &Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

fn expected_blocks(frames: u64, block_size: usize) -> usize {
    frames.div_ceil(block_size as u64) as usize
}

fn is_valid_cached(
    cached: &CachedSampleAnalysis,
    source_len_bytes: Option<u64>,
    source_mtime_unix_secs: Option<u64>,
) -> bool {
    cached.version == ANALYSIS_VERSION
        && cached.overview_block_frames == PYRAMID_BLOCK_256 as u32
        && cached.source_len_bytes.is_some()
        && cached.source_len_bytes == source_len_bytes
        && cached.source_mtime_unix_secs == source_mtime_unix_secs
        && cached.peak_overview.len() == expected_blocks(cached.frames, PYRAMID_BLOCK_256)
        && cached.overview_4096.len() == expected_blocks(cached.frames, PYRAMID_BLOCK_4096)
        && cached.overview_65536.len() == expected_blocks(cached.frames, PYRAMID_BLOCK_65536)
}

fn write_atomic_json(path: &Path, analysis: &CachedSampleAnalysis) -> Result<(), AudioDecodeError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_vec_pretty(analysis)
        .map_err(|e| AudioDecodeError::InvalidMetadata(format!("serialize analysis JSON: {e}")))?;

    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp_os = path.as_os_str().to_os_string();
    tmp_os.push(format!(".tmp.{}.{seq}", std::process::id()));
    let tmp_path = PathBuf::from(tmp_os);

    let write_res = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(&json)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(AudioDecodeError::Io(err));
    }

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(AudioDecodeError::Io(err));
    }
    Ok(())
}

/// Loads a valid `.lcanalysis` cache when fresh, or streams the sample to build and optionally persist it.
///
/// Live's `.asd` analysis files are never modified or deleted.
pub fn load_or_build_analysis(
    sample_path: &Path,
    loc: &AnalysisCacheLocation,
    write_cache: bool,
) -> Result<CachedSampleAnalysis, AudioDecodeError> {
    let meta = fs::metadata(sample_path)?;
    let source_len_bytes = Some(meta.len());
    let source_mtime_unix_secs = mtime_unix_secs(&meta);

    let cache_path = analysis_cache_path(sample_path, loc);
    if cache_path.is_file()
        && let Ok(bytes) = fs::read(&cache_path)
        && let Ok(cached) = serde_json::from_slice::<CachedSampleAnalysis>(&bytes)
        && is_valid_cached(&cached, source_len_bytes, source_mtime_unix_secs)
    {
        return Ok(cached);
    }

    let (pyramid, info, actual_frames) = build_waveform_pyramid_streaming(sample_path)?;
    let analysis = CachedSampleAnalysis {
        version: ANALYSIS_VERSION,
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: actual_frames,
        overview_block_frames: PYRAMID_BLOCK_256 as u32,
        peak_overview: pyramid.block_256,
        overview_4096: pyramid.block_4096,
        overview_65536: pyramid.block_65536,
        source_len_bytes,
        source_mtime_unix_secs,
        metrics: None,
    };

    if write_cache {
        write_atomic_json(&cache_path, &analysis)?;
    }

    Ok(analysis)
}
