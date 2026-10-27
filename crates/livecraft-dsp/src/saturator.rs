use crate::biquad::{BiquadFilter, FilterType};
use livecraft_model::model::Device;
use std::f32::consts::FRAC_PI_2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturatorCurve {
    SoftSine,
    HardClip,
    AnalogWarmth,
    Foldback,
}

#[derive(Debug, Clone)]
pub struct SaturatorEffect {
    pub device_id: u64,
    pub drive_db: f32,
    pub curve: SaturatorCurve,
    pub output_db: f32,
    pub dry_wet: f32,
    pub tone_filter: BiquadFilter,
    pub is_enabled: bool,
}

impl SaturatorEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            device_id: 0,
            drive_db: 6.0,
            curve: SaturatorCurve::SoftSine,
            output_db: -2.0,
            dry_wet: 1.0,
            tone_filter: BiquadFilter::new(FilterType::LowPass, sample_rate, 12000.0, 0.0, 0.707),
            is_enabled: true,
        }
    }

    pub fn update_from_device(&mut self, dev: &Device) {
        self.is_enabled = dev.is_enabled;
        self.drive_db = dev.get_param("drive_db");
        let c = dev.get_param("curve_type").round() as i32;
        self.curve = match c {
            0 => SaturatorCurve::SoftSine,
            1 => SaturatorCurve::HardClip,
            2 => SaturatorCurve::AnalogWarmth,
            3 => SaturatorCurve::Foldback,
            _ => SaturatorCurve::SoftSine,
        };
        self.output_db = dev.get_param("output_db");
        self.dry_wet = dev.get_param("dry_wet").clamp(0.0, 1.0);
    }

    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l, in_r);
        }

        let drive_lin = 10.0f32.powf(self.drive_db / 20.0);
        let out_lin = 10.0f32.powf(self.output_db / 20.0);

        let sat_l = self.saturate(in_l * drive_lin) * out_lin;
        let sat_r = self.saturate(in_r * drive_lin) * out_lin;

        // Apply tone filter to tame harsh harmonics
        let (filt_l, filt_r) = self.tone_filter.process_stereo(sat_l, sat_r);

        let final_l = in_l * (1.0 - self.dry_wet) + filt_l * self.dry_wet;
        let final_r = in_r * (1.0 - self.dry_wet) + filt_r * self.dry_wet;

        (final_l, final_r)
    }

    fn saturate(&self, x: f32) -> f32 {
        match self.curve {
            SaturatorCurve::SoftSine => {
                let clamped = x.clamp(-2.0, 2.0);
                (clamped * FRAC_PI_2).sin()
            }
            SaturatorCurve::HardClip => x.clamp(-1.0, 1.0),
            SaturatorCurve::AnalogWarmth => {
                // Asymmetric warmth modeling tube/tape saturation
                let pos = x / (1.0 + x.abs());
                let even_harmonics = 0.08 * (x * x).min(1.0);
                (pos + even_harmonics).clamp(-1.0, 1.0)
            }
            SaturatorCurve::Foldback => {
                // Wavefolding distortion
                let mut v = x;
                while v > 1.0 || v < -1.0 {
                    if v > 1.0 {
                        v = 2.0 - v;
                    } else if v < -1.0 {
                        v = -2.0 - v;
                    }
                }
                v
            }
        }
    }
}
