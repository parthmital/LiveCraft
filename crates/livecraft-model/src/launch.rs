//! Clip launch settings, Follow Actions, and the metronome settings (M07; manual 16, 19.4.1, 19.6).
//!
//! These types only describe the settings. The engine's launch scheduler and metronome read them.

use livecraft_time::{Grid, LaunchQuantization};
use serde::{Deserialize, Serialize};

/// How a clip responds to the down and up of a launch (manual 16.2, page 361).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchMode {
    /// "Trigger: down starts the clip; up is ignored."
    #[default]
    Trigger,
    /// "Gate: down starts the clip; up stops the clip."
    Gate,
    /// "Toggle: down starts the clip; up is ignored. The clip will stop on the next down."
    Toggle,
    /// "Repeat: As long as the mouse switch/key is held, the clip is triggered repeatedly at the clip
    /// quantization rate."
    Repeat,
}

/// A launch quantization setting as the Set stores it (manual 16.4, page 363). It mirrors
/// `livecraft_time::LaunchQuantization`, which has no serde form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantization {
    None,
    /// The Control Bar's Global Quantization.
    #[default]
    Global,
    /// Every `n` bars.
    Bars(u32),
    /// A note value `num / den` of a whole note, two thirds of it when `triplet` is set.
    Note {
        num: u32,
        den: u32,
        triplet: bool,
    },
}

impl Quantization {
    /// The setting as the time crate computes with it.
    pub fn to_launch(self) -> LaunchQuantization {
        match self {
            Quantization::None => LaunchQuantization::None,
            Quantization::Global => LaunchQuantization::Global,
            Quantization::Bars(bars) => LaunchQuantization::Grid(Grid::Bars(bars)),
            Quantization::Note { num, den, triplet } => {
                LaunchQuantization::Grid(Grid::Fraction { num, den, triplet })
            }
        }
    }

    /// The grid of a setting that is not `Global`; `None` for no quantization. The Control Bar's
    /// setting is never `Global`, so this is its grid.
    pub fn grid(self) -> Grid {
        match self {
            Quantization::None | Quantization::Global => Grid::None,
            Quantization::Bars(bars) => Grid::Bars(bars),
            Quantization::Note { num, den, triplet } => Grid::Fraction { num, den, triplet },
        }
    }
}

/// The launch settings of a Session View clip (manual 16.1 to 16.5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipLaunch {
    pub mode: LaunchMode,
    pub quantization: Quantization,
    /// "When a clip in Legato Mode is launched, it takes over the play position from whatever clip was
    /// played in that track before." (16.3, page 362)
    pub legato: bool,
    /// From 0 to 1. "If set to zero, there is no influence; at 100 percent, the softest notes play the
    /// clip silently." (16.5, page 364)
    pub velocity_amount: f32,
}

impl Default for ClipLaunch {
    fn default() -> Self {
        Self {
            mode: LaunchMode::Trigger,
            quantization: Quantization::Global,
            legato: false,
            velocity_amount: 0.0,
        }
    }
}

/// One of the ten Follow Actions (manual 16.7, page 367).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FollowAction {
    #[default]
    NoAction,
    Stop,
    PlayAgain,
    Previous,
    Next,
    First,
    Last,
    Any,
    Other,
    /// Jumps to the clip slot at this row, counted from 0.
    Jump(u32),
}

/// A clip's Follow Actions (manual 16.7, pages 366 to 368).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowActions {
    /// "This button is disabled by default."
    pub enabled: bool,
    pub action_a: FollowAction,
    pub action_b: FollowAction,
    /// Chance A and Chance B, each from 0 to 1.
    pub chance_a: f32,
    pub chance_b: f32,
    /// "This switch is set to Linked by default. In Linked mode, the Follow Action is triggered at the
    /// end of the clip, or after the number of loops set in the Follow Action Multiplier field."
    pub linked: bool,
    /// The Follow Action Multiplier: how many times the clip plays before a linked Follow Action.
    pub multiplier: u32,
    /// The Follow Action Time in beats, used when unlinked. "The default for this setting is one bar."
    pub time_beats: f64,
}

impl Default for FollowActions {
    fn default() -> Self {
        Self {
            enabled: false,
            action_a: FollowAction::Next,
            action_b: FollowAction::NoAction,
            chance_a: 1.0,
            chance_b: 0.0,
            linked: true,
            multiplier: 1,
            time_beats: 4.0,
        }
    }
}

/// The beat division the metronome ticks at (manual 19.4.1, page 419).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetronomeRhythm {
    /// "With the default "Auto" setting, the tick interval follows the time signature's denominator."
    #[default]
    Auto,
    /// A tick every `1 / den` of a whole note, two thirds of it when `triplet` is set.
    Division { den: u32, triplet: bool },
}

/// The sound of the metronome's tick. The clicks are synthesised, so no sample from Live is used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetronomeSound {
    #[default]
    Classic,
    Click,
    Wood,
}

/// The Metronome Settings menu (manual 19.4.1, page 419; 19.6, page 420).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metronome {
    /// The count-in length in bars; 0 is "None".
    pub count_in_bars: u8,
    pub rhythm: MetronomeRhythm,
    pub sound: MetronomeSound,
    /// "When Enable Only While Recording is checked and the metronome is activated, the metronome will be
    /// highlighted while transport is running, but will only be audible while recording."
    pub only_while_recording: bool,
    /// The click level in dB. Live sets it with the Preview Volume knob (page 418).
    pub volume_db: f32,
}

impl Metronome {
    /// The count-in lengths the menu offers, in bars.
    pub const COUNT_IN_BARS: [u8; 4] = [0, 1, 2, 4];
}

impl Default for Metronome {
    fn default() -> Self {
        Self {
            count_in_bars: 0,
            rhythm: MetronomeRhythm::Auto,
            sound: MetronomeSound::Classic,
            only_while_recording: false,
            volume_db: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_settings_default_to_trigger_global_and_no_velocity() {
        let launch = ClipLaunch::default();
        assert_eq!(launch.mode, LaunchMode::Trigger);
        assert_eq!(launch.quantization, Quantization::Global);
        assert!(!launch.legato);
        assert_eq!(launch.velocity_amount, 0.0);
    }

    #[test]
    fn follow_actions_are_disabled_linked_and_one_bar_by_default() {
        // "This button is disabled by default." "This switch is set to Linked by default." "The default
        // for this setting is one bar." (manual 16.7, page 366)
        let follow = FollowActions::default();
        assert!(!follow.enabled);
        assert!(follow.linked);
        assert_eq!(follow.time_beats, 4.0);
    }

    #[test]
    fn quantization_converts_to_the_time_crates_setting() {
        assert_eq!(Quantization::None.to_launch(), LaunchQuantization::None);
        assert_eq!(Quantization::Global.to_launch(), LaunchQuantization::Global);
        assert_eq!(
            Quantization::Bars(2).to_launch(),
            LaunchQuantization::Grid(Grid::Bars(2))
        );
        assert_eq!(
            Quantization::Note {
                num: 1,
                den: 16,
                triplet: true
            }
            .grid(),
            Grid::Fraction {
                num: 1,
                den: 16,
                triplet: true
            }
        );
    }

    #[test]
    fn a_clip_saved_without_launch_settings_loads_the_defaults() {
        let launch: ClipLaunch = serde_json::from_str("{}").expect("load empty settings");
        assert_eq!(launch, ClipLaunch::default());
        let follow: FollowActions =
            serde_json::from_str(r#"{"enabled": true}"#).expect("load partial settings");
        assert!(follow.enabled && follow.linked);
    }
}
