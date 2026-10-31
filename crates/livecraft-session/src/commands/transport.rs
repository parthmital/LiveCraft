//! Transport edits (M04 items 3 and 7): tempo, time signature, loop, and metronome.

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::ids::IdAllocator;
use livecraft_model::model::Project;
use livecraft_time::{MAX_BPM, MIN_BPM, TempoMap, TimeSignatureMap};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<SetTempo>();
    registry.register::<SetTimeSignature>();
    registry.register::<SetLoop>();
    registry.register::<SetMetronome>();
}

/// Sets one tempo for the whole Set. A drag on the tempo control merges into one undo step.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTempo {
    pub bpm: f32,
    #[serde(skip)]
    previous: Option<TempoMap>,
}

impl SetTempo {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm,
            previous: None,
        }
    }
}

impl Operation for SetTempo {
    const NAME: &'static str = "transport.set_tempo";
    const LABEL: &'static str = "Set Tempo";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        // The range check also rejects NaN, because no range contains it.
        let bpm = f64::from(self.bpm);
        if !(MIN_BPM..=MAX_BPM).contains(&bpm) {
            return Err(CommandError::OutOfRange {
                what: "tempo",
                min: MIN_BPM,
                max: MAX_BPM,
                value: bpm,
            });
        }
        self.previous = Some(project.transport.tempo.clone());
        project.transport.set_bpm(self.bpm);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(tempo) = self.previous.take() {
            project.transport.tempo = tempo;
        }
    }

    fn absorb(&mut self, next: &Self) -> bool {
        self.bpm = next.bpm;
        true
    }
}

/// Sets the time signature for the whole Set (manual 6.5). The numerator and the denominator must
/// make a signature that `TimeSignatureMap` accepts.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTimeSignature {
    pub numerator: u8,
    pub denominator: u8,
    #[serde(skip)]
    previous: Option<TimeSignatureMap>,
}

impl SetTimeSignature {
    pub fn new(numerator: u8, denominator: u8) -> Self {
        Self {
            numerator,
            denominator,
            previous: None,
        }
    }
}

impl Operation for SetTimeSignature {
    const NAME: &'static str = "transport.set_time_signature";
    const LABEL: &'static str = "Set Time Signature";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let previous = project.transport.time_signature.clone();
        project
            .transport
            .set_time_signature(self.numerator, self.denominator)?;
        self.previous = Some(previous);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(signature) = self.previous.take() {
            project.transport.time_signature = signature;
        }
    }
}

/// Sets the loop: whether it is on, where it starts, and how long it is, in beats.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetLoop {
    pub loop_enabled: bool,
    pub loop_start_beats: f64,
    pub loop_length_beats: f64,
    #[serde(skip)]
    previous: Option<(bool, f64, f64)>,
}

impl SetLoop {
    pub fn new(loop_enabled: bool, loop_start_beats: f64, loop_length_beats: f64) -> Self {
        Self {
            loop_enabled,
            loop_start_beats,
            loop_length_beats,
            previous: None,
        }
    }
}

impl Operation for SetLoop {
    const NAME: &'static str = "transport.set_loop";
    const LABEL: &'static str = "Set Loop";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("loop start", self.loop_start_beats)?;
        finite("loop length", self.loop_length_beats)?;
        if self.loop_start_beats < 0.0 {
            return Err(CommandError::InvalidRange {
                what: "loop",
                start: self.loop_start_beats,
                end: self.loop_start_beats + self.loop_length_beats,
            });
        }
        if self.loop_length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "loop length",
                value: self.loop_length_beats,
            });
        }
        let transport = &mut project.transport;
        self.previous = Some((
            transport.loop_enabled,
            transport.loop_start_beats,
            transport.loop_length_beats,
        ));
        transport.loop_enabled = self.loop_enabled;
        transport.loop_start_beats = self.loop_start_beats;
        transport.loop_length_beats = self.loop_length_beats;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some((enabled, start, length)) = self.previous.take() {
            let transport = &mut project.transport;
            transport.loop_enabled = enabled;
            transport.loop_start_beats = start;
            transport.loop_length_beats = length;
        }
    }
}

/// Turns the metronome on or off.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetMetronome {
    pub metronome_enabled: bool,
    #[serde(skip)]
    previous: Option<bool>,
}

impl SetMetronome {
    pub fn new(metronome_enabled: bool) -> Self {
        Self {
            metronome_enabled,
            previous: None,
        }
    }
}

impl Operation for SetMetronome {
    const NAME: &'static str = "transport.set_metronome";
    const LABEL: &'static str = "Set Metronome";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        self.previous = Some(std::mem::replace(
            &mut project.transport.metronome_enabled,
            self.metronome_enabled,
        ));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(enabled) = self.previous.take() {
            project.transport.metronome_enabled = enabled;
        }
    }
}
