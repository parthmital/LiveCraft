//! The UI-independent app core (M04): named, serialisable edits with undo.
//!
//! Every change to a Live Set is an `Operation`. The GUI, the CLI, and later the control server
//! dispatch operations by name, and the `UndoHistory` records them so each one can be undone.

pub mod command;
pub mod commands;
mod engine_sync;
pub mod history;
pub mod registry;
pub mod session;

pub use command::{Command, CommandError, Operation};
pub use history::{HistoryEntry, MERGE_GAP, OPENING_LABEL, Transaction, UndoHistory};
pub use registry::CommandRegistry;
pub use session::{EngineStatus, Session, TrackMeter, demo_set, load_set};
