//! End-to-end use of both domains. Generation uses the real application
//! controller with only hidden terminal input substituted; XOR runs the binary.
use stardust::app::{Environment, run_with};
use std::{
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::Mutex,
};

static PROCESS_SETUP: Mutex<()> = Mutex::new(());
const PASSWORD: &str = "combined workflow password \u{937} with spaces";

struct App {
    directory: tempfile::TempDir,
    executable: PathBuf,
}

impl App {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory
            .path()
            .join(format!("stardust{}", std::env::consts::EXE_SUFFIX));
        // Avoid Linux ETXTBSY from writable descriptors inherited during another
        // test's fork. Only copy/spawn is serialized, never the running programs.
        {
            let _guard = PROCESS_SETUP.lock().unwrap();
            fs::copy(env!("CARGO_BIN_EXE_stardust"), &executable).unwrap();
        }
        Self {
            directory,
            executable,
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn generate(&self, command: &str, directory: &Path, size: usize) -> Vec<u8> {
        let mut output = Vec::new();
        let mut prompts = Vec::new();
        run_with(
            [command.into()],
            &Environment {
                working_directory: directory,
                executable: &self.executable,
            },
            &mut Cursor::new(format!("{size}\n")),
            &mut output,
            |prompt| {
                prompts.push(prompt.to_owned());
                Ok(PASSWORD.to_owned())
            },
        )
        .unwrap();
        assert_eq!(prompts, ["Password: ", "Confirm password: "]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(&format!("Created key.key ({size} bytes).")));
        assert!(!output.contains(PASSWORD));
        let bytes = fs::read(directory.join("key.key")).unwrap();
        assert_eq!(bytes.len(), size);
        assert_clean(directory);
        bytes
    }

    fn invoke(&self, cwd: &Path, arguments: &[&str]) -> Output {
        let child = {
            let _guard = PROCESS_SETUP.lock().unwrap();
            Command::new(&self.executable)
                .current_dir(cwd)
                .args(arguments)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        };
        child.wait_with_output().unwrap()
    }

    fn xor(&self, args: &[&str]) {
        let mut arguments = vec!["xor"];
        arguments.extend_from_slice(args);
        let output = self.invoke(self.path(), &arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_clean(self.path());
    }
}

fn assert_clean(directory: &Path) {
    for entry in fs::read_dir(directory).unwrap() {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.starts_with(".key.key.") && !name.starts_with(".astra-xor-"),
            "leaked {name}"
        );
    }
}

fn payload(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| ((i * 31 + i / 257) % 256) as u8)
        .collect()
}

macro_rules! suite_workflows {
    ($module:ident, $command:literal) => {
        mod $module {
            use super::*;

            #[test]
            fn generated_key_crosses_both_buffer_sizes_and_restores_exactly() {
                let app = App::new();
                let input = payload(2 * 1024 * 1024 + 137);
                // Extra key bytes must stay unused in both XOR directions.
                let key = app.generate($command, app.path(), input.len() + 19);
                fs::write(app.path().join("plain"), &input).unwrap();
                app.xor(&["plain", "masked"]);
                let expected: Vec<_> = input.iter().zip(&key).map(|(a, b)| a ^ b).collect();
                assert_eq!(fs::read(app.path().join("masked")).unwrap(), expected);
                app.xor(&["masked", "restored"]);
                assert_eq!(fs::read(app.path().join("restored")).unwrap(), input);
                assert_eq!(fs::read(app.path().join("plain")).unwrap(), input);
                assert_eq!(fs::read(app.path().join("key.key")).unwrap(), key);
            }

            #[test]
            fn generated_key_supports_atomic_in_place_round_trip() {
                let app = App::new();
                let input = payload(65537);
                let key = app.generate($command, app.path(), input.len());
                fs::write(app.path().join("data"), &input).unwrap();
                app.xor(&["--in-place", "data"]);
                let expected: Vec<_> = input.iter().zip(&key).map(|(a, b)| a ^ b).collect();
                assert_eq!(fs::read(app.path().join("data")).unwrap(), expected);
                app.xor(&["--in-place", "data"]);
                assert_eq!(fs::read(app.path().join("data")).unwrap(), input);
                assert_eq!(fs::read(app.path().join("key.key")).unwrap(), key);
            }

            #[test]
            fn generation_and_xor_retain_their_distinct_directory_rules() {
                let app = App::new();
                let cwd = tempfile::tempdir().unwrap();
                let input = payload(37);
                let executable_key = vec![0xf0; input.len()];
                fs::write(app.path().join("key.key"), &executable_key).unwrap();
                fs::write(app.path().join("data"), &input).unwrap();
                fs::write(cwd.path().join("data"), b"wrong directory").unwrap();
                let generated = app.generate($command, cwd.path(), input.len());
                assert_ne!(generated, executable_key);
                let result = app.invoke(cwd.path(), &["xor", "data", "out"]);
                assert!(result.status.success(), "{:?}", result);
                let expected: Vec<_> = input.iter().map(|byte| byte ^ 0xf0).collect();
                assert_eq!(fs::read(app.path().join("out")).unwrap(), expected);
                assert!(!cwd.path().join("out").exists());
                assert_eq!(fs::read(cwd.path().join("key.key")).unwrap(), generated);
                assert_eq!(
                    fs::read(app.path().join("key.key")).unwrap(),
                    executable_key
                );
            }

            #[test]
            fn regenerated_key_restores_a_previous_ciphertext() {
                let app = App::new();
                let input = payload(64);
                let first = app.generate($command, app.path(), input.len());
                fs::write(app.path().join("plain"), &input).unwrap();
                app.xor(&["plain", "masked"]);
                fs::write(app.path().join("key.key"), b"replace this unrelated key").unwrap();
                let regenerated = app.generate($command, app.path(), input.len());
                assert_eq!(first, regenerated);
                app.xor(&["masked", "restored"]);
                assert_eq!(fs::read(app.path().join("restored")).unwrap(), input);
            }

            #[test]
            fn failed_regeneration_keeps_a_key_usable_for_restoration() {
                let app = App::new();
                let input = payload(64);
                let key = app.generate($command, app.path(), input.len());
                fs::write(app.path().join("data"), &input).unwrap();
                app.xor(&["--in-place", "data"]);
                let mut passwords = ["first password", "different confirmation"].into_iter();
                let mut output = Vec::new();
                let error = run_with(
                    [$command.into()],
                    &Environment {
                        working_directory: app.path(),
                        executable: &app.executable,
                    },
                    &mut Cursor::new("64\n"),
                    &mut output,
                    |_| Ok(passwords.next().unwrap().to_owned()),
                )
                .unwrap_err();
                assert!(error.to_string().contains("passwords did not match"));
                assert!(
                    !String::from_utf8(output)
                        .unwrap()
                        .contains("Created key.key")
                );
                assert_eq!(fs::read(app.path().join("key.key")).unwrap(), key);
                app.xor(&["--in-place", "data"]);
                assert_eq!(fs::read(app.path().join("data")).unwrap(), input);
            }

            #[test]
            fn generated_short_key_cannot_damage_an_input_or_existing_output() {
                let app = App::new();
                let key = app.generate($command, app.path(), 20);
                let input = payload(21);
                fs::write(app.path().join("data"), &input).unwrap();
                fs::write(app.path().join("out"), b"sentinel").unwrap();
                for arguments in [
                    &["xor", "data", "out"][..],
                    &["xor", "--in-place", "data"][..],
                ] {
                    let result = app.invoke(app.path(), arguments);
                    assert_eq!(result.status.code(), Some(1));
                    assert!(
                        String::from_utf8_lossy(&result.stderr).contains("key.key is too short")
                    );
                    assert_eq!(fs::read(app.path().join("out")).unwrap(), b"sentinel");
                    assert_eq!(fs::read(app.path().join("data")).unwrap(), input);
                    assert_eq!(fs::read(app.path().join("key.key")).unwrap(), key);
                    assert_clean(app.path());
                }
            }
        }
    };
}

suite_workflows!(argon2_aes, "argon2-aes");
suite_workflows!(scrypt_aes, "scrypt-aes");
suite_workflows!(pbkdf2_aes, "pbkdf2-aes");
suite_workflows!(argon2_chacha20, "argon2-chacha20");
suite_workflows!(argon2_blake3, "argon2-blake3");

#[test]
fn xor_does_not_prompt_or_generate_when_key_is_missing() {
    let app = App::new();
    fs::write(app.path().join("data"), b"keep").unwrap();
    let error = run_with(
        ["xor".into(), "--in-place".into(), "data".into()],
        &Environment {
            working_directory: app.path(),
            executable: &app.executable,
        },
        &mut Cursor::new("64\n"),
        &mut Vec::new(),
        |_| -> io::Result<String> { panic!("XOR must never request a password") },
    )
    .unwrap_err();
    assert!(error.to_string().contains("key.key"));
    assert!(!app.path().join("key.key").exists());
    assert_eq!(fs::read(app.path().join("data")).unwrap(), b"keep");
    assert_clean(app.path());
}
