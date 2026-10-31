use crate::ids::DeviceId;
use crate::param::{ParamSpec, ParamValues, param_id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    EqEight,
    Delay,
    Reverb,
    Compressor,
    Limiter,
    Utility,
    Operator,
    Simpler,
    Saturator,
    DrumRack,
    AutoFilter,
    Chorus,
    AutoPan,
    Gate,
    EqThree,
    Redux,
    Custom(String),
}

const fn with_default(name: &'static str, default: f32) -> ParamSpec {
    ParamSpec {
        name,
        default: Some(default),
    }
}

const fn without_default(name: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        default: None,
    }
}

// Each table is append only: a parameter's id is its position here, and the JSON export
// writes the names. Parameters without a default exist only once they are set.
const EQ_EIGHT: &[ParamSpec] = &[
    with_default("band_1_freq", 100.0),
    with_default("band_1_gain", 0.0),
    with_default("band_1_q", 0.707),
    with_default("band_1_enabled", 1.0),
    with_default("band_2_freq", 200.0),
    with_default("band_2_gain", 0.0),
    with_default("band_2_q", 0.707),
    with_default("band_2_enabled", 1.0),
    with_default("band_3_freq", 400.0),
    with_default("band_3_gain", 0.0),
    with_default("band_3_q", 0.707),
    with_default("band_3_enabled", 1.0),
    with_default("band_4_freq", 800.0),
    with_default("band_4_gain", 0.0),
    with_default("band_4_q", 0.707),
    with_default("band_4_enabled", 1.0),
    with_default("band_5_freq", 1600.0),
    with_default("band_5_gain", 0.0),
    with_default("band_5_q", 0.707),
    with_default("band_5_enabled", 1.0),
    with_default("band_6_freq", 3200.0),
    with_default("band_6_gain", 0.0),
    with_default("band_6_q", 0.707),
    with_default("band_6_enabled", 1.0),
    with_default("band_7_freq", 6400.0),
    with_default("band_7_gain", 0.0),
    with_default("band_7_q", 0.707),
    with_default("band_7_enabled", 1.0),
    with_default("band_8_freq", 12800.0),
    with_default("band_8_gain", 0.0),
    with_default("band_8_q", 0.707),
    with_default("band_8_enabled", 1.0),
];

const DELAY: &[ParamSpec] = &[
    with_default("time_ms", 375.0),
    with_default("feedback", 0.35),
    with_default("dry_wet", 0.30),
    with_default("ping_pong", 1.0),
];

const REVERB: &[ParamSpec] = &[
    with_default("decay_time_sec", 2.2),
    with_default("room_size", 0.65),
    with_default("damping", 0.4),
    with_default("dry_wet", 0.25),
    with_default("stereo_width", 1.0),
];

const COMPRESSOR: &[ParamSpec] = &[
    with_default("threshold_db", -18.0),
    with_default("ratio", 4.0),
    with_default("attack_ms", 15.0),
    with_default("release_ms", 100.0),
    with_default("makeup_gain_db", 2.0),
];

const LIMITER: &[ParamSpec] = &[
    with_default("ceiling_db", -0.3),
    with_default("gain_db", 0.0),
    with_default("release_ms", 150.0),
];

const UTILITY: &[ParamSpec] = &[
    with_default("gain_db", 0.0),
    with_default("pan", 0.0),
    with_default("width", 1.0),
    with_default("mute", 0.0),
];

const OPERATOR: &[ParamSpec] = &[
    with_default("osc1_waveform", 0.0),
    with_default("osc1_level", 1.0),
    with_default("osc1_coarse", 1.0),
    with_default("osc2_waveform", 1.0),
    with_default("osc2_level", 0.5),
    with_default("osc2_coarse", 2.0),
    with_default("filter_cutoff", 2400.0),
    with_default("filter_resonance", 0.3),
    with_default("attack_ms", 5.0),
    with_default("decay_ms", 120.0),
    with_default("sustain", 0.7),
    with_default("release_ms", 300.0),
    without_default("algorithm"),
    without_default("feedback"),
];

const SIMPLER: &[ParamSpec] = &[
    with_default("playback_mode", 0.0),
    with_default("gain_db", 0.0),
    with_default("attack_ms", 2.0),
    with_default("decay_ms", 50.0),
    with_default("sustain", 1.0),
    with_default("release_ms", 100.0),
    with_default("filter_cutoff", 20000.0),
    without_default("start_pct"),
    without_default("length_pct"),
    without_default("crossfade_pct"),
    without_default("filter_resonance"),
];

const DRUM_RACK: &[ParamSpec] = &[without_default("gain_db")];

const SATURATOR: &[ParamSpec] = &[
    with_default("drive_db", 6.0),
    with_default("curve_type", 0.0),
    with_default("output_db", -2.0),
    with_default("dry_wet", 1.0),
    with_default("tone", 0.5),
];

const AUTO_FILTER: &[ParamSpec] = &[
    with_default("filter_type", 0.0),
    with_default("cutoff", 1200.0),
    with_default("resonance", 1.5),
    with_default("lfo_rate", 1.0),
    with_default("lfo_depth", 0.4),
    with_default("envelope_amount", 0.3),
];

const CHORUS: &[ParamSpec] = &[
    with_default("rate_hz", 1.2),
    with_default("depth", 0.5),
    with_default("feedback", 0.2),
    with_default("dry_wet", 0.5),
];

const AUTO_PAN: &[ParamSpec] = &[
    with_default("amount", 1.0),
    with_default("rate_hz", 1.0),
    with_default("phase_deg", 180.0),
    with_default("shape", 0.0),
    with_default("mode", 0.0),
];

const GATE: &[ParamSpec] = &[
    with_default("threshold_db", -30.0),
    with_default("return_db", 3.0),
    with_default("attack_ms", 1.0),
    with_default("hold_ms", 50.0),
    with_default("release_ms", 100.0),
    with_default("floor_db", -60.0),
];

const EQ_THREE: &[ParamSpec] = &[
    with_default("low_gain_db", 0.0),
    with_default("mid_gain_db", 0.0),
    with_default("high_gain_db", 0.0),
    with_default("low_freq", 250.0),
    with_default("high_freq", 2500.0),
    with_default("low_on", 1.0),
    with_default("mid_on", 1.0),
    with_default("high_on", 1.0),
];

const REDUX: &[ParamSpec] = &[
    with_default("rate_factor", 1.0),
    with_default("bits", 16.0),
    with_default("jitter", 0.0),
    with_default("dry_wet", 1.0),
];

/// The parameter table of a device kind. Kinds without parameters have an empty table.
pub fn param_table(device_type: &DeviceType) -> &'static [ParamSpec] {
    match device_type {
        DeviceType::EqEight => EQ_EIGHT,
        DeviceType::Delay => DELAY,
        DeviceType::Reverb => REVERB,
        DeviceType::Compressor => COMPRESSOR,
        DeviceType::Limiter => LIMITER,
        DeviceType::Utility => UTILITY,
        DeviceType::Operator => OPERATOR,
        DeviceType::Simpler => SIMPLER,
        DeviceType::Saturator => SATURATOR,
        DeviceType::AutoFilter => AUTO_FILTER,
        DeviceType::Chorus => CHORUS,
        DeviceType::AutoPan => AUTO_PAN,
        DeviceType::Gate => GATE,
        DeviceType::EqThree => EQ_THREE,
        DeviceType::Redux => REDUX,
        DeviceType::DrumRack => DRUM_RACK,
        DeviceType::Custom(_) => &[],
    }
}

/// A device on a track. Values are stored by parameter id. `get_param` and `set_param` take the
/// names of M01, which are the adapter until the engine moves to ids (M05).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "DeviceRepr", into = "DeviceRepr")]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub device_type: DeviceType,
    pub is_enabled: bool,
    pub parameters: ParamValues,
}

impl Device {
    pub fn new(id: DeviceId, name: impl Into<String>, device_type: DeviceType) -> Self {
        let parameters = ParamValues::from_defaults(param_table(&device_type));
        Self {
            id,
            name: name.into(),
            device_type,
            is_enabled: true,
            parameters,
        }
    }

    /// The stored values by parameter name, in table order.
    pub fn named_params(&self) -> impl Iterator<Item = (&'static str, f32)> + '_ {
        let table = param_table(&self.device_type);
        self.parameters.iter().filter_map(move |(id, value)| {
            table.get(usize::from(id.0)).map(|spec| (spec.name, value))
        })
    }

    /// The value of a parameter by name, or `None` when the device has no value for it.
    pub fn param(&self, name: &str) -> Option<f32> {
        let table = param_table(&self.device_type);
        param_id(table, name).and_then(|id| self.parameters.get(id))
    }

    /// The value of a parameter by name, or 0.0 when the device has no value for it.
    pub fn get_param(&self, name: &str) -> f32 {
        self.param(name).unwrap_or(0.0)
    }

    /// Sets a parameter by name. A name outside the device kind's table is ignored.
    pub fn set_param(&mut self, name: impl AsRef<str>, value: f32) {
        let table = param_table(&self.device_type);
        if let Some(id) = param_id(table, name.as_ref()) {
            self.parameters.set(id, value);
        }
    }
}

/// The exported shape of a device: parameters by name, as M01 wrote them.
#[derive(Serialize, Deserialize)]
struct DeviceRepr {
    id: DeviceId,
    name: String,
    device_type: DeviceType,
    is_enabled: bool,
    parameters: BTreeMap<String, f32>,
}

impl From<Device> for DeviceRepr {
    fn from(device: Device) -> Self {
        let parameters = device
            .named_params()
            .map(|(name, value)| (name.to_string(), value))
            .collect();
        Self {
            id: device.id,
            name: device.name,
            device_type: device.device_type,
            is_enabled: device.is_enabled,
            parameters,
        }
    }
}

impl From<DeviceRepr> for Device {
    fn from(repr: DeviceRepr) -> Self {
        let mut device = Self {
            id: repr.id,
            name: repr.name,
            device_type: repr.device_type,
            is_enabled: repr.is_enabled,
            parameters: ParamValues::default(),
        };
        for (name, value) in &repr.parameters {
            device.set_param(name, *value);
        }
        device
    }
}
