//! Versioned suite types, fixed parameters, size validation, and errors.
use aes::Aes256;
use std::{error::Error, fmt, io};

/// Twenty decimal gigabytes, expressed only in bytes at the CLI.
pub const MAX_KEY_BYTES: u64 = 20_000_000_000;
pub const OUTPUT_FILENAME: &str = "key.key";

pub(super) const STREAM_BUFFER_BYTES: usize = 1024 * 1024;
pub(super) const AES_BLOCK_BYTES: u64 = 16;
pub(super) const CHACHA_NONCE_BYTES: usize = 12;
pub(super) const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
pub(super) const ARGON2_ITERATIONS: u32 = 3;
pub(super) const ARGON2_LANES: u32 = 1;
pub(super) const SCRYPT_LOG_N: u8 = 16;
pub(super) const SCRYPT_R: u32 = 8;
pub(super) const SCRYPT_P: u32 = 1;
pub(super) const PBKDF2_ITERATIONS: u32 = 600_000;
pub(super) const DERIVED_KEY_BYTES: usize = 32;
pub(super) const SALT_FORMAT_TAG: &[u8] = b"versakey/effective-salt/v1";
pub(super) const PEPPERED_SALT_FORMAT_TAG: &[u8] = b"versakey/effective-salt-with-pepper/v1";
pub(super) const BLAKE3_XOF_FORMAT_TAG: &[u8] = b"versakey/blake3-keyed-xof-stream/v1";

pub(super) type Aes256Ctr = ctr::Ctr128BE<Aes256>;

/// Build-specific values that deliberately separate one compiled application
/// from another. Changing any field changes the generated key material.
///
/// Give each generator suite a distinct domain. The suite enum is not itself
/// folded into the KDF so the original suite can retain byte-for-byte
/// compatibility; the shipped subcommands therefore use separate versioned
/// domains.
#[derive(Clone, Copy)]
pub struct GeneratorConfig<'a> {
    pub application_salt: &'a [u8],
    pub application_pepper: &'a [u8],
    pub domain: &'a [u8],
}

/// A complete, versioned password-KDF and stream-expansion construction.
///
/// The original Argon2idAes256Ctr suite remains the default used by
/// generate_key_file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorSuite {
    Argon2idAes256Ctr,
    ScryptAes256Ctr,
    Pbkdf2Sha256Aes256Ctr,
    Argon2idChaCha20,
    Argon2idBlake3Xof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StreamGenerator {
    Aes256Ctr,
    ChaCha20,
    Blake3Xof,
}

impl GeneratorSuite {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Argon2idAes256Ctr => "Argon2id + AES-256-CTR",
            Self::ScryptAes256Ctr => "scrypt + AES-256-CTR",
            Self::Pbkdf2Sha256Aes256Ctr => "PBKDF2-HMAC-SHA-256 + AES-256-CTR",
            Self::Argon2idChaCha20 => "Argon2id + ChaCha20",
            Self::Argon2idBlake3Xof => "Argon2id + keyed BLAKE3 XOF",
        }
    }

    pub(super) fn stream_generator(self) -> StreamGenerator {
        match self {
            Self::Argon2idAes256Ctr | Self::ScryptAes256Ctr | Self::Pbkdf2Sha256Aes256Ctr => {
                StreamGenerator::Aes256Ctr
            }
            Self::Argon2idChaCha20 => StreamGenerator::ChaCha20,
            Self::Argon2idBlake3Xof => StreamGenerator::Blake3Xof,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeError {
    Empty,
    NotDecimal,
    OutOfRange,
}

impl Error for SizeError {}

// Size errors are safe to display to CLI users.

impl fmt::Display for SizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "key size cannot be empty"),
            Self::NotDecimal => write!(
                formatter,
                "key size must contain decimal digits only (bytes, with no unit suffix)"
            ),
            Self::OutOfRange => write!(
                formatter,
                "key size must be between 1 and {MAX_KEY_BYTES} bytes"
            ),
        }
    }
}

#[derive(Debug)]
pub enum GenerateError {
    InvalidSize(SizeError),
    InvalidConfiguration(&'static str),
    EmptyPassword,
    KeyDerivation(argon2::Error),
    ScryptParameters(scrypt::errors::InvalidParams),
    ScryptKeyDerivation(scrypt::errors::InvalidOutputLen),
    CounterExhausted,
    Io(io::Error),
    CommittedButDirectorySyncFailed(io::Error),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize(error) => error.fmt(formatter),
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid generator configuration: {message}")
            }
            Self::EmptyPassword => write!(formatter, "password cannot be empty"),
            Self::KeyDerivation(_) | Self::ScryptParameters(_) | Self::ScryptKeyDerivation(_) => {
                write!(formatter, "password key derivation failed")
            }
            Self::CounterExhausted => {
                write!(formatter, "stream cipher counter capacity was exhausted")
            }
            Self::Io(error) => write!(formatter, "file operation failed: {error}"),
            Self::CommittedButDirectorySyncFailed(error) => write!(
                formatter,
                "key.key was replaced, but synchronizing its directory failed: {error}"
            ),
        }
    }
}

impl Error for GenerateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSize(error) => Some(error),
            Self::KeyDerivation(_) | Self::ScryptParameters(_) | Self::ScryptKeyDerivation(_) => {
                None
            }
            Self::Io(error) => Some(error),
            Self::CommittedButDirectorySyncFailed(error) => Some(error),
            Self::InvalidConfiguration(_) | Self::EmptyPassword | Self::CounterExhausted => None,
        }
    }
}

impl From<SizeError> for GenerateError {
    fn from(error: SizeError) -> Self {
        Self::InvalidSize(error)
    }
}

impl From<io::Error> for GenerateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Parse a decimal byte count. Surrounding console whitespace is ignored, but
/// signs, separators, fractions, exponents, and unit suffixes are rejected.
pub fn parse_size(input: &str) -> Result<u64, SizeError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SizeError::Empty);
    }
    if !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SizeError::NotDecimal);
    }

    let size = trimmed.parse::<u64>().map_err(|_| SizeError::OutOfRange)?;
    validate_size(size)?;
    Ok(size)
}

pub fn validate_size(size: u64) -> Result<(), SizeError> {
    if (1..=MAX_KEY_BYTES).contains(&size) {
        Ok(())
    } else {
        Err(SizeError::OutOfRange)
    }
}
