use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AutomationTarget {
    TrackVolume,
    TrackPan,
    SendLevel(usize),
    DeviceParam { device_id: u64, param_key: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub beat: f64,
    pub value: f32,
    pub curve: f32, // -1.0 to 1.0 curvature
}

impl AutomationPoint {
    pub fn new(beat: f64, value: f32, curve: f32) -> Self {
        Self { beat, value, curve }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationLane {
    pub id: u64,
    pub target: AutomationTarget,
    pub points: Vec<AutomationPoint>,
    pub is_enabled: bool,
}

impl AutomationLane {
    pub fn new(id: u64, target: AutomationTarget) -> Self {
        Self {
            id,
            target,
            points: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn evaluate_at(&self, beat: f64) -> Option<f32> {
        if self.points.is_empty() || !self.is_enabled {
            return None;
        }
        if beat <= self.points[0].beat {
            return Some(self.points[0].value);
        }
        if beat >= self.points.last().unwrap().beat {
            return Some(self.points.last().unwrap().value);
        }
        // Find surrounding segment and interpolate
        for w in self.points.windows(2) {
            if beat >= w[0].beat && beat <= w[1].beat {
                let span = w[1].beat - w[0].beat;
                if span <= 1e-6 {
                    return Some(w[0].value);
                }
                let t = (beat - w[0].beat) / span;
                let factor = if w[0].curve.abs() > 1e-4 {
                    let exp = (-w[0].curve as f64 * 2.0).exp();
                    t.powf(exp)
                } else {
                    t
                };
                return Some(w[0].value + (w[1].value - w[0].value) * factor as f32);
            }
        }
        None
    }
}
