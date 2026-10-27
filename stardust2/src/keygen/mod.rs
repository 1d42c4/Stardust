//! VersaKey-compatible key generation; no process-global working-directory changes.
pub mod cli;
pub mod commands;
mod controller;
mod derivation;
mod model;
mod storage;
mod stream;

pub use controller::{generate_key_file, generate_key_file_with_suite};
pub use model::{
    GenerateError, GeneratorConfig, GeneratorSuite, MAX_KEY_BYTES, OUTPUT_FILENAME, SizeError,
    parse_size, validate_size,
};

#[cfg(test)]
use controller::generate_key_file_with_params;
#[cfg(test)]
use derivation::*;
#[cfg(test)]
use model::*;
#[cfg(test)]
use storage::write_atomically;
#[cfg(test)]
use stream::*;
#[cfg(test)]
mod tests;
