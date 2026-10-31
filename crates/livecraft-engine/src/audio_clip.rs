use hound::WavReader;
use std::path::Path;

#[inline(always)]
pub fn hermite_interp(y0: f32, y1: f32, y2: f32, y3: f32, mu: f32) -> f32 {
    let mu2 = mu * mu;
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    let a3 = y1;
    a0 * mu * mu2 + a1 * mu2 + a2 * mu + a3
}

#[derive(Debug, Clone, Default)]
pub struct DecodedAudio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

/// Most samples (all channels together) a WAV file may hold to be decoded into memory:
/// 2^28 samples, 1 GiB as `f32`. Longer files need disk streaming (milestone M11).
pub const MAX_WAV_SAMPLES: u64 = 1 << 28;

/// Decodes a WAV file into de-interleaved left and right channels (mono is copied to both).
pub fn load_file(path: &Path) -> anyhow::Result<DecodedAudio> {
    load_file_with_limit(path, MAX_WAV_SAMPLES)
}

/// Like [`load_file`], but rejects a file holding more than `max_samples` samples
/// before reading any sample data.
pub fn load_file_with_limit(path: &Path, max_samples: u64) -> anyhow::Result<DecodedAudio> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    let total = u64::from(reader.duration()) * u64::from(spec.channels);
    if total > max_samples {
        anyhow::bail!(
            "{} holds {total} samples, over the limit of {max_samples}",
            path.display()
        );
    }
    if spec.channels == 0 {
        anyhow::bail!("{} has no audio channels", path.display());
    }
    let channels = spec.channels;
    let sample_rate = spec.sample_rate;

    let mut samples_l = Vec::new();
    let mut samples_r = Vec::new();

    let max_val = match spec.bits_per_sample {
        16 => 32768.0,
        24 => 8388608.0,
        32 => 2147483648.0,
        _ => 32768.0,
    };

    if spec.sample_format == hound::SampleFormat::Float {
        let mut ch = 0;
        for s in reader.samples::<f32>() {
            let sample = s.unwrap_or(0.0);
            if ch == 0 {
                samples_l.push(sample);
                if channels == 1 {
                    samples_r.push(sample);
                }
            } else {
                samples_r.push(sample);
            }
            ch = (ch + 1) % channels;
        }
    } else {
        let mut ch = 0;
        for s in reader.samples::<i32>() {
            let sample = s.unwrap_or(0) as f32 / max_val;
            if ch == 0 {
                samples_l.push(sample);
                if channels == 1 {
                    samples_r.push(sample);
                }
            } else {
                samples_r.push(sample);
            }
            ch = (ch + 1) % channels;
        }
    }

    Ok(DecodedAudio {
        channels,
        sample_rate,
        samples_l,
        samples_r,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermite_interpolation_passes_through_a_straight_line_at_its_midpoint() {
        let mid = hermite_interp(0.0, 1.0, 2.0, 3.0, 0.5);
        assert!((mid - 1.5).abs() < 1e-4);
    }
}
