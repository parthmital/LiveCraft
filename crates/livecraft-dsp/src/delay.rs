use crate::biquad::{BiquadFilter, FilterType};

#[derive(Debug, Clone)]
pub struct DelayEffect {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    pub delay_time_ms: f32,
    pub feedback: f32,
    pub dry_wet: f32,
    pub ping_pong: bool,
    pub is_enabled: bool,
    filter_l: BiquadFilter,
    filter_r: BiquadFilter,
}

impl DelayEffect {
    pub fn new(sample_rate: f32) -> Self {
        // Up to 2.0 seconds of delay buffer
        let max_samples = (sample_rate * 2.0) as usize;
        let filter_l = BiquadFilter::new(FilterType::LowPass, sample_rate, 4500.0, 0.0, 0.707);
        let filter_r = BiquadFilter::new(FilterType::LowPass, sample_rate, 4500.0, 0.0, 0.707);

        Self {
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            delay_time_ms: 375.0, // 1/8th note at 120 bpm
            feedback: 0.40,
            dry_wet: 0.30,
            ping_pong: true,
            is_enabled: true,
            filter_l,
            filter_r,
        }
    }

    pub fn set_tempo_synced(&mut self, bpm: f32, subdivision: f32) {
        // e.g. subdivision 0.5 = 1/8th note
        let beat_sec = 60.0 / bpm.max(20.0);
        self.delay_time_ms = (beat_sec * subdivision * 1000.0).clamp(1.0, 1900.0);
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let delay_samples = ((self.delay_time_ms / 1000.0) * self.sample_rate)
            .clamp(1.0, self.buffer_l.len() as f32 - 1.0) as usize;
        let buf_len = self.buffer_l.len();

        let read_pos = if self.write_pos >= delay_samples {
            self.write_pos - delay_samples
        } else {
            buf_len + self.write_pos - delay_samples
        };

        let delayed_l = self.buffer_l[read_pos];
        let delayed_r = self.buffer_r[read_pos];

        // Filter delayed signal
        let (filtered_l, filtered_r) = if self.ping_pong {
            (
                self.filter_l.process_stereo(delayed_r, delayed_r).0,
                self.filter_r.process_stereo(delayed_l, delayed_l).0,
            )
        } else {
            (
                self.filter_l.process_stereo(delayed_l, delayed_l).0,
                self.filter_r.process_stereo(delayed_r, delayed_r).0,
            )
        };

        // Feedback into buffer
        self.buffer_l[self.write_pos] = in_l + filtered_l * self.feedback.clamp(0.0, 0.98);
        self.buffer_r[self.write_pos] = in_r + filtered_r * self.feedback.clamp(0.0, 0.98);

        self.write_pos = (self.write_pos + 1) % buf_len;

        let out_l = in_l * (1.0 - self.dry_wet) + delayed_l * self.dry_wet;
        let out_r = in_r * (1.0 - self.dry_wet) + delayed_r * self.dry_wet;

        (out_l, out_r)
    }
}
