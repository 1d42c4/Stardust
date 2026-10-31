//! Compatibility-critical KDFs and byte framing, preserved from VersaKey.
use super::model::*;
use argon2::{Algorithm, Argon2, Params, Version};
use pbkdf2::{pbkdf2_hmac, sha2::Sha256};
use scrypt::{Params as ScryptParams, scrypt};
use std::mem::size_of;
use zeroize::Zeroizing;

pub(super) fn validate_config(config: GeneratorConfig<'_>) -> Result<(), GenerateError> {
    if config.application_salt.is_empty() {
        return Err(GenerateError::InvalidConfiguration(
            "application salt cannot be empty",
        ));
    }
    if config.application_pepper.is_empty() {
        return Err(GenerateError::InvalidConfiguration(
            "application pepper cannot be empty",
        ));
    }
    if config.domain.is_empty() {
        return Err(GenerateError::InvalidConfiguration(
            "domain cannot be empty",
        ));
    }
    Ok(())
}

pub(super) fn production_params() -> Result<Params, GenerateError> {
    Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_LANES,
        Some(DERIVED_KEY_BYTES),
    )
    .map_err(GenerateError::KeyDerivation)
}

pub(super) fn production_scrypt_params() -> Result<ScryptParams, GenerateError> {
    ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P).map_err(GenerateError::ScryptParameters)
}

pub(super) fn derive_production_stream_key(
    password: &str,
    size: u64,
    config: GeneratorConfig<'_>,
    suite: GeneratorSuite,
) -> Result<Zeroizing<[u8; DERIVED_KEY_BYTES]>, GenerateError> {
    match suite {
        GeneratorSuite::Argon2idAes256Ctr
        | GeneratorSuite::Argon2idChaCha20
        | GeneratorSuite::Argon2idBlake3Xof => {
            derive_stream_key_with_params(password, size, config, production_params()?)
        }
        GeneratorSuite::ScryptAes256Ctr => derive_scrypt_stream_key_with_params(
            password,
            size,
            config,
            production_scrypt_params()?,
        ),
        GeneratorSuite::Pbkdf2Sha256Aes256Ctr => {
            derive_pbkdf2_stream_key_with_rounds(password, size, config, PBKDF2_ITERATIONS)
        }
    }
}

pub(super) fn derive_stream_key_with_params(
    password: &str,
    size: u64,
    config: GeneratorConfig<'_>,
    params: Params,
) -> Result<Zeroizing<[u8; DERIVED_KEY_BYTES]>, GenerateError> {
    validate_size(size)?;
    validate_config(config)?;
    if password.is_empty() {
        return Err(GenerateError::EmptyPassword);
    }

    let effective_salt = build_effective_salt(config, size)?;
    derive_argon2_key_with_effective_salt(password, config, params, effective_salt)
}

pub(super) fn derive_argon2_key_with_effective_salt(
    password: &str,
    config: GeneratorConfig<'_>,
    params: Params,
    effective_salt: Vec<u8>,
) -> Result<Zeroizing<[u8; DERIVED_KEY_BYTES]>, GenerateError> {
    let effective_salt = Zeroizing::new(effective_salt);
    let memory_block_count = params.block_count();
    let argon2 = Argon2::new_with_secret(
        config.application_pepper,
        Algorithm::Argon2id,
        Version::V0x13,
        params,
    )
    .map_err(GenerateError::KeyDerivation)?;

    let mut key = Zeroizing::new([0_u8; DERIVED_KEY_BYTES]);
    let mut memory = Zeroizing::new(vec![argon2::Block::default(); memory_block_count]);
    argon2
        .hash_password_into_with_memory(
            password.as_bytes(),
            &effective_salt,
            key.as_mut(),
            memory.as_mut_slice(),
        )
        .map_err(GenerateError::KeyDerivation)?;
    Ok(key)
}

pub(super) fn derive_scrypt_stream_key_with_params(
    password: &str,
    size: u64,
    config: GeneratorConfig<'_>,
    params: ScryptParams,
) -> Result<Zeroizing<[u8; DERIVED_KEY_BYTES]>, GenerateError> {
    validate_size(size)?;
    validate_config(config)?;
    if password.is_empty() {
        return Err(GenerateError::EmptyPassword);
    }

    let effective_salt = Zeroizing::new(build_peppered_effective_salt(config, size)?);
    let mut key = Zeroizing::new([0_u8; DERIVED_KEY_BYTES]);
    scrypt(password.as_bytes(), &effective_salt, &params, key.as_mut())
        .map_err(GenerateError::ScryptKeyDerivation)?;
    Ok(key)
}

pub(super) fn derive_pbkdf2_stream_key_with_rounds(
    password: &str,
    size: u64,
    config: GeneratorConfig<'_>,
    rounds: u32,
) -> Result<Zeroizing<[u8; DERIVED_KEY_BYTES]>, GenerateError> {
    validate_size(size)?;
    validate_config(config)?;
    if password.is_empty() {
        return Err(GenerateError::EmptyPassword);
    }
    if rounds == 0 {
        return Err(GenerateError::InvalidConfiguration(
            "PBKDF2 iteration count cannot be zero",
        ));
    }

    let effective_salt = Zeroizing::new(build_peppered_effective_salt(config, size)?);
    let mut key = Zeroizing::new([0_u8; DERIVED_KEY_BYTES]);
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &effective_salt, rounds, key.as_mut());
    Ok(key)
}

pub(super) fn build_effective_salt(
    config: GeneratorConfig<'_>,
    size: u64,
) -> Result<Vec<u8>, GenerateError> {
    let fields = [SALT_FORMAT_TAG, config.domain, config.application_salt];
    let mut salt = allocate_framed_salt(&fields, size.to_be_bytes().len())?;
    append_length_prefixed(&mut salt, SALT_FORMAT_TAG)?;
    append_length_prefixed(&mut salt, config.domain)?;
    append_length_prefixed(&mut salt, config.application_salt)?;
    salt.extend_from_slice(&size.to_be_bytes());
    Ok(salt)
}

pub(super) fn build_peppered_effective_salt(
    config: GeneratorConfig<'_>,
    size: u64,
) -> Result<Vec<u8>, GenerateError> {
    let fields = [
        PEPPERED_SALT_FORMAT_TAG,
        config.domain,
        config.application_salt,
        config.application_pepper,
    ];
    let mut salt = allocate_framed_salt(&fields, size.to_be_bytes().len())?;
    for field in fields {
        append_length_prefixed(&mut salt, field)?;
    }
    salt.extend_from_slice(&size.to_be_bytes());
    Ok(salt)
}

pub(super) fn allocate_framed_salt(
    fields: &[&[u8]],
    trailing_bytes: usize,
) -> Result<Vec<u8>, GenerateError> {
    let capacity = fields
        .iter()
        .try_fold(trailing_bytes, |total, field| {
            total
                .checked_add(size_of::<u64>())
                .and_then(|total| total.checked_add(field.len()))
        })
        .ok_or(GenerateError::InvalidConfiguration(
            "configuration values are too large",
        ))?;
    let mut salt = Vec::new();
    salt.try_reserve_exact(capacity)
        .map_err(|_| GenerateError::InvalidConfiguration("configuration values are too large"))?;
    Ok(salt)
}

pub(super) fn append_length_prefixed(
    target: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), GenerateError> {
    let length = u64::try_from(value.len())
        .map_err(|_| GenerateError::InvalidConfiguration("configuration value is too large"))?;
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(value);
    Ok(())
}
