//! Transport edits (M04 items 3 and 7). Item 3 ports the tempo action from M01. Item 7 adds the
//! time signature, loop, and metronome.

use crate::command::{CommandError, Operation};
use crate::registry::CommandRegistry;
use livecraft_model::ids::IdAllocator;
use livecraft_model::model::Project;
use livecraft_time::{MAX_BPM, MIN_BPM, TempoMap};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<SetTempo>();
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
