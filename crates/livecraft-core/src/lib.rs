pub mod als;
pub use livecraft_model::command;
pub use livecraft_model::model;
pub mod theme;

pub use als::*;
pub use command::*;
pub use model::*;
pub use theme::*;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests;
