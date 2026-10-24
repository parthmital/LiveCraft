use hound::WavReader;
use livecraft_core::model::{Clip, ClipPayload};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct DecodedAudio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct AudioSampleCache {
    cache: HashMap<String, DecodedAudio>,
}

impl AudioSampleCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_or_load<P: AsRef<Path>>(&mut self, path: P) -> Option<&DecodedAudio> {
        let p_str = path.as_ref().to_string_lossy().to_string();
        if !self.cache.contains_key(&p_str) {
            if let Ok(decoded) = Self::load_file(path.as_ref()) {
                self.cache.insert(p_str.clone(), decoded);
            } else {
                return None;
            }
        }
        self.cache.get(&p_str)
    }

    fn load_file(path: &Path) -> anyhow::Result<DecodedAudio> {
        let mut reader = WavReader::open(path)?;
        let spec = reader.spec();
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

    pub fn sample_clip(&mut self, clip: &Clip, clip_pos_beats: f64, bpm: f32) -> (f32, f32) {
        if let ClipPayload::Audio(ref audio_data) = clip.payload {
            if audio_data.file_path.is_empty() {
                return (0.0, 0.0);
            }

            let path = audio_data.file_path.clone();
            if let Some(audio) = self.get_or_load(&path) {
                let total_samples = audio.samples_l.len();
                if total_samples == 0 {
                    return (0.0, 0.0);
                }

                let time_sec = if audio_data.is_warped {
                    (clip_pos_beats / bpm.max(20.0) as f64) * 60.0
                } else {
                    (clip_pos_beats / audio_data.original_bpm.max(20.0)) * 60.0
                };

                let sample_idx_f = time_sec * audio.sample_rate as f64;
                if sample_idx_f < 0.0 {
                    return (0.0, 0.0);
                }

                let idx0 = sample_idx_f.floor() as usize;
                if idx0 >= total_samples {
                    return (0.0, 0.0);
                }

                let frac = (sample_idx_f - sample_idx_f.floor()) as f32;
                let idx1 = (idx0 + 1).min(total_samples - 1);

                let sl0 = audio.samples_l[idx0];
                let sl1 = audio.samples_l[idx1];
                let sr0 = audio.samples_r[idx0];
                let sr1 = audio.samples_r[idx1];

                let l = sl0 + frac * (sl1 - sl0);
                let r = sr0 + frac * (sr1 - sr0);

                let gain_lin = 10.0f32.powf(audio_data.gain_db / 20.0);
                return (l * gain_lin, r * gain_lin);
            }
        }
        (0.0, 0.0)
    }
}
