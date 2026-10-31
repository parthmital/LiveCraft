//! Launch and transport settings (M07): clip launch settings and Follow Actions, scene tempo and time
//! signature, Global Quantization, punch, the global Follow Action switch, and the Metronome Settings.

use crate::command::{CommandError, Operation};
use crate::commands::clip::clip_mut;
use crate::registry::CommandRegistry;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, IdAllocator, SceneId};
use livecraft_model::model::{ClipLaunch, FollowActions, Metronome, Project, Quantization, Scene};
use livecraft_time::{MAX_BPM, MIN_BPM, TimeSignatureMap};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<SetClipLaunch>();
    registry.register::<SetClipFollowActions>();
    registry.register::<SetSceneTempo>();
    registry.register::<SetSceneTimeSignature>();
    registry.register::<SetGlobalQuantization>();
    registry.register::<SetPunch>();
    registry.register::<SetFollowActionsEnabled>();
    registry.register::<SetMetronomeSettings>();
}

/// Fails unless `value` is between `min` and `max`. NaN fails, because no range contains it.
fn within(what: &'static str, value: f64, min: f64, max: f64) -> Result<(), CommandError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(CommandError::OutOfRange {
            what,
            min,
            max,
            value,
        })
    }
}

/// Fails for a quantization the Set cannot hold: zero bars, or a note value with a zero part.
fn check_quantization(quantization: Quantization) -> Result<(), CommandError> {
    match quantization {
        Quantization::Bars(0) => Err(CommandError::NotPositive {
            what: "quantization bars",
            value: 0.0,
        }),
        Quantization::Note { num, den, .. } if num == 0 || den == 0 => {
            Err(CommandError::NotPositive {
                what: "quantization note value",
                value: 0.0,
            })
        }
        _ => Ok(()),
    }
}

/// Sets a clip's launch settings (manual 16.1 to 16.5).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetClipLaunch {
    pub clip: ClipId,
    pub launch: ClipLaunch,
    #[serde(skip)]
    previous: Option<ClipLaunch>,
}

impl SetClipLaunch {
    pub fn new(clip: ClipId, launch: ClipLaunch) -> Self {
        Self {
            clip,
            launch,
            previous: None,
        }
    }
}

impl Operation for SetClipLaunch {
    const NAME: &'static str = "clip.set_launch";
    const LABEL: &'static str = "Change Clip Launch Settings";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        within(
            "velocity amount",
            f64::from(self.launch.velocity_amount),
            0.0,
            1.0,
        )?;
        check_quantization(self.launch.quantization)?;
        let clip = clip_mut(project, self.clip)?;
        self.previous = Some(std::mem::replace(&mut clip.launch, self.launch));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(launch), Ok(clip)) = (self.previous.take(), clip_mut(project, self.clip)) {
            clip.launch = launch;
        }
    }
}

/// Sets a clip's Follow Actions (manual 16.7).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetClipFollowActions {
    pub clip: ClipId,
    pub follow: FollowActions,
    #[serde(skip)]
    previous: Option<FollowActions>,
}

impl SetClipFollowActions {
    pub fn new(clip: ClipId, follow: FollowActions) -> Self {
        Self {
            clip,
            follow,
            previous: None,
        }
    }
}

impl Operation for SetClipFollowActions {
    const NAME: &'static str = "clip.set_follow_actions";
    const LABEL: &'static str = "Change Follow Actions";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        within("Chance A", f64::from(self.follow.chance_a), 0.0, 1.0)?;
        within("Chance B", f64::from(self.follow.chance_b), 0.0, 1.0)?;
        if self.follow.multiplier == 0 {
            return Err(CommandError::NotPositive {
                what: "Follow Action Multiplier",
                value: 0.0,
            });
        }
        within(
            "Follow Action Time",
            self.follow.time_beats,
            0.0,
            f64::from(u32::MAX),
        )?;
        let clip = clip_mut(project, self.clip)?;
        self.previous = Some(std::mem::replace(&mut clip.follow, self.follow));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(follow), Ok(clip)) = (self.previous.take(), clip_mut(project, self.clip)) {
            clip.follow = follow;
        }
    }
}

fn scene_mut(project: &mut Project, scene: SceneId) -> Result<&mut Scene, CommandError> {
    project
        .scenes
        .iter_mut()
        .find(|candidate| candidate.id == scene)
        .ok_or(CommandError::Edit(EditError::NoSuchScene(scene)))
}

/// Sets or clears a scene's tempo, which launching the scene applies (manual 7.2.1).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetSceneTempo {
    pub scene: SceneId,
    pub tempo: Option<f32>,
    #[serde(skip)]
    previous: Option<Option<f32>>,
}

impl SetSceneTempo {
    pub fn new(scene: SceneId, tempo: Option<f32>) -> Self {
        Self {
            scene,
            tempo,
            previous: None,
        }
    }
}

impl Operation for SetSceneTempo {
    const NAME: &'static str = "scene.set_tempo";
    const LABEL: &'static str = "Change Scene Tempo";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if let Some(tempo) = self.tempo {
            within("scene tempo", f64::from(tempo), MIN_BPM, MAX_BPM)?;
        }
        let scene = scene_mut(project, self.scene)?;
        self.previous = Some(std::mem::replace(&mut scene.tempo, self.tempo));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(tempo), Ok(scene)) = (self.previous.take(), scene_mut(project, self.scene)) {
            scene.tempo = tempo;
        }
    }
}

/// Sets or clears a scene's time signature, which launching the scene applies (manual 7.2.1).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetSceneTimeSignature {
    pub scene: SceneId,
    pub time_signature: Option<(u8, u8)>,
    #[serde(skip)]
    previous: Option<Option<(u8, u8)>>,
}

impl SetSceneTimeSignature {
    pub fn new(scene: SceneId, time_signature: Option<(u8, u8)>) -> Self {
        Self {
            scene,
            time_signature,
            previous: None,
        }
    }
}

impl Operation for SetSceneTimeSignature {
    const NAME: &'static str = "scene.set_time_signature";
    const LABEL: &'static str = "Change Scene Time Signature";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if let Some((numerator, denominator)) = self.time_signature {
            TimeSignatureMap::constant(numerator, denominator)?;
        }
        let scene = scene_mut(project, self.scene)?;
        self.previous = Some(std::mem::replace(
            &mut scene.time_signature,
            self.time_signature,
        ));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(signature), Ok(scene)) = (self.previous.take(), scene_mut(project, self.scene))
        {
            scene.time_signature = signature;
        }
    }
}

/// Sets the Control Bar's Global Quantization (manual 16.4). It cannot be `Global` itself.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetGlobalQuantization {
    pub quantization: Quantization,
    #[serde(skip)]
    previous: Option<Quantization>,
}

impl SetGlobalQuantization {
    pub fn new(quantization: Quantization) -> Self {
        Self {
            quantization,
            previous: None,
        }
    }
}

impl Operation for SetGlobalQuantization {
    const NAME: &'static str = "transport.set_quantization";
    const LABEL: &'static str = "Change Global Quantization";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if self.quantization == Quantization::Global {
            return Err(CommandError::BadArguments {
                name: Self::NAME.to_string(),
                message: "the Global Quantization cannot be Global".to_string(),
            });
        }
        check_quantization(self.quantization)?;
        self.previous = Some(std::mem::replace(
            &mut project.transport.global_quantization,
            self.quantization,
        ));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(quantization) = self.previous.take() {
            project.transport.global_quantization = quantization;
        }
    }
}

/// Sets the Punch-In and Punch-Out switches (manual 19.3.1, page 414).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetPunch {
    pub punch_in: bool,
    pub punch_out: bool,
    #[serde(skip)]
    previous: Option<(bool, bool)>,
}

impl SetPunch {
    pub fn new(punch_in: bool, punch_out: bool) -> Self {
        Self {
            punch_in,
            punch_out,
            previous: None,
        }
    }
}

impl Operation for SetPunch {
    const NAME: &'static str = "transport.set_punch";
    const LABEL: &'static str = "Change Punch";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let transport = &mut project.transport;
        self.previous = Some((transport.punch_in, transport.punch_out));
        transport.punch_in = self.punch_in;
        transport.punch_out = self.punch_out;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some((punch_in, punch_out)) = self.previous.take() {
            project.transport.punch_in = punch_in;
            project.transport.punch_out = punch_out;
        }
    }
}

/// Sets the Enable Follow Actions Globally button (manual 16.7, page 368).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetFollowActionsEnabled {
    pub enabled: bool,
    #[serde(skip)]
    previous: Option<bool>,
}

impl SetFollowActionsEnabled {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            previous: None,
        }
    }
}

impl Operation for SetFollowActionsEnabled {
    const NAME: &'static str = "transport.set_follow_actions";
    const LABEL: &'static str = "Enable Follow Actions Globally";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        self.previous = Some(std::mem::replace(
            &mut project.transport.follow_actions_enabled,
            self.enabled,
        ));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(enabled) = self.previous.take() {
            project.transport.follow_actions_enabled = enabled;
        }
    }
}

/// Sets the Metronome Settings: count-in, rhythm, sound, Enable Only While Recording, and level
/// (manual 19.4.1 and 19.6).
#[derive(Debug, Serialize, Deserialize)]
pub struct SetMetronomeSettings {
    pub metronome: Metronome,
    #[serde(skip)]
    previous: Option<Metronome>,
}

impl SetMetronomeSettings {
    pub fn new(metronome: Metronome) -> Self {
        Self {
            metronome,
            previous: None,
        }
    }
}

impl Operation for SetMetronomeSettings {
    const NAME: &'static str = "transport.set_metronome_settings";
    const LABEL: &'static str = "Change Metronome Settings";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let count_in = self.metronome.count_in_bars;
        if !Metronome::COUNT_IN_BARS.contains(&count_in) {
            return Err(CommandError::BadArguments {
                name: Self::NAME.to_string(),
                message: format!("a count-in of {count_in} bars is not offered (0, 1, 2, or 4)"),
            });
        }
        within(
            "metronome level",
            f64::from(self.metronome.volume_db),
            -70.0,
            6.0,
        )?;
        self.previous = Some(std::mem::replace(
            &mut project.transport.metronome,
            self.metronome,
        ));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(metronome) = self.previous.take() {
            project.transport.metronome = metronome;
        }
    }
}
