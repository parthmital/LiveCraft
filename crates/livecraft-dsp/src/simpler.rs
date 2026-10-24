use crate::biquad::{BiquadFilter, FilterType};
use crate::operator::AdsrEnvelope;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SimplerSample {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SimplerInstrument {
    pub sample: Option<SimplerSample>,
    sample_rate: f32,
    playhead: f64,
    speed: f64,
    is_active: bool,
    pub envelope: AdsrEnvelope,
    filter: BiquadFilter,
    pub filter_cutoff: f32,
    pub root_note: u8,
    pub is_looping: bool,
    pub is_enabled: bool,
}

impl SimplerInstrument {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample: None,
            sample_rate,
            playhead: 0.0,
            speed: 1.0,
            is_active: false,
            envelope: AdsrEnvelope::default(),
            filter: BiquadFilter::new(FilterType::LowPass, sample_rate, 18000.0, 0.0, 0.707),
            filter_cutoff: 18000.0,
            root_note: 60, // Middle C
            is_looping: false,
            is_enabled: true,
        }
    }

    pub fn load_wav<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let channels = spec.channels;
        let srate = spec.sample_rate;

        let mut samples_l = Vec::new();
        let mut samples_r = Vec::new();

        let max_val = match spec.bits_per_sample {
            16 => i16::MAX as f32,
            24 => 8388607.0,
            32 => i32::MAX as f32,
            _ => 32767.0,
        };

        if spec.sample_format == hound::SampleFormat::Float {
            let mut ch = 0;
            for sample in reader.samples::<f32>() {
                let s = sample.unwrap_or(0.0);
                if ch == 0 {
                    samples_l.push(s);
                    if channels == 1 {
                        samples_r.push(s);
                    }
                } else {
                    samples_r.push(s);
                }
                ch = (ch + 1) % channels;
            }
        } else {
            let mut ch = 0;
            for sample in reader.samples::<i32>() {
                let s = sample.unwrap_or(0) as f32 / max_val;
                if ch == 0 {
                    samples_l.push(s);
                    if channels == 1 {
                        samples_r.push(s);
                    }
                } else {
                    samples_r.push(s);
                }
                ch = (ch + 1) % channels;
            }
        }

        self.sample = Some(SimplerSample {
            channels,
            sample_rate: srate,
            samples_l,
            samples_r,
        });

        Ok(())
    }

    pub fn note_on(&mut self, note: u8, _velocity: u8) {
        if let Some(ref sample) = self.sample {
            if !sample.samples_l.is_empty() {
                self.playhead = 0.0;
                let semitones = (note as i32) - (self.root_note as i32);
                self.speed = 2.0f64.powf(semitones as f64 / 12.0)
                    * (sample.sample_rate as f64 / self.sample_rate as f64);
                self.is_active = true;
            }
        }
    }

    pub fn note_off(&mut self, _note: u8) {
        if !self.is_looping {
            // Let one-shot decay or release
        } else {
            self.is_active = false;
        }
    }

    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.is_enabled || !self.is_active {
            return (0.0, 0.0);
        }

        if let Some(ref sample) = self.sample {
            let len = sample.samples_l.len();
            if len == 0 {
                return (0.0, 0.0);
            }

            let idx = self.playhead as usize;
            if idx >= len {
                if self.is_looping {
                    self.playhead %= len as f64;
                } else {
                    self.is_active = false;
                    return (0.0, 0.0);
                }
            }

            let s_l = sample.samples_l[self.playhead as usize];
            let s_r = sample.samples_r[self.playhead as usize];

            self.playhead += self.speed;

            self.filter.set_params(self.filter_cutoff, 0.0, 0.707);
            self.filter.process_stereo(s_l, s_r)
        } else {
            (0.0, 0.0)
        }
    }
}
