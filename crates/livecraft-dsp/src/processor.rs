use livecraft_core::model::{Device, DeviceType};

use crate::autofilter::{AutoFilterEffect, AutoFilterMode};
use crate::autopan::AutoPanEffect;
use crate::chorus::ChorusEffect;
use crate::compressor::CompressorEffect;
use crate::delay::DelayEffect;
use crate::eq3::EqThreeEffect;
use crate::eq8::EqEight;
use crate::gate::GateEffect;
use crate::limiter::{LimiterEffect, UtilityEffect};
use crate::redux::ReduxEffect;
use crate::reverb::ReverbEffect;
use crate::saturator::{SaturatorCurve, SaturatorEffect};

/// Unified interface for all modular audio effect processors.
pub trait DspProcessor: Send + Sync {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32);
    fn update_param(&mut self, key: &str, value: f32);
    fn reset(&mut self);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn device_id(&self) -> u64;
}

impl DspProcessor for EqEight {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        if key.starts_with("band_") {
            let parts: Vec<&str> = key.split('_').collect();
            if parts.len() >= 3 {
                if let Ok(band_num) = parts[1].parse::<usize>() {
                    if (1..=8).contains(&band_num) {
                        let band_idx = band_num - 1;
                        match parts[2] {
                            "freq" => {
                                if let Some(band) = self.bands.get_mut(band_idx) {
                                    band.freq = value;
                                    band.filter.set_params(band.freq, band.gain_db, band.q);
                                }
                            }
                            "gain" => {
                                if let Some(band) = self.bands.get_mut(band_idx) {
                                    band.gain_db = value;
                                    band.filter.set_params(band.freq, band.gain_db, band.q);
                                }
                            }
                            "q" => {
                                if let Some(band) = self.bands.get_mut(band_idx) {
                                    band.q = value;
                                    band.filter.set_params(band.freq, band.gain_db, band.q);
                                }
                            }
                            "enabled" => {
                                self.set_band_enabled(band_idx, value > 0.5);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        for b in &mut self.bands {
            b.filter.reset_state();
        }
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for CompressorEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "threshold_db" => self.threshold_db = value,
            "ratio" => self.ratio = value.max(1.0),
            "attack_ms" => self.attack_ms = value.max(0.1),
            "release_ms" => self.release_ms = value.max(1.0),
            "makeup_gain_db" => self.makeup_gain_db = value,
            "knee_db" => self.knee_db = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.gain_reduction_db = 0.0;
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for DelayEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "time_ms" | "delay_time_ms" => self.delay_time_ms = value.clamp(1.0, 1900.0),
            "feedback" => self.feedback = value.clamp(0.0, 0.98),
            "dry_wet" => self.dry_wet = value.clamp(0.0, 1.0),
            "ping_pong" => self.ping_pong = value > 0.5,
            _ => {}
        }
    }

    fn reset(&mut self) {
        // clear delay buffers if needed
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for ReverbEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "room_size" => {
                self.room_size = value.clamp(0.0, 0.98);
                self.set_params(self.room_size, self.damping, self.dry_wet);
            }
            "damping" => {
                self.damping = value.clamp(0.0, 1.0);
                self.set_params(self.room_size, self.damping, self.dry_wet);
            }
            "dry_wet" => {
                self.dry_wet = value.clamp(0.0, 1.0);
                self.set_params(self.room_size, self.damping, self.dry_wet);
            }
            "stereo_width" => self.stereo_width = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for LimiterEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "ceiling_db" => self.ceiling_db = value,
            "gain_db" => self.gain_db = value,
            "release_ms" => self.release_ms = value.clamp(1.0, 1000.0),
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for UtilityEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        UtilityEffect::process_stereo(self, in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "gain_db" => self.gain_db = value,
            "pan" => self.pan = value.clamp(-1.0, 1.0),
            "width" => self.width = value.clamp(0.0, 4.0),
            "mute" => self.mute = value > 0.5,
            "invert_l" => self.invert_l = value > 0.5,
            "invert_r" => self.invert_r = value > 0.5,
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for SaturatorEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "drive_db" => self.drive_db = value,
            "curve_type" => {
                self.curve = match value.round() as i32 {
                    0 => SaturatorCurve::SoftSine,
                    1 => SaturatorCurve::HardClip,
                    2 => SaturatorCurve::AnalogWarmth,
                    3 => SaturatorCurve::Foldback,
                    _ => SaturatorCurve::SoftSine,
                };
            }
            "output_db" => self.output_db = value,
            "dry_wet" => self.dry_wet = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.tone_filter.reset_state();
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for AutoFilterEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "cutoff" => self.cutoff = value.clamp(20.0, 20000.0),
            "resonance" => self.resonance = value.clamp(0.1, 10.0),
            "lfo_rate" => self.lfo_rate = value.clamp(0.01, 40.0),
            "lfo_depth" => self.lfo_depth = value.clamp(0.0, 1.0),
            "envelope_amount" => self.envelope_amount = value.clamp(0.0, 1.0),
            "filter_type" => {
                self.mode = match value.round() as i32 {
                    0 => AutoFilterMode::LowPass,
                    1 => AutoFilterMode::HighPass,
                    2 => AutoFilterMode::BandPass,
                    3 => AutoFilterMode::Notch,
                    _ => AutoFilterMode::LowPass,
                };
            }
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for ChorusEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.process_stereo(in_l, in_r)
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "rate_hz" => self.rate_hz = value.clamp(0.1, 10.0),
            "depth" => self.depth = value.clamp(0.0, 1.0),
            "feedback" => self.feedback = value.clamp(0.0, 0.75),
            "dry_wet" => self.dry_wet = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for AutoPanEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut left = [in_l];
        let mut right = [in_r];
        self.process(&mut left, &mut right);
        (left[0], right[0])
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "amount" => self.amount = value.clamp(0.0, 1.0),
            "rate_hz" => self.rate_hz = value.clamp(0.01, 40.0),
            "phase_deg" => self.phase_deg = value.clamp(0.0, 360.0),
            "shape" => self.shape = value,
            "mode" => self.mode = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        AutoPanEffect::reset(self);
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for GateEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut left = [in_l];
        let mut right = [in_r];
        self.process(&mut left, &mut right);
        (left[0], right[0])
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "threshold_db" => self.threshold_db = value,
            "return_db" => self.return_db = value,
            "attack_ms" => self.attack_ms = value.max(0.1),
            "hold_ms" => self.hold_ms = value.max(0.0),
            "release_ms" => self.release_ms = value.max(1.0),
            "floor_db" => self.floor_db = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        GateEffect::reset(self);
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for EqThreeEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut left = [in_l];
        let mut right = [in_r];
        self.process(&mut left, &mut right);
        (left[0], right[0])
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "low_gain_db" => self.low_gain_db = value,
            "mid_gain_db" => self.mid_gain_db = value,
            "high_gain_db" => self.high_gain_db = value,
            "low_freq" => {
                self.low_freq = value;
                self.update_filters();
            }
            "high_freq" => {
                self.high_freq = value;
                self.update_filters();
            }
            "low_on" => self.low_on = value > 0.5,
            "mid_on" => self.mid_on = value > 0.5,
            "high_on" => self.high_on = value > 0.5,
            _ => {}
        }
    }

    fn reset(&mut self) {
        EqThreeEffect::reset(self);
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

impl DspProcessor for ReduxEffect {
    fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut left = [in_l];
        let mut right = [in_r];
        self.process(&mut left, &mut right);
        (left[0], right[0])
    }

    fn update_param(&mut self, key: &str, value: f32) {
        match key {
            "rate_factor" => self.rate_factor = value.clamp(1.0, 32.0),
            "bits" => self.bits = value.clamp(1.0, 16.0),
            "jitter" => self.jitter = value.clamp(0.0, 1.0),
            "dry_wet" => self.dry_wet = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        ReduxEffect::reset(self);
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }
}

/// Factory function to create a boxed `DspProcessor` from a data model `Device`.
pub fn create_processor(dev: &Device, sample_rate: f32) -> Option<Box<dyn DspProcessor>> {
    let mut proc: Box<dyn DspProcessor> = match dev.device_type {
        DeviceType::EqEight => {
            let mut eq = EqEight::new(sample_rate);
            eq.device_id = dev.id;
            eq.update_from_device(dev);
            Box::new(eq)
        }
        DeviceType::Compressor => {
            let mut comp = CompressorEffect::new(sample_rate);
            comp.device_id = dev.id;
            comp.update_from_device(dev);
            Box::new(comp)
        }
        DeviceType::Delay => {
            let mut d = DelayEffect::new(sample_rate);
            d.device_id = dev.id;
            d.update_from_device(dev);
            Box::new(d)
        }
        DeviceType::Reverb => {
            let mut r = ReverbEffect::new(sample_rate);
            r.device_id = dev.id;
            r.update_from_device(dev);
            Box::new(r)
        }
        DeviceType::Limiter => {
            let mut l = LimiterEffect::new(sample_rate);
            l.device_id = dev.id;
            l.update_from_device(dev);
            Box::new(l)
        }
        DeviceType::Utility => {
            let mut u = UtilityEffect::new();
            u.device_id = dev.id;
            u.update_from_device(dev);
            Box::new(u)
        }
        DeviceType::Saturator => {
            let mut s = SaturatorEffect::new(sample_rate);
            s.device_id = dev.id;
            s.update_from_device(dev);
            Box::new(s)
        }
        DeviceType::AutoFilter => {
            let mut af = AutoFilterEffect::new(sample_rate);
            af.device_id = dev.id;
            af.update_from_device(dev);
            Box::new(af)
        }
        DeviceType::Chorus => {
            let mut ch = ChorusEffect::new(sample_rate);
            ch.device_id = dev.id;
            ch.update_from_device(dev);
            Box::new(ch)
        }
        DeviceType::AutoPan => {
            let mut ap = AutoPanEffect::new(sample_rate);
            ap.device_id = dev.id;
            ap.is_enabled = dev.is_enabled;
            ap.amount = dev.get_param("amount");
            ap.rate_hz = dev.get_param("rate_hz");
            ap.phase_deg = dev.get_param("phase_deg");
            ap.shape = dev.get_param("shape");
            ap.mode = dev.get_param("mode");
            Box::new(ap)
        }
        DeviceType::Gate => {
            let mut g = GateEffect::new(sample_rate);
            g.device_id = dev.id;
            g.is_enabled = dev.is_enabled;
            g.threshold_db = dev.get_param("threshold_db");
            g.return_db = dev.get_param("return_db");
            g.attack_ms = dev.get_param("attack_ms");
            g.hold_ms = dev.get_param("hold_ms");
            g.release_ms = dev.get_param("release_ms");
            g.floor_db = dev.get_param("floor_db");
            Box::new(g)
        }
        DeviceType::EqThree => {
            let mut eq = EqThreeEffect::new(sample_rate);
            eq.device_id = dev.id;
            eq.is_enabled = dev.is_enabled;
            eq.low_gain_db = dev.get_param("low_gain_db");
            eq.mid_gain_db = dev.get_param("mid_gain_db");
            eq.high_gain_db = dev.get_param("high_gain_db");
            eq.low_freq = dev.get_param("low_freq");
            eq.high_freq = dev.get_param("high_freq");
            eq.low_on = dev.get_param("low_on") > 0.5;
            eq.mid_on = dev.get_param("mid_on") > 0.5;
            eq.high_on = dev.get_param("high_on") > 0.5;
            Box::new(eq)
        }
        DeviceType::Redux => {
            let mut rd = ReduxEffect::new(sample_rate);
            rd.device_id = dev.id;
            rd.is_enabled = dev.is_enabled;
            rd.rate_factor = dev.get_param("rate_factor");
            rd.bits = dev.get_param("bits");
            rd.jitter = dev.get_param("jitter");
            rd.dry_wet = dev.get_param("dry_wet");
            Box::new(rd)
        }
        _ => return None,
    };

    proc.set_enabled(dev.is_enabled);
    for (k, v) in &dev.parameters {
        proc.update_param(k, *v);
    }
    Some(proc)
}
