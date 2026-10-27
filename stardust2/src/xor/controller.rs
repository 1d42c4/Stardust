//! Coordinate validation, streaming and atomic replacement.
use super::atomic_file::{overwrite_atomically, write_atomically};
use super::file_safety::{check_unchanged, file_identity, open_input};
use super::model::{Args, validate_name};
use super::stream::xor_streams;
use std::fs::{self, File};
use std::io;
use std::path::Path;
const KEY_NAME: &str = "key.key";

pub fn run(args: &Args) -> Result<u64, String> {
    let executable = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| format!("cannot locate the executable: {e}"))?;
    run_beside_executable(args, &executable)
}

pub fn run_beside_executable(args: &Args, executable: &Path) -> Result<u64, String> {
    validate_name(&args.a)?;
    validate_name(args.output_name())?;
    let directory = executable
        .parent()
        .ok_or("executable has no parent directory")?;
    let a_path = directory.join(&args.a);
    let key_path = directory.join(KEY_NAME);
    let output = directory.join(args.output_name());
    let mut a =
        open_input(&a_path).map_err(|e| format!("cannot open {}: {e}", a_path.display()))?;
    let mut key =
        open_input(&key_path).map_err(|e| format!("cannot open {}: {e}", key_path.display()))?;
    let a_before = a.metadata().map_err(|e| e.to_string())?;
    let key_before = key.metadata().map_err(|e| e.to_string())?;
    if key_before.len() < a_before.len() {
        return Err(format!(
            "key.key is too short ({} bytes); FILE_A requires at least {} bytes",
            key_before.len(),
            a_before.len()
        ));
    }
    // Keep the handles alive until publication so file IDs cannot be recycled.
    let program = File::open(executable).map_err(|e| format!("cannot open executable: {e}"))?;
    let mut protected = [file_identity(&key), file_identity(&program)]
        .into_iter()
        .collect::<io::Result<Vec<_>>>()
        .map_err(|e| format!("cannot identify protected files: {e}"))?;
    let mut transform = |a: &mut File, out: &mut File| {
        let bytes = xor_streams(a, &mut key, out, a_before.len())?;
        check_unchanged(&a_before, &a.metadata()?, "FILE_A")?;
        check_unchanged(&key_before, &key.metadata()?, KEY_NAME)?;
        Ok(bytes)
    };
    let result = if args.out.is_none() {
        // The transaction owns and closes this reader before Windows renames it.
        overwrite_atomically(&output, a, &protected, transform)
    } else {
        protected.push(file_identity(&a).map_err(|e| format!("cannot identify FILE_A: {e}"))?);
        write_atomically(&output, &protected, |out| transform(&mut a, out))
    };
    result.map_err(|e| format!("{}: {e}", output.display()))
}
