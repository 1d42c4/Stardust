//! The five original configurations. Never rename any derivation byte string.
use super::{GeneratorConfig, GeneratorSuite};

// These values and each command's domain preserve the five original binaries'
// output. Renaming a command must never change its cryptographic configuration.
const APPLICATION_SALT: &[u8] = b"versakey application salt 2026-08-11 v1";
const APPLICATION_PEPPER: &[u8] = b"VersaKey-build-pepper-change-me-v1";

pub struct GeneratorCommand {
    pub name: &'static str,
    pub suite: GeneratorSuite,
    pub domain: &'static [u8],
}

impl GeneratorCommand {
    pub fn config(&self) -> GeneratorConfig<'static> {
        GeneratorConfig {
            application_salt: APPLICATION_SALT,
            application_pepper: APPLICATION_PEPPER,
            domain: self.domain,
        }
    }
}

pub const COMMANDS: [GeneratorCommand; 5] = [
    GeneratorCommand {
        name: "argon2-aes",
        suite: GeneratorSuite::Argon2idAes256Ctr,
        domain: b"versakey/key-file/aes256-ctr/v1",
    },
    GeneratorCommand {
        name: "scrypt-aes",
        suite: GeneratorSuite::ScryptAes256Ctr,
        domain: b"versakey/key-file/scrypt-aes256-ctr/v1",
    },
    GeneratorCommand {
        name: "pbkdf2-aes",
        suite: GeneratorSuite::Pbkdf2Sha256Aes256Ctr,
        domain: b"versakey/key-file/pbkdf2-sha256-aes256-ctr/v1",
    },
    GeneratorCommand {
        name: "argon2-chacha20",
        suite: GeneratorSuite::Argon2idChaCha20,
        domain: b"versakey/key-file/argon2id-chacha20/v1",
    },
    GeneratorCommand {
        name: "argon2-blake3",
        suite: GeneratorSuite::Argon2idBlake3Xof,
        domain: b"versakey/key-file/argon2id-blake3-xof/v1",
    },
];
