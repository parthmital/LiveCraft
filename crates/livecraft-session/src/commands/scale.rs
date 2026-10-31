//! Scale settings (M04 item 9): whether the scale is active, its root note, and its type. The three
//! are one edit, because the transport bar sets them together.

use crate::command::{CommandError, Operation};
use crate::registry::CommandRegistry;
use livecraft_model::ids::IdAllocator;
use livecraft_model::model::{Project, ScaleConfig};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<SetScale>();
}

/// Sets the scale of the Set. The root note is 0 for C up to 11 for B.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetScale {
    pub scale: ScaleConfig,
    #[serde(skip)]
    previous: Option<ScaleConfig>,
}

impl SetScale {
    pub fn new(scale: ScaleConfig) -> Self {
        Self {
            scale,
            previous: None,
        }
    }
}

impl Operation for SetScale {
    const NAME: &'static str = "scale.set";
    const LABEL: &'static str = "Set Scale";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if self.scale.root_note > 11 {
            return Err(CommandError::OutOfRange {
                what: "scale root note",
                min: 0.0,
                max: 11.0,
                value: f64::from(self.scale.root_note),
            });
        }
        self.previous = Some(std::mem::replace(&mut project.scale, self.scale));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(scale) = self.previous.take() {
            project.scale = scale;
        }
    }
}
