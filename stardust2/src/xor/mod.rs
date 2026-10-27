//! Astra-compatible XOR. All filenames resolve beside the executable.
mod atomic_file;
mod controller;
mod file_safety;
mod model;
mod stream;

pub use controller::{run, run_beside_executable};
pub use model::{Action, Args, USAGE, VERSION, parse_args};

// Keep inherited regression tests in their original groups. These imports are
// test-only; production code imports its dependencies explicitly.
#[cfg(test)]
use atomic_file::write_atomically;
#[cfg(test)]
use model::validate_name;
#[cfg(test)]
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
};
#[cfg(test)]
const KEY_NAME: &str = "key.key";
#[cfg(test)]
use stream::{BUF_SIZE, xor_streams};
#[cfg(test)]
#[path = "tests/arguments.rs"]
mod arguments;
#[cfg(test)]
#[path = "tests/file_checks.rs"]
mod file_checks;
#[cfg(test)]
#[path = "tests/streams.rs"]
mod streams;
#[cfg(test)]
#[path = "../../tests/support/mod.rs"]
mod test_support;
