//! Generate a complete key with explicit inputs and no terminal access.
#[cfg(test)]
use super::derivation::derive_stream_key_with_params;
use super::derivation::{derive_production_stream_key, validate_config};
use super::model::*;
use super::storage::write_atomically;
use super::stream::write_stream_with_generator;
#[cfg(test)]
use argon2::Params;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

/// Generate `key.key` in `directory`, replacing an existing file only after a
/// complete temporary file has been flushed and synchronized.
pub fn generate_key_file(
    directory: &Path,
    password: Zeroizing<String>,
    size: u64,
    config: GeneratorConfig<'_>,
) -> Result<PathBuf, GenerateError> {
    generate_key_file_with_suite(
        directory,
        password,
        size,
        config,
        GeneratorSuite::Argon2idAes256Ctr,
    )
}

/// Generate key.key with an explicitly selected construction.
///
/// Each suite is deterministic for the password, size, configuration, suite,
/// and pinned algorithm parameters. Suites are intentionally not
/// interchangeable. Callers must use a distinct configuration domain for each
/// suite, as the supplied subcommands do.
pub fn generate_key_file_with_suite(
    directory: &Path,
    password: Zeroizing<String>,
    size: u64,
    config: GeneratorConfig<'_>,
    suite: GeneratorSuite,
) -> Result<PathBuf, GenerateError> {
    validate_size(size)?;
    validate_config(config)?;
    let stream_key = derive_production_stream_key(password.as_str(), size, config, suite)?;

    // The potentially long file write no longer needs either password copy.
    drop(password);
    write_key_file_from_stream_key(directory, size, &stream_key, suite.stream_generator())
}

#[cfg(test)]
pub(super) fn generate_key_file_with_params(
    directory: &Path,
    password: &str,
    size: u64,
    config: GeneratorConfig<'_>,
    params: Params,
) -> Result<PathBuf, GenerateError> {
    validate_size(size)?;
    validate_config(config)?;
    let stream_key = derive_stream_key_with_params(password, size, config, params)?;
    write_key_file_from_stream_key(directory, size, &stream_key, StreamGenerator::Aes256Ctr)
}

fn write_key_file_from_stream_key(
    directory: &Path,
    size: u64,
    stream_key: &[u8; DERIVED_KEY_BYTES],
    stream_generator: StreamGenerator,
) -> Result<PathBuf, GenerateError> {
    let output_path = directory.join(OUTPUT_FILENAME);

    write_atomically(directory, &output_path, |file| {
        write_stream_with_generator(
            file,
            stream_key,
            size,
            STREAM_BUFFER_BYTES,
            stream_generator,
        )
    })?;
    Ok(output_path)
}
