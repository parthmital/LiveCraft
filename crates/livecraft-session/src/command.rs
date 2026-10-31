//! Named, serialisable edits with undo (M04 item 1).
//!
//! An `Operation` is one kind of edit. Its fields are its arguments, which serialise to JSON for
//! the CLI and the control server. It keeps what it needs to undo itself in `#[serde(skip)]`
//! fields, which `perform` fills in. `Command` is the object-safe form the history stores; every
//! `Operation` is a `Command` through the blanket impl below.

use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, DeviceId, IdAllocator, LaneId, TrackId};
use livecraft_model::model::Project;
use livecraft_time::TimeError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::any::Any;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error(transparent)]
    Time(#[from] TimeError),
    #[error("no command named `{0}`")]
    UnknownCommand(String),
    #[error("the arguments of `{name}` do not decode: {message}")]
    BadArguments { name: String, message: String },
    #[error("track {track} has no automation lane {lane}")]
    NoSuchLane { track: TrackId, lane: LaneId },
    #[error("lane {lane} of track {track} has no point at index {index}")]
    NoSuchPoint {
        track: TrackId,
        lane: LaneId,
        index: usize,
    },
    #[error("{what} must be between {min} and {max}, not {value}")]
    OutOfRange {
        what: &'static str,
        min: f64,
        max: f64,
        value: f64,
    },
    #[error("{what} must be a finite number, not {value}")]
    NotFinite { what: &'static str, value: f64 },
    #[error("{value:?} is not a color in the form #RRGGBB")]
    BadColor { value: String },
    #[error("no clip with id {0}")]
    NoSuchClip(ClipId),
    #[error("clip {0} is not a MIDI clip, so it has no notes")]
    NotMidiClip(ClipId),
    #[error("clip {clip} has no note at index {index}")]
    NoSuchNote { clip: ClipId, index: usize },
    #[error("device {device} has no parameter named `{name}`")]
    NoSuchParam { device: DeviceId, name: String },
    #[error("track {track} already has a clip in slot {slot}")]
    SlotOccupied { track: TrackId, slot: usize },
    #[error("track {track} has no clip in slot {slot}")]
    SlotEmpty { track: TrackId, slot: usize },
    #[error("{what} runs from {start} to {end}, which is not a valid range")]
    InvalidRange {
        what: &'static str,
        start: f64,
        end: f64,
    },
    #[error("{what} must be greater than zero, not {value}")]
    NotPositive { what: &'static str, value: f64 },
}

/// One kind of edit to a Live Set.
///
/// `perform` may allocate ids from `ids` the first time it runs and must reuse them on later runs,
/// so a redo recreates the same entity that later commands refer to. `revert` restores the Set to
/// what it was before the last `perform`.
pub trait Operation: Serialize + DeserializeOwned + Send + 'static {
    /// The stable dotted name used by the registry, the CLI, and the control server.
    const NAME: &'static str;
    /// The entry shown in the Undo History.
    const LABEL: &'static str;

    fn perform(&mut self, project: &mut Project, ids: &mut IdAllocator)
    -> Result<(), CommandError>;

    fn revert(&mut self, project: &mut Project);

    /// Continues a gesture: when `next` is the same kind of edit on the same target, takes its
    /// arguments and keeps this operation's undo data, and returns true. A fader drag uses this.
    fn absorb(&mut self, next: &Self) -> bool {
        let _ = next;
        false
    }
}

/// The object-safe form of an `Operation`, which the history stores.
pub trait Command: Send {
    fn name(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn apply(&mut self, project: &mut Project, ids: &mut IdAllocator) -> Result<(), CommandError>;
    fn undo(&mut self, project: &mut Project);
    /// Folds `next` into this command when it continues the same gesture. Returns false when the
    /// two are not mergeable, and then `next` is recorded as its own step.
    fn merge(&mut self, next: &dyn Command) -> bool;
    /// The arguments as JSON, the form the registry decodes.
    fn arguments(&self) -> serde_json::Value;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Operation> Command for T {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn label(&self) -> &'static str {
        T::LABEL
    }

    fn apply(&mut self, project: &mut Project, ids: &mut IdAllocator) -> Result<(), CommandError> {
        self.perform(project, ids)
    }

    fn undo(&mut self, project: &mut Project) {
        self.revert(project);
    }

    fn merge(&mut self, next: &dyn Command) -> bool {
        match next.as_any().downcast_ref::<T>() {
            Some(next) => self.absorb(next),
            None => false,
        }
    }

    fn arguments(&self) -> serde_json::Value {
        // Serialising a struct of ids, numbers, strings, and enums cannot fail, so the fallback
        // is unreachable in practice.
        serde_json::to_value(self).unwrap_or_default()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
