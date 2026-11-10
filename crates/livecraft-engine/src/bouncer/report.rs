use super::{DitherMode, RenderOptions};
pub use livecraft_dsp::metrics::AudioMetrics;
use livecraft_formats::audio_encode::AudioFileFormat;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Version tag written into `.lcanalysis` sidecar files.
pub const ANALYSIS_FILE_VERSION: u32 = 1;

/// Frame block size used for min/max peak overview entries in `.lcanalysis` files.
pub const ANALYSIS_OVERVIEW_BLOCK_FRAMES: usize = 256;

/// Min and max sample amplitude over one block of frames in an [`AnalysisFile`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeakFrame {
    pub min: f32,
    pub max: f32,
}

/// Sidecar audio analysis written to `<output>.lcanalysis` when `create_analysis_file` is true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFile {
    pub version: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    #[serde(with = "metrics_serde")]
    pub metrics: AudioMetrics,
    pub overview_block_frames: u32,
    pub peak_overview: Vec<PeakFrame>,
}

/// Metadata and cryptographic hash of a single exported audio file (B13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderOutputReport {
    pub path: String,
    pub format: String,
    pub bit_depth: u16,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub sha256: String,
    #[serde(with = "metrics_serde")]
    pub metrics: AudioMetrics,
}

/// Deterministic render manifest written to `<output>.report.json` when `write_report` is true (B13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderReport {
    pub livecraft_version: String,
    pub git_commit: String,
    pub settings: RenderOptions,
    pub outputs: Vec<RenderOutputReport>,
}

pub(super) mod dither_mode_serde {
    use super::*;

    pub fn serialize<S: Serializer>(mode: &DitherMode, serializer: S) -> Result<S::Ok, S::Error> {
        let name = match mode {
            DitherMode::NoDither => "NoDither",
            DitherMode::Rectangular => "Rectangular",
            DitherMode::Triangular => "Triangular",
            DitherMode::PowR1 => "PowR1",
            DitherMode::PowR2 => "PowR2",
            DitherMode::PowR3 => "PowR3",
        };
        serializer.serialize_str(name)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DitherMode, D::Error> {
        let value = String::deserialize(deserializer)?;
        DitherMode::parse_label(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid dither mode '{value}'")))
    }
}

pub(super) mod audio_file_format_serde {
    use super::*;

    pub fn serialize<S: Serializer>(
        format: &AudioFileFormat,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let name = match format {
            AudioFileFormat::Wav => "Wav",
            AudioFileFormat::Aiff => "Aiff",
            AudioFileFormat::Flac => "Flac",
        };
        serializer.serialize_str(name)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<AudioFileFormat, D::Error> {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "wav" => Ok(AudioFileFormat::Wav),
            "aiff" | "aif" => Ok(AudioFileFormat::Aiff),
            "flac" => Ok(AudioFileFormat::Flac),
            _ => Err(serde::de::Error::custom(format!(
                "invalid audio file format '{value}'"
            ))),
        }
    }
}

pub(super) mod metrics_serde {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct MetricsRepr {
        peak_dbfs: Option<f64>,
        true_peak_dbtp: Option<f64>,
        rms_dbfs: Option<f64>,
        integrated_lufs: Option<f64>,
        short_term_max_lufs: Option<f64>,
        momentary_max_lufs: Option<f64>,
    }

    fn finite_opt(v: f64) -> Option<f64> {
        v.is_finite().then_some(v)
    }

    fn opt_to_f64(v: Option<f64>) -> f64 {
        v.unwrap_or(f64::NEG_INFINITY)
    }

    pub fn serialize<S: Serializer>(
        metrics: &AudioMetrics,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        MetricsRepr {
            peak_dbfs: finite_opt(metrics.peak_dbfs),
            true_peak_dbtp: finite_opt(metrics.true_peak_dbtp),
            rms_dbfs: finite_opt(metrics.rms_dbfs),
            integrated_lufs: finite_opt(metrics.integrated_lufs),
            short_term_max_lufs: finite_opt(metrics.short_term_max_lufs),
            momentary_max_lufs: finite_opt(metrics.momentary_max_lufs),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<AudioMetrics, D::Error> {
        let repr = MetricsRepr::deserialize(deserializer)?;
        Ok(AudioMetrics {
            peak_dbfs: opt_to_f64(repr.peak_dbfs),
            true_peak_dbtp: opt_to_f64(repr.true_peak_dbtp),
            rms_dbfs: opt_to_f64(repr.rms_dbfs),
            integrated_lufs: opt_to_f64(repr.integrated_lufs),
            short_term_max_lufs: opt_to_f64(repr.short_term_max_lufs),
            momentary_max_lufs: opt_to_f64(repr.momentary_max_lufs),
        })
    }
}

pub(super) fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".");
    os.push(suffix);
    PathBuf::from(os)
}

pub(super) fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(super) fn build_peak_overview(samples: &[f32], channels: usize) -> Vec<PeakFrame> {
    let block_samples = ANALYSIS_OVERVIEW_BLOCK_FRAMES * channels;
    let mut overview = Vec::with_capacity(samples.len().div_ceil(block_samples));
    for chunk in samples.chunks(block_samples) {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &sample in chunk {
            let v = if sample.is_finite() { sample } else { 0.0 };
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
        if !min.is_finite() || !max.is_finite() {
            min = 0.0;
            max = 0.0;
        }
        overview.push(PeakFrame { min, max });
    }
    overview
}

pub(super) fn write_analysis_sidecar(
    audio_path: &Path,
    sample_rate: u32,
    channels: u16,
    frames: u64,
    metrics: AudioMetrics,
    samples: &[f32],
) -> anyhow::Result<()> {
    let analysis = AnalysisFile {
        version: ANALYSIS_FILE_VERSION,
        sample_rate,
        channels,
        frames,
        metrics,
        overview_block_frames: ANALYSIS_OVERVIEW_BLOCK_FRAMES as u32,
        peak_overview: build_peak_overview(samples, usize::from(channels)),
    };
    let analysis_path = sidecar_path(audio_path, "lcanalysis");
    ensure_parent_dir(&analysis_path)?;
    let json = serde_json::to_string_pretty(&analysis)?;
    fs::write(analysis_path, json)?;
    Ok(())
}

pub(super) fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

pub(super) fn write_report_file(report_path: &Path, report: &RenderReport) -> anyhow::Result<()> {
    ensure_parent_dir(report_path)?;
    let json = serde_json::to_string_pretty(report)?;
    fs::write(report_path, json)?;
    Ok(())
}
