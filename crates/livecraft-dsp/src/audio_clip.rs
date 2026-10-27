use hound::WavReader;
use livecraft_core::model::{Clip, ClipPayload};
use std::collections::HashMap;
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
            match Self::load_file(path.as_ref()) {
                Ok(decoded) => {
                    self.cache.insert(p_str.clone(), decoded);
                }
                _ => {
                    return None;
                }
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

    pub fn render_clip_samples(
        &mut self,
        clip: &Clip,
        start_clip_pos_beats: f64,
        dt_beats_per_sample: f64,
        bpm: f32,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        out_l.fill(0.0);
        out_r.fill(0.0);

        if let ClipPayload::Audio(ref audio_data) = clip.payload {
            if audio_data.file_path.is_empty() {
                return;
            }

            let path = audio_data.file_path.clone();
            if let Some(audio) = self.get_or_load(&path) {
                let total_samples = audio.samples_l.len();
                if total_samples == 0 {
                    return;
                }

                let gain_lin = 10.0f32.powf(audio_data.gain_db / 20.0);
                let time_scale = if audio_data.is_warped {
                    (60.0 / bpm.max(20.0) as f64) * audio.sample_rate as f64
                } else {
                    (60.0 / audio_data.original_bpm.max(20.0)) * audio.sample_rate as f64
                };

                let n = out_l.len().min(out_r.len());
                let last_idx = (total_samples - 1) as isize;
                let clamp_idx = |idx: isize| -> usize { idx.clamp(0, last_idx) as usize };

                for i in 0..n {
                    let beat_pos = start_clip_pos_beats + (i as f64) * dt_beats_per_sample;
                    if beat_pos < 0.0 {
                        continue;
                    }

                    let sample_idx_f = beat_pos * time_scale;
                    if sample_idx_f < 0.0 {
                        continue;
                    }

                    let idx1 = sample_idx_f.floor() as isize;
                    if idx1 >= total_samples as isize {
                        continue;
                    }

                    let mu = (sample_idx_f - sample_idx_f.floor()) as f32;

                    let i0 = clamp_idx(idx1 - 1);
                    let i1 = clamp_idx(idx1);
                    let i2 = clamp_idx(idx1 + 1);
                    let i3 = clamp_idx(idx1 + 2);

                    let y0_l = audio.samples_l[i0];
                    let y1_l = audio.samples_l[i1];
                    let y2_l = audio.samples_l[i2];
                    let y3_l = audio.samples_l[i3];

                    let y0_r = audio.samples_r[i0];
                    let y1_r = audio.samples_r[i1];
                    let y2_r = audio.samples_r[i2];
                    let y3_r = audio.samples_r[i3];

                    let samp_l = hermite_interp(y0_l, y1_l, y2_l, y3_l, mu);
                    let samp_r = hermite_interp(y0_r, y1_r, y2_r, y3_r, mu);

                    out_l[i] = samp_l * gain_lin;
                    out_r[i] = samp_r * gain_lin;
                }
            }
        }
    }

    pub fn sample_clip(&mut self, clip: &Clip, clip_pos_beats: f64, bpm: f32) -> (f32, f32) {
        let mut buf_l = [0.0f32; 1];
        let mut buf_r = [0.0f32; 1];
        self.render_clip_samples(clip, clip_pos_beats, 0.0, bpm, &mut buf_l, &mut buf_r);
        (buf_l[0], buf_r[0])
    }
}
