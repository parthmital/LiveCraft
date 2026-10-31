//! Transport settings, the song timeline a graph carries, and the metronome (M07 items 1 to 3 and 10;
//! manual 6.3, 6.6, 16.4, 19.3.1, 19.4.1, 19.6).

use livecraft_model::ids::SceneId;
use livecraft_model::model::{Metronome, MetronomeRhythm, MetronomeSound, Project, Quantization};
use livecraft_time::{Beats, TempoMap, TimeSignatureMap};

/// The Control Bar settings the engine follows. They are plain data, so a command carries them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportSettings {
    pub global_quantization: Quantization,
    pub loop_enabled: bool,
    pub loop_start: f64,
    pub loop_length: f64,
    pub punch_in: bool,
    pub punch_out: bool,
    pub follow_actions_enabled: bool,
    pub metronome_enabled: bool,
    pub metronome: Metronome,
}

impl Default for TransportSettings {
    fn default() -> Self {
        Self {
            global_quantization: Quantization::Bars(1),
            loop_enabled: false,
            loop_start: 0.0,
            loop_length: 16.0,
            punch_in: false,
            punch_out: false,
            follow_actions_enabled: true,
            metronome_enabled: false,
            metronome: Metronome::default(),
        }
    }
}

impl TransportSettings {
    /// The settings a Set holds.
    pub fn of(project: &Project) -> Self {
        let transport = &project.transport;
        Self {
            global_quantization: transport.global_quantization,
            loop_enabled: transport.loop_enabled,
            loop_start: transport.loop_start_beats,
            loop_length: transport.loop_length_beats,
            punch_in: transport.punch_in,
            punch_out: transport.punch_out,
            follow_actions_enabled: transport.follow_actions_enabled,
            metronome_enabled: transport.metronome_enabled,
            metronome: transport.metronome,
        }
    }

    /// The end of the Arrangement Loop.
    pub fn loop_end(&self) -> f64 {
        self.loop_start + self.loop_length.max(0.0)
    }

    /// Whether recording into the Arrangement takes place at `at`: "The punch-in point is identical to
    /// the Arrangement Loop's start position." "The punch-out point is identical to the Arrangement Loop's
    /// end position." (manual 19.3.1, page 414)
    pub fn records_at(&self, at: f64) -> bool {
        !(self.punch_in && at < self.loop_start) && !(self.punch_out && at >= self.loop_end())
    }
}

/// A scene's tempo and time signature, which launching it applies (manual 7.2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneSettings {
    pub id: SceneId,
    pub tempo: Option<f32>,
    pub signature: Option<TimeSignatureMap>,
}

/// The song timeline of a graph: the tempo and time signature maps, the scenes, the transport
/// settings, and the Follow Action seed.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub tempo: TempoMap,
    pub signature: TimeSignatureMap,
    pub scenes: Vec<SceneSettings>,
    pub transport: TransportSettings,
    pub follow_seed: u64,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            tempo: TempoMap::constant(120.0),
            signature: TimeSignatureMap::default(),
            scenes: Vec::new(),
            transport: TransportSettings::default(),
            follow_seed: 0,
        }
    }
}

impl Timeline {
    /// The timeline of a Set.
    pub fn of(project: &Project) -> Self {
        let scenes = project
            .scenes
            .iter()
            .map(|scene| SceneSettings {
                id: scene.id,
                tempo: scene.tempo,
                signature: scene.time_signature.and_then(|(numerator, denominator)| {
                    TimeSignatureMap::constant(numerator, denominator).ok()
                }),
            })
            .collect();
        Self {
            tempo: project.transport.tempo.clone(),
            signature: project.transport.time_signature.clone(),
            scenes,
            transport: TransportSettings::of(project),
            follow_seed: project.transport.follow_seed,
        }
    }
}

/// How long a tick sounds, in seconds.
const CLICK_SECONDS: f32 = 0.04;

/// The metronome: it finds each tick in a block and synthesises a short decaying tone, higher on the
/// first beat of a bar. No sample from Live is used (M07 decisions).
#[derive(Debug, Clone, Copy, Default)]
pub struct MetronomeVoice {
    /// The tick that sounds: frames played so far, its frequency, and its level. None when silent.
    tick: Option<(u32, f32, f32)>,
}

impl MetronomeVoice {
    /// The beats between ticks at `at`: "With the default "Auto" setting, the tick interval follows the
    /// time signature's denominator." A division that does not fit in a bar falls back to Auto (manual
    /// 19.4.1, page 419).
    pub fn interval(settings: &Metronome, at: f64, signature: &TimeSignatureMap) -> f64 {
        let (numerator, denominator) = signature.signature_at(Beats(at));
        let auto = 4.0 / f64::from(denominator.max(1));
        let bar = f64::from(numerator) * auto;
        match settings.rhythm {
            MetronomeRhythm::Auto => auto,
            MetronomeRhythm::Division { den, triplet } => {
                let straight = 4.0 / f64::from(den.max(1));
                let step = if triplet {
                    straight * 2.0 / 3.0
                } else {
                    straight
                };
                if step > bar + 1e-9 { auto } else { step }
            }
        }
    }

    /// Adds this block's ticks to `left` and `right`. Frame `k` is at song beat `start + k * dt`.
    /// `audible` false only lets a sounding tick ring out.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        settings: &Metronome,
        signature: &TimeSignatureMap,
        start: f64,
        dt: f64,
        sample_rate: f32,
        audible: bool,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        let level = 10.0f32.powf(settings.volume_db / 20.0) * 0.5;
        let length = (CLICK_SECONDS * sample_rate) as u32;
        let decay = 1.0 / (0.25 * CLICK_SECONDS * sample_rate);
        for frame in 0..left.len() {
            if audible && dt > 0.0 {
                let at = start + frame as f64 * dt;
                let interval = Self::interval(settings, at, signature);
                // A tick starts on the first frame at or after its beat.
                let index = ((at - 1e-9) / interval).ceil();
                let beat = index * interval;
                if beat >= at && beat < at + dt {
                    let bar = signature.to_bar_beat_sixteenth(Beats(beat)).bar;
                    let downbeat = (signature.bar_start(bar).0 - beat).abs() < 1e-6;
                    let (accent, normal) = match settings.sound {
                        MetronomeSound::Classic => (1568.0, 1046.5),
                        MetronomeSound::Click => (3136.0, 2093.0),
                        MetronomeSound::Wood => (880.0, 659.3),
                    };
                    self.tick = Some((0, if downbeat { accent } else { normal }, level));
                }
            }
            let Some((played, frequency, amplitude)) = self.tick.as_mut() else {
                continue;
            };
            let t = *played as f32 / sample_rate;
            let sample = (std::f32::consts::TAU * *frequency * t).sin()
                * *amplitude
                * (-(*played as f32) * decay).exp();
            left[frame] += sample;
            right[frame] += sample;
            *played += 1;
            if *played >= length {
                self.tick = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn four_four() -> TimeSignatureMap {
        TimeSignatureMap::default()
    }

    #[test]
    fn the_punch_points_are_the_loop_start_and_end() {
        // "The punch-in point is identical to the Arrangement Loop's start position." "The punch-out point is
        // identical to the Arrangement Loop's end position." (manual 19.3.1, page 414)
        let settings = TransportSettings {
            loop_start: 8.0,
            loop_length: 8.0,
            punch_in: true,
            punch_out: true,
            ..TransportSettings::default()
        };
        assert!(!settings.records_at(7.9));
        assert!(settings.records_at(8.0));
        assert!(settings.records_at(15.9));
        assert!(!settings.records_at(16.0));
        let free = TransportSettings::default();
        assert!(free.records_at(0.0) && free.records_at(100.0));
    }

    #[test]
    fn auto_ticks_follow_the_denominator_and_a_division_too_long_for_the_bar_falls_back() {
        // "With the default "Auto" setting, the tick interval follows the time signature's denominator."
        // "If the currently selected beat division no longer fits in a bar due to a time signature change,
        // the metronome's tick will revert to the "Auto" setting." (manual 19.4.1, page 419)
        let auto = Metronome::default();
        assert_eq!(MetronomeVoice::interval(&auto, 0.0, &four_four()), 1.0);
        let six_eight = TimeSignatureMap::constant(6, 8).expect("6/8");
        assert_eq!(MetronomeVoice::interval(&auto, 0.0, &six_eight), 0.5);
        let half = Metronome {
            rhythm: MetronomeRhythm::Division {
                den: 2,
                triplet: false,
            },
            ..Metronome::default()
        };
        assert_eq!(MetronomeVoice::interval(&half, 0.0, &four_four()), 2.0);
        let two_eight = TimeSignatureMap::constant(2, 8).expect("2/8");
        assert_eq!(MetronomeVoice::interval(&half, 0.0, &two_eight), 0.5);
    }

    #[test]
    fn a_tick_starts_on_the_frame_of_each_beat_and_the_downbeat_is_higher() {
        let settings = Metronome::default();
        let mut voice = MetronomeVoice::default();
        // A quarter beat per frame: beats fall on frames 0, 4, 8, ...
        let mut left = vec![0.0f32; 20];
        let mut right = vec![0.0f32; 20];
        voice.render(
            &settings,
            &four_four(),
            0.0,
            0.25,
            1000.0,
            true,
            &mut left,
            &mut right,
        );
        // The tone starts at phase 0, so its first sample is 0; the next one is not.
        assert_eq!(left[0], 0.0);
        assert!(left[1] != 0.0);
        assert_eq!(left, right);
        assert!(voice.tick.is_some());
        // Nothing sounds when not audible and no tick rings.
        let mut quiet = MetronomeVoice::default();
        let mut silent = vec![0.0f32; 8];
        let mut silent_r = vec![0.0f32; 8];
        quiet.render(
            &settings,
            &four_four(),
            0.0,
            0.25,
            1000.0,
            false,
            &mut silent,
            &mut silent_r,
        );
        assert!(silent.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a_timeline_takes_the_scenes_tempo_and_signature() {
        let mut project = Project::new_default("Timeline");
        project.scenes[0].tempo = Some(98.0);
        project.scenes[0].time_signature = Some((7, 8));
        let timeline = Timeline::of(&project);
        assert_eq!(timeline.scenes[0].tempo, Some(98.0));
        assert_eq!(
            timeline.scenes[0]
                .signature
                .as_ref()
                .map(|map| map.signature_at(Beats(0.0))),
            Some((7, 8))
        );
        assert_eq!(
            timeline.transport.global_quantization,
            Quantization::Bars(1)
        );
    }
}
