pub mod als;
pub mod command;
pub mod model;
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
