use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const SENTINEL_KEY: &[u8] = b"preexisting key material must survive CLI validation errors";
const EXPECTED_PROMPT_AFTER_SUITE: &str =
    "Stardust deterministic key maker (1 to 20000000000 bytes)\nKey size in bytes: ";
const OVERSIZED_SIZE_INPUT: [u8; 129] = [b' '; 129];
const COMMANDS: [(&str, &str); 5] = [
    ("argon2-aes", "Argon2id + AES-256-CTR"),
    ("scrypt-aes", "scrypt + AES-256-CTR"),
    ("pbkdf2-aes", "PBKDF2-HMAC-SHA-256 + AES-256-CTR"),
    ("argon2-chacha20", "Argon2id + ChaCha20"),
    ("argon2-blake3", "Argon2id + keyed BLAKE3 XOF"),
];

struct FailureCase {
    name: &'static str,
    stdin: &'static [u8],
    expected_stderr: &'static str,
}

const FAILURE_CASES: &[FailureCase] = &[
    FailureCase {
        name: "zero size",
        stdin: b"0\n",
        expected_stderr: "Error: key size must be between 1 and 20000000000 bytes\n",
    },
    FailureCase {
        name: "above maximum",
        stdin: b"20000000001\n",
        expected_stderr: "Error: key size must be between 1 and 20000000000 bytes\n",
    },
    FailureCase {
        name: "non-decimal size",
        stdin: b"1KB\n",
        expected_stderr: "Error: key size must contain decimal digits only (bytes, with no unit suffix)\n",
    },
    FailureCase {
        name: "end of input",
        stdin: b"",
        expected_stderr: "Error: console input/output failed: no key size was provided\n",
    },
    FailureCase {
        name: "oversized input",
        stdin: &OVERSIZED_SIZE_INPUT,
        expected_stderr: "Error: key size input is too long\n",
    },
];

fn run_with_stdin(arguments: &[&str], current_dir: &Path, stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stardust"))
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch versakey");

    child
        .stdin
        .take()
        .expect("piped child stdin")
        .write_all(stdin)
        .expect("write child stdin");

    child.wait_with_output().expect("wait for child process")
}

fn assert_existing_key_untouched(directory: &Path) {
    assert_eq!(
        std::fs::read(directory.join("key.key")).unwrap(),
        SENTINEL_KEY
    );
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
}

fn assert_cli_validation_failures(command: &str, suite_name: &str) {
    for case in FAILURE_CASES {
        let directory = tempfile::tempdir().expect("temporary working directory");
        std::fs::write(directory.path().join("key.key"), SENTINEL_KEY).unwrap();

        let output = run_with_stdin(&[command], directory.path(), case.stdin);
        let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");

        assert!(!output.status.success(), "{command}: {}", case.name);
        let expected_stdout =
            format!("Stardust suite: {suite_name}\n{EXPECTED_PROMPT_AFTER_SUITE}");
        assert_eq!(stdout, expected_stdout, "{command}: {}", case.name);
        assert_eq!(stderr, case.expected_stderr, "{command}: {}", case.name);

        for forbidden in [
            "Password:",
            "Confirm password:",
            "Generating key.key",
            "Created key.key",
        ] {
            assert!(
                !stdout.contains(forbidden) && !stderr.contains(forbidden),
                "{command} printed {forbidden:?} for {}",
                case.name
            );
        }

        assert_existing_key_untouched(directory.path());
    }
}

macro_rules! validation_test {
    ($test:ident, $index:expr) => {
        #[test]
        fn $test() {
            let (command, suite) = COMMANDS[$index];
            assert_cli_validation_failures(command, suite);
        }
    };
}

validation_test!(argon2_aes_validates_before_password_or_file_access, 0);
validation_test!(scrypt_aes_validates_before_password_or_file_access, 1);
validation_test!(pbkdf2_aes_validates_before_password_or_file_access, 2);
validation_test!(argon2_chacha20_validates_before_password_or_file_access, 3);
validation_test!(argon2_blake3_validates_before_password_or_file_access, 4);

#[test]
fn missing_unknown_or_extra_arguments_never_default_to_a_generator() {
    let cases: &[&[&str]] = &[
        &[],
        &["unknown"],
        &["argon2"],
        &["ARGON2-AES"],
        &["--unknown"],
        &["help", "unknown"],
        &["--help", "extra"],
        &["--version", "extra"],
        &["argon2-aes", "scrypt-aes"],
        &["scrypt-aes", "64"],
        &["pbkdf2-aes", "--password", "NEVER-ECHO-THIS"],
        &["argon2-chacha20", "--help", "extra"],
        &["argon2-blake3", "NEVER-ECHO-THIS"],
    ];
    for arguments in cases {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("key.key"), SENTINEL_KEY).unwrap();
        let output = run_with_stdin(arguments, directory.path(), b"");
        assert!(!output.status.success(), "arguments {arguments:?}");
        assert!(output.stdout.is_empty(), "arguments {arguments:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("Run 'stardust --help' for usage."));
        assert!(!stderr.contains("NEVER-ECHO-THIS"));
        assert_existing_key_untouched(directory.path());
    }
}

#[test]
fn help_and_version_exit_successfully_without_prompting_or_writing() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("key.key"), SENTINEL_KEY).unwrap();
    for arguments in [["--help"], ["-h"], ["help"]] {
        let output = run_with_stdin(&arguments, directory.path(), b"");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Usage: stardust <subcommand>"));
        for (command, suite) in COMMANDS {
            assert!(stdout.contains(command));
            assert!(stdout.contains(suite));
        }
        assert!(!stdout.contains("Key size in bytes:"));
        assert_existing_key_untouched(directory.path());
    }
    for argument in ["--version", "-V"] {
        let output = run_with_stdin(&[argument], directory.path(), b"");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("stardust {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert_existing_key_untouched(directory.path());
    }
}

#[test]
fn each_subcommand_has_help_without_starting_generation() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("key.key"), SENTINEL_KEY).unwrap();
    for (command, suite) in COMMANDS {
        for arguments in [[command, "--help"], [command, "-h"], ["help", command]] {
            let output = run_with_stdin(&arguments, directory.path(), b"");
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.contains(&format!("Usage: stardust {command}")));
            assert!(stdout.contains(suite));
            assert!(!stdout.contains("Key size in bytes:"));
            assert_existing_key_untouched(directory.path());
        }
    }
}
