use livecraft_model::ids::DeviceId;
use livecraft_model::model::Device;
use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct ChorusEffect {
    pub device_id: DeviceId,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    lfo_phase: f32,
    pub rate_hz: f32,
    pub depth: f32,
    pub feedback: f32,
    pub dry_wet: f32,
    pub is_enabled: bool,
}

impl ChorusEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = 2048;
        Self {
            device_id: DeviceId(0),
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            lfo_phase: 0.0,
            rate_hz: 1.2,
            depth: 0.5,
            feedback: 0.2,
            dry_wet: 0.5,
            is_enabled: true,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.rate_hz = dev.get_param("rate_hz").clamp(0.1, 10.0);
        self.depth = dev.get_param("depth").clamp(0.0, 1.0);
        self.feedback = dev.get_param("feedback").clamp(0.0, 0.75);
        self.dry_wet = dev.get_param("dry_wet").clamp(0.0, 1.0);
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let buf_len = self.buffer_l.len();
        let dt = 1.0 / self.sample_rate;

        // Quadrature LFOs (Left = sin, Right = cos for rich stereo widening)
        let lfo_l = (self.lfo_phase * 2.0 * PI).sin();
        let lfo_r = (self.lfo_phase * 2.0 * PI).cos();
        self.lfo_phase = (self.lfo_phase + self.rate_hz * dt).fract();

        // Delay modulated between 3ms and 15ms
        let base_delay_ms = 7.0;
        let mod_delay_ms = 5.0 * self.depth;
        let delay_ms_l = base_delay_ms + mod_delay_ms * lfo_l;
        let delay_ms_r = base_delay_ms + mod_delay_ms * lfo_r;

        let delay_samples_l =
            (delay_ms_l * 0.001 * self.sample_rate).clamp(1.0, (buf_len - 2) as f32);
        let delay_samples_r =
            (delay_ms_r * 0.001 * self.sample_rate).clamp(1.0, (buf_len - 2) as f32);

        // Read interpolated delayed samples
        let read_l = self.read_interpolated(&self.buffer_l, delay_samples_l);
        let read_r = self.read_interpolated(&self.buffer_r, delay_samples_r);

        // Write with feedback
        self.buffer_l[self.write_pos] = in_l + read_l * self.feedback;
        self.buffer_r[self.write_pos] = in_r + read_r * self.feedback;
        self.write_pos = (self.write_pos + 1) % buf_len;

        let out_l = in_l * (1.0 - self.dry_wet) + read_l * self.dry_wet;
        let out_r = in_r * (1.0 - self.dry_wet) + read_r * self.dry_wet;

        (out_l, out_r)
    }

    fn read_interpolated(&self, buf: &[f32], delay_samples: f32) -> f32 {
        let buf_len = buf.len();
        let int_delay = delay_samples.floor() as usize;
        let frac = delay_samples - delay_samples.floor();

        let idx0 = if self.write_pos >= int_delay {
            self.write_pos - int_delay
        } else {
            buf_len + self.write_pos - int_delay
        };

        let idx1 = if idx0 > 0 { idx0 - 1 } else { buf_len - 1 };

        let s0 = buf[idx0 % buf_len];
        let s1 = buf[idx1 % buf_len];

        s0 + frac * (s1 - s0)
    }
}
