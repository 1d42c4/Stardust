//! Regression coverage through the same parser, dispatch, prompts and file writer
//! used by the executable. Only terminal password input is replaced in tests.

use super::*;
use aes::cipher::{Array, BlockCipherEncrypt, KeyInit, KeyIvInit, StreamCipher};
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::sync::OnceLock;

const FIXTURE_PASSWORD: &str = "application compatibility fixture";
const DIFFERENT_PASSWORD: &str = "application compatibility fixture changed";
const FIXTURE_SIZE: u64 = 64;
const LARGE_SIZE: usize = 2 * crate::STREAM_BUFFER_BYTES + 137;
const SENTINEL: &[u8] = b"keep the existing key on any pre-commit failure";

fn selected(name: &str) -> &'static GeneratorCommand {
    match parse_args([name]).expect("parse generator subcommand") {
        Action::Generate(command) => command,
        _ => panic!("expected a generator command"),
    }
}

struct Attempt {
    result: Result<(), CliError>,
    output: String,
    prompts: Vec<String>,
}

fn attempt(name: &str, directory: &Path, size: &str, passwords: &[&str]) -> Attempt {
    let mut input = Cursor::new(size.as_bytes());
    let mut output = Vec::new();
    let mut passwords = passwords.iter();
    let mut prompts = Vec::new();
    let result = run_command_with(
        selected(name),
        directory,
        &mut input,
        &mut output,
        |prompt| {
            prompts.push(prompt.to_owned());
            passwords
                .next()
                .map(|password| (*password).to_owned())
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "test input ended"))
        },
    );
    Attempt {
        result,
        output: String::from_utf8(output).expect("UTF-8 status output"),
        prompts,
    }
}

fn generate(name: &str, directory: &Path, password: &str, size: u64) -> Vec<u8> {
    let attempt = attempt(name, directory, &format!("{size}\n"), &[password, password]);
    attempt.result.expect("generate regression key through CLI");
    assert_eq!(attempt.prompts, ["Password: ", "Confirm password: "]);
    assert!(
        attempt
            .output
            .contains(&format!("Created key.key ({size} bytes)."))
    );
    assert!(!attempt.output.contains(password));
    assert_eq!(
        fs::read_dir(directory).unwrap().count(),
        1,
        "temporary file leaked"
    );
    let bytes = fs::read(directory.join(OUTPUT_FILENAME)).expect("read regression key");
    assert_eq!(
        bytes.len() as u64,
        size,
        "incorrect length or an unexpected header"
    );
    bytes
}

struct RegressionFixture {
    baseline: Vec<u8>,
    repeated: Vec<u8>,
    different_password: Vec<u8>,
    different_size: Vec<u8>,
    large: Vec<u8>,
    repeated_large: Vec<u8>,
}

impl RegressionFixture {
    fn build(name: &str) -> Self {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        // The first generation must replace and truncate a longer existing file.
        fs::write(first.path().join(OUTPUT_FILENAME), vec![0xa5; 4096]).unwrap();
        let baseline = generate(name, first.path(), FIXTURE_PASSWORD, FIXTURE_SIZE);
        let repeated = generate(name, second.path(), FIXTURE_PASSWORD, FIXTURE_SIZE);
        let different_password = generate(name, first.path(), DIFFERENT_PASSWORD, FIXTURE_SIZE);
        let different_size = generate(name, first.path(), FIXTURE_PASSWORD, FIXTURE_SIZE + 1);
        // Cross two production buffer boundaries and end on a partial cipher block.
        let large = generate(name, first.path(), FIXTURE_PASSWORD, LARGE_SIZE as u64);
        let repeated_large = generate(name, second.path(), FIXTURE_PASSWORD, LARGE_SIZE as u64);
        Self {
            baseline,
            repeated,
            different_password,
            different_size,
            large,
            repeated_large,
        }
    }
}

// An independent expansion path, without the application's streaming helpers:
// direct AES counter encryption, one ChaCha20 call, or one BLAKE3 XOF read.
fn reference_expansion(name: &str) -> Vec<u8> {
    let command = selected(name);
    let key = crate::derive_production_stream_key(
        FIXTURE_PASSWORD,
        LARGE_SIZE as u64,
        command.config(),
        command.suite,
    )
    .unwrap();
    let mut expected = vec![0; LARGE_SIZE];
    match command.suite {
        GeneratorSuite::Argon2idAes256Ctr
        | GeneratorSuite::ScryptAes256Ctr
        | GeneratorSuite::Pbkdf2Sha256Aes256Ctr => {
            let cipher = aes::Aes256::new(&Array::from(*key));
            for (counter, chunk) in expected.chunks_mut(16).enumerate() {
                let mut block = Array::from((counter as u128).to_be_bytes());
                cipher.encrypt_block(&mut block);
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
        }
        GeneratorSuite::Argon2idChaCha20 => {
            let mut cipher = chacha20::ChaCha20::new((&*key).into(), (&[0_u8; 12]).into());
            cipher.try_apply_keystream(&mut expected).unwrap();
        }
        GeneratorSuite::Argon2idBlake3Xof => {
            let mut hasher = blake3::Hasher::new_keyed(&key);
            hasher.update(b"versakey/blake3-keyed-xof-stream/v1");
            hasher.finalize_xof().fill(&mut expected);
        }
    }
    expected
}

macro_rules! command_regressions {
    ($module:ident, $name:literal, $suite:expr, $domain:literal, $known_answer:expr) => {
        mod $module {
            use super::*;

            fn fixture() -> &'static RegressionFixture {
                static FIXTURE: OnceLock<RegressionFixture> = OnceLock::new();
                FIXTURE.get_or_init(|| RegressionFixture::build($name))
            }

            #[test]
            fn original_suite_and_configuration_are_frozen() {
                let command = selected($name);
                let config = command.config();
                assert_eq!(command.name, $name);
                assert_eq!(command.suite, $suite);
                assert_eq!(config.domain, $domain);
                assert_eq!(
                    config.application_salt,
                    b"versakey application salt 2026-08-11 v1"
                );
                assert_eq!(
                    config.application_pepper,
                    b"VersaKey-build-pepper-change-me-v1"
                );
            }

            #[test]
            fn matches_original_binary_known_answer() {
                let expected: [u8; 64] = $known_answer;
                assert_eq!(fixture().baseline, expected);
            }

            #[test]
            fn small_and_large_keys_are_deterministic_across_directories() {
                let fixture = fixture();
                assert_eq!(fixture.baseline, fixture.repeated);
                assert!(
                    fixture.large == fixture.repeated_large,
                    "large output changed"
                );
            }

            #[test]
            fn password_and_size_change_the_output() {
                let fixture = fixture();
                assert_ne!(fixture.baseline, fixture.different_password);
                assert!(!fixture.different_size.starts_with(&fixture.baseline));
                assert!(!fixture.large.starts_with(&fixture.baseline));
            }

            #[test]
            fn exact_lengths_replace_existing_files_without_temporary_files() {
                let fixture = fixture();
                // Every generation also checks the directory for leaked temporaries.
                assert_eq!(fixture.baseline.len(), FIXTURE_SIZE as usize);
                assert_eq!(fixture.repeated.len(), FIXTURE_SIZE as usize);
                assert_eq!(fixture.different_password.len(), FIXTURE_SIZE as usize);
                assert_eq!(fixture.different_size.len(), FIXTURE_SIZE as usize + 1);
                assert_eq!(fixture.large.len(), LARGE_SIZE);
                assert_eq!(fixture.repeated_large.len(), LARGE_SIZE);
            }

            #[test]
            fn multi_buffer_output_matches_independent_expansion() {
                assert!(
                    fixture().large == reference_expansion($name),
                    "output differs from a continuous reference stream"
                );
            }

            #[test]
            fn large_key_is_not_a_repeated_short_key() {
                let bytes = &fixture().large;
                // Exhaustively reject every whole-file period from 1 byte through
                // 1 MiB, including periods that do not divide the requested size.
                for period in 1..=crate::STREAM_BUFFER_BYTES {
                    assert!(
                        bytes[period..] != bytes[..bytes.len() - period],
                        "output repeats with period {period}"
                    );
                }
            }

            #[test]
            fn aligned_blocks_do_not_repeat_in_fixed_large_fixture() {
                // A fixed regression fixture, not a promise that arbitrary
                // ChaCha20/BLAKE3 output can never contain a repeated block.
                let bytes = &fixture().large;
                let mut seen = HashSet::with_capacity(bytes.len() / 16);
                for (index, block) in bytes.chunks_exact(16).enumerate() {
                    assert!(seen.insert(block), "repeated 16-byte block at {index}");
                }
            }

            #[test]
            fn invalid_input_and_password_failures_preserve_existing_key() {
                let directory = tempfile::tempdir().unwrap();
                let output_path = directory.path().join(OUTPUT_FILENAME);
                fs::write(&output_path, SENTINEL).unwrap();
                let cases: &[(&str, &[&str])] = &[
                    ("0\n", &[]),
                    ("20000000001\n", &[]),
                    ("not a size\n", &[]),
                    ("", &[]),
                    ("64\n", &["", ""]),
                    ("64\n", &["password", "mismatch"]),
                    ("64\n", &[]),
                    ("64\n", &["password"]),
                ];
                for (size, passwords) in cases {
                    let attempt = attempt($name, directory.path(), size, passwords);
                    assert!(attempt.result.is_err());
                    assert!(!attempt.output.contains("Created key.key"));
                    assert_eq!(fs::read(&output_path).unwrap(), SENTINEL);
                    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                }
            }

            #[test]
            fn failed_replacement_cleans_up_and_reports_no_success() {
                let directory = tempfile::tempdir().unwrap();
                let destination = directory.path().join(OUTPUT_FILENAME);
                fs::create_dir(&destination).unwrap();
                fs::write(destination.join("sentinel"), SENTINEL).unwrap();
                let attempt = attempt(
                    $name,
                    directory.path(),
                    "64\n",
                    &[FIXTURE_PASSWORD, FIXTURE_PASSWORD],
                );
                assert!(matches!(
                    attempt.result,
                    Err(CliError::Generate(crate::GenerateError::Io(_)))
                ));
                assert!(!attempt.output.contains("Created key.key"));
                assert_eq!(fs::read(destination.join("sentinel")).unwrap(), SENTINEL);
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            }
        }
    };
}

command_regressions!(
    argon2_aes,
    "argon2-aes",
    GeneratorSuite::Argon2idAes256Ctr,
    b"versakey/key-file/aes256-ctr/v1",
    [
        144, 129, 36, 143, 81, 37, 153, 95, 36, 50, 200, 67, 211, 94, 54, 137, 115, 251, 54, 75,
        215, 49, 172, 129, 143, 184, 175, 125, 104, 22, 118, 171, 211, 150, 14, 128, 153, 149, 229,
        253, 206, 101, 200, 148, 186, 69, 133, 115, 84, 171, 201, 8, 248, 121, 144, 94, 122, 153,
        112, 201, 190, 78, 55, 49,
    ]
);

command_regressions!(
    scrypt_aes,
    "scrypt-aes",
    GeneratorSuite::ScryptAes256Ctr,
    b"versakey/key-file/scrypt-aes256-ctr/v1",
    [
        178, 180, 153, 225, 64, 190, 31, 96, 102, 35, 101, 219, 231, 53, 51, 159, 131, 229, 223,
        104, 81, 90, 75, 27, 175, 167, 224, 127, 2, 172, 241, 140, 98, 160, 189, 194, 30, 196, 57,
        218, 195, 155, 190, 158, 112, 61, 171, 104, 5, 169, 208, 90, 184, 4, 133, 37, 156, 186, 18,
        9, 131, 33, 220, 253,
    ]
);

command_regressions!(
    pbkdf2_aes,
    "pbkdf2-aes",
    GeneratorSuite::Pbkdf2Sha256Aes256Ctr,
    b"versakey/key-file/pbkdf2-sha256-aes256-ctr/v1",
    [
        117, 8, 19, 69, 47, 161, 193, 120, 73, 195, 197, 166, 9, 186, 227, 239, 152, 200, 241, 133,
        54, 234, 87, 173, 197, 65, 198, 42, 24, 133, 219, 101, 192, 164, 209, 158, 248, 28, 245,
        137, 232, 38, 204, 16, 125, 187, 215, 8, 207, 167, 191, 130, 135, 147, 80, 13, 65, 185, 99,
        11, 127, 220, 93, 231,
    ]
);

command_regressions!(
    argon2_chacha20,
    "argon2-chacha20",
    GeneratorSuite::Argon2idChaCha20,
    b"versakey/key-file/argon2id-chacha20/v1",
    [
        52, 31, 122, 47, 231, 144, 90, 180, 32, 206, 106, 80, 79, 14, 19, 205, 111, 185, 148, 89,
        103, 69, 173, 128, 168, 216, 233, 145, 79, 134, 90, 48, 235, 73, 190, 233, 10, 250, 242,
        118, 158, 14, 121, 98, 152, 155, 30, 231, 207, 79, 1, 43, 83, 62, 97, 215, 79, 94, 40, 35,
        136, 95, 239, 219,
    ]
);

command_regressions!(
    argon2_blake3,
    "argon2-blake3",
    GeneratorSuite::Argon2idBlake3Xof,
    b"versakey/key-file/argon2id-blake3-xof/v1",
    [
        11, 234, 63, 217, 88, 200, 243, 144, 182, 165, 220, 57, 194, 119, 187, 186, 47, 178, 114,
        195, 234, 137, 108, 253, 112, 3, 240, 162, 169, 188, 207, 49, 17, 236, 122, 91, 223, 207,
        205, 78, 55, 112, 95, 136, 121, 159, 52, 39, 142, 90, 137, 89, 113, 224, 108, 141, 191, 45,
        183, 33, 211, 211, 237, 19,
    ]
);

#[test]
fn subcommands_have_distinct_domains_and_outputs() {
    let mut domains = HashSet::new();
    let mut outputs = HashSet::new();
    let directory = tempfile::tempdir().unwrap();
    for command in &COMMANDS {
        assert!(domains.insert(command.domain));
        let bytes = generate(
            command.name,
            directory.path(),
            FIXTURE_PASSWORD,
            FIXTURE_SIZE,
        );
        assert!(
            outputs.insert(bytes),
            "two subcommands produced the same key"
        );
    }
}
