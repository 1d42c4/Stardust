//! Application layer: parse once, then dispatch with explicit paths and I/O.
mod controller;
mod model;
mod view;

pub use controller::{Environment, execute, run_cli, run_with};
pub use model::{Action, AppError, parse_args};
pub const VERSION: &str = concat!("stardust ", env!("CARGO_PKG_VERSION"));
