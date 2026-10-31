//! Every registered command, grouped by the part of the Set it edits (M04). A new command is an
//! `Operation` in one of these modules, registered in that module's `register`.

pub mod automation;
pub mod clip;
pub mod device;
pub mod note;
pub mod track;
pub mod transport;

use crate::command::CommandError;
use crate::registry::CommandRegistry;

/// A registry with every command of the workspace.
pub fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    register(&mut registry);
    registry
}

/// Registers every command. Each area registers its own operations.
pub fn register(registry: &mut CommandRegistry) {
    automation::register(registry);
    clip::register(registry);
    device::register(registry);
    note::register(registry);
    track::register(registry);
    transport::register(registry);
}

/// A value a command stores must be a finite number, so no NaN or infinity reaches the Set or the
/// audio engine.
fn finite(what: &'static str, value: f64) -> Result<(), CommandError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CommandError::NotFinite { what, value })
    }
}
