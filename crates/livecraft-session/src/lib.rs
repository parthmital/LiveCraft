//! The UI-independent app core (M04): named, serialisable edits with undo.
//!
//! Every change to a Live Set is an `Operation`. The GUI, the CLI, and later the control server
//! dispatch operations by name, and the `UndoHistory` records them so each one can be undone.

pub mod command;
pub mod commands;
pub mod history;
pub mod registry;

pub use command::{Command, CommandError, Operation};
pub use history::{MERGE_GAP, UndoHistory};
pub use registry::CommandRegistry;
