use std::path::Path;

use livecraft_formats::audio_decode::{
    AudioDecodeError, decode_audio_file, decode_audio_file_with_limit,
};
pub use livecraft_formats::audio_decode::{DecodedAudio, MAX_RAM_AUDIO_SAMPLES as MAX_WAV_SAMPLES};

#[inline(always)]
pub fn hermite_interp(y0: f32, y1: f32, y2: f32, y3: f32, mu: f32) -> f32 {
    let mu2 = mu * mu;
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    let a3 = y1;
    a0 * mu * mu2 + a1 * mu2 + a2 * mu + a3
}

fn map_decode_error(path: &Path, err: AudioDecodeError) -> anyhow::Error {
    match err {
        AudioDecodeError::ExceedsLimit { samples, limit } => anyhow::anyhow!(
            "{} holds {samples} samples, over the limit of {limit}",
            path.display()
        ),
        other => anyhow::Error::new(other),
    }
}

/// Decodes an audio file (WAV, AIFF, FLAC, MP3, Ogg Vorbis, AAC/ALAC M4A) into de-interleaved
/// stereo `f32` channels (mono is duplicated to both channels), enforcing [`MAX_WAV_SAMPLES`].
pub fn load_file(path: &Path) -> anyhow::Result<DecodedAudio> {
    decode_audio_file(path).map_err(|err| map_decode_error(path, err))
}

/// Like [`load_file`], but rejects a file holding more than `max_samples` total samples
/// (frames * channels).
pub fn load_file_with_limit(path: &Path, max_samples: u64) -> anyhow::Result<DecodedAudio> {
    decode_audio_file_with_limit(path, max_samples).map_err(|err| map_decode_error(path, err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("audio")
    }

    #[test]
    fn hermite_interpolation_passes_through_a_straight_line_at_its_midpoint() {
        let mid = hermite_interp(0.0, 1.0, 2.0, 3.0, 0.5);
        assert!((mid - 1.5).abs() < 1e-4);
    }

    #[test]
    fn load_file_decodes_all_supported_audio_formats() {
        let dir = fixtures_dir();
        for name in [
            "sine_440.wav",
            "sine_440.aif",
            "sine_440.flac",
            "sine_440.mp3",
            "sine_440.ogg",
            "sine_440_aac.m4a",
            "sine_440_alac.m4a",
        ] {
            let audio = load_file(&dir.join(name)).expect("decode fixture");
            assert_eq!(audio.sample_rate, 44100, "{name}");
            assert_eq!(audio.channels, 2, "{name}");
            assert!(!audio.samples_l.is_empty(), "{name}");
            assert_eq!(audio.samples_l.len(), audio.samples_r.len(), "{name}");
        }
    }

    #[test]
    fn load_file_with_limit_rejects_oversized_non_wav_files() {
        let flac = fixtures_dir().join("sine_440.flac");
        let err = load_file_with_limit(&flac, 1000).unwrap_err();
        assert!(err.to_string().contains("over the limit"), "{err}");
    }
}
