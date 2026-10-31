use stardust::{
    app::{Action, Environment, parse_args, run_with},
    keygen::commands::COMMANDS,
};
use std::{
    ffi::OsString,
    fs,
    io::{self, Cursor, Write},
    path::Path,
};

fn run_without_side_effects(arguments: &[&str]) -> Result<Vec<u8>, stardust::app::AppError> {
    let mut output = Vec::new();
    run_with(
        arguments.iter().map(OsString::from),
        &Environment {
            working_directory: Path::new("nonexistent-stardust-working-directory"),
            executable: Path::new("nonexistent-stardust-executable"),
        },
        &mut Cursor::new(Vec::<u8>::new()),
        &mut output,
        |_| panic!("unexpected password prompt"),
    )?;
    Ok(output)
}

#[test]
fn root_help_lists_all_six_operations_and_both_location_policies() {
    for arguments in [&["--help"][..], &["-h"], &["help"]] {
        let output = String::from_utf8(run_without_side_effects(arguments).unwrap()).unwrap();
        for command in &COMMANDS {
            assert!(output.contains(command.name));
        }
        for text in [
            "stardust xor",
            "--in-place",
            "current directory",
            "beside the executable",
        ] {
            assert!(output.contains(text), "missing {text}");
        }
    }
}

#[test]
fn xor_help_aliases_explain_the_same_contract_without_files() {
    let expected = run_without_side_effects(&["xor", "--help"]).unwrap();
    for arguments in [&["help", "xor"][..], &["xor", "-h"]] {
        assert_eq!(run_without_side_effects(arguments).unwrap(), expected);
    }
}

#[test]
fn global_version_does_not_require_a_directory_or_terminal() {
    for argument in ["--version", "-V"] {
        assert_eq!(
            run_without_side_effects(&[argument]).unwrap(),
            format!("{}\n", stardust::app::VERSION).as_bytes()
        );
    }
}

#[test]
fn each_generator_routes_to_its_original_configuration() {
    for expected in &COMMANDS {
        let Action::Generate(actual) = parse_args([expected.name.into()]).unwrap() else {
            panic!("wrong route")
        };
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.suite, expected.suite);
        assert_eq!(
            actual.config().application_salt,
            expected.config().application_salt
        );
        assert_eq!(actual.domain, expected.domain);
    }
}

#[test]
fn invalid_global_commands_never_read_input_or_echo_secrets() {
    let cases: &[&[&str]] = &[
        &[],
        &["UNKNOWN-SECRET"],
        &["ARGON2-AES"],
        &["key"],
        &["help", "xor", "extra"],
        &["--help", "UNKNOWN-SECRET"],
        &["--version", "UNKNOWN-SECRET"],
        &["argon2-aes", "--password", "UNKNOWN-SECRET"],
        &["scrypt-aes", "64"],
        &["pbkdf2-aes", "xor"],
        &["argon2-chacha20", "--in-place"],
        &["argon2-blake3", "--help", "extra"],
    ];
    for arguments in cases {
        let error = run_without_side_effects(arguments).unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert!(!error.to_string().contains("UNKNOWN-SECRET"));
    }
}

#[test]
fn malformed_xor_commands_retain_astra_usage_exit_code() {
    for arguments in [
        &["xor"][..],
        &["xor", "input"],
        &["xor", "--in-place", "input", "output"],
        &["xor", "../escape", "out"],
    ] {
        let error = run_without_side_effects(arguments).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("stardust xor"));
    }
}

#[test]
fn command_names_remain_literal_xor_filenames() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("program");
    fs::write(&executable, b"protected executable").unwrap();
    fs::write(directory.path().join("key.key"), [0xa0]).unwrap();
    let mut names: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
    names.extend(["xor", "help", "--help", "--version"]);
    for name in names {
        fs::write(directory.path().join(name), [0x0f]).unwrap();
        let mut output = Vec::new();
        run_with(
            ["xor", "--", name, "out"].map(OsString::from),
            &Environment {
                working_directory: Path::new("missing"),
                executable: &executable,
            },
            &mut Cursor::new(Vec::<u8>::new()),
            &mut output,
            |_| panic!("filename selected a generator"),
        )
        .unwrap();
        assert_eq!(fs::read(directory.path().join("out")).unwrap(), [0xaf]);
    }
}

struct BrokenOutput;
impl Write for BrokenOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn broken_generator_status_pipe_fails_before_password_or_generation() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("key.key"), b"sentinel").unwrap();
    for command in &COMMANDS {
        assert!(
            run_with(
                [command.name.into()],
                &Environment {
                    working_directory: directory.path(),
                    executable: Path::new("missing")
                },
                &mut Cursor::new("64\n"),
                &mut BrokenOutput,
                |_| panic!("must fail before password prompt"),
            )
            .is_err()
        );
        assert_eq!(
            fs::read(directory.path().join("key.key")).unwrap(),
            b"sentinel"
        );
    }
}

#[test]
fn closed_status_pipe_does_not_turn_committed_xor_into_failure() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("program");
    fs::write(&executable, b"program").unwrap();
    fs::write(directory.path().join("key.key"), [0x01]).unwrap();
    fs::write(directory.path().join("data"), [0x02]).unwrap();
    run_with(
        ["xor", "--in-place", "data"].map(OsString::from),
        &Environment {
            working_directory: Path::new("missing"),
            executable: &executable,
        },
        &mut Cursor::new(""),
        &mut BrokenOutput,
        |_| panic!("unexpected prompt"),
    )
    .unwrap();
    assert_eq!(fs::read(directory.path().join("data")).unwrap(), [0x03]);
}

#[test]
fn generating_a_key_never_requires_an_executable_file() {
    let directory = tempfile::tempdir().unwrap();
    run_with(
        ["argon2-blake3".into()],
        &Environment {
            working_directory: directory.path(),
            executable: Path::new("missing"),
        },
        &mut Cursor::new("1\n"),
        &mut Vec::new(),
        |_| Ok("test password".to_owned()),
    )
    .unwrap();
    assert_eq!(
        fs::metadata(directory.path().join("key.key"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn wrong_generator_size_is_rejected_before_password_in_combined_dispatch() {
    for command in &COMMANDS {
        for size in ["0\n", "20000000001\n", "1MiB\n", "-1\n", ""] {
            let error = run_with(
                [command.name.into()],
                &Environment {
                    working_directory: Path::new("missing"),
                    executable: Path::new("missing"),
                },
                &mut Cursor::new(size),
                &mut Vec::new(),
                |_| panic!("invalid size prompted for password"),
            )
            .unwrap_err();
            assert_eq!(error.exit_code(), 1);
        }
    }
}

#[test]
fn native_non_unicode_subcommand_is_rejected_without_lossy_dispatch() {
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'x', 0xff])
    };
    #[cfg(windows)]
    let name = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0x78, 0xd800])
    };
    assert!(parse_args([name]).is_err());
}

#[test]
fn final_flush_failure_reports_commit_and_keeps_each_generated_key() {
    struct FinalFlushFailure(usize);
    impl Write for FinalFlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0 += 1;
            if self.0 == 3 {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(())
            }
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("key.key");
    for command in &COMMANDS {
        fs::write(&key, b"old key sentinel").unwrap();
        let error = run_with(
            [command.name.into()],
            &Environment {
                working_directory: directory.path(),
                executable: Path::new("missing"),
            },
            &mut Cursor::new("32\n"),
            &mut FinalFlushFailure(0),
            |_| Ok("synthetic integration password".into()),
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().contains("key.key was created"));
        let committed = fs::read(&key).unwrap();
        assert_eq!(committed.len(), 32);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        run_with(
            [command.name.into()],
            &Environment {
                working_directory: directory.path(),
                executable: Path::new("missing"),
            },
            &mut Cursor::new("32\n"),
            &mut Vec::new(),
            |_| Ok("synthetic integration password".into()),
        )
        .unwrap();
        assert_eq!(fs::read(&key).unwrap(), committed);
    }
}

#[test]
fn every_generator_accepts_a_native_non_unicode_working_directory() {
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'k', 0xff])
    };
    #[cfg(windows)]
    let name = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0x6b, 0xd800])
    };
    let directory = tempfile::tempdir().unwrap();
    let native = directory.path().join(name);
    fs::create_dir(&native).unwrap();
    for command in &COMMANDS {
        run_with(
            [command.name.into()],
            &Environment {
                working_directory: &native,
                executable: Path::new("missing"),
            },
            &mut Cursor::new("17\n"),
            &mut Vec::new(),
            |_| Ok("synthetic path password".into()),
        )
        .unwrap();
        assert_eq!(fs::read(native.join("key.key")).unwrap().len(), 17);
        assert_eq!(fs::read_dir(&native).unwrap().count(), 1);
        assert!(!directory.path().join("key.key").exists());
    }
}

#[test]
fn confirmation_read_failure_preserves_the_existing_key_and_hard_link() {
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("key.key");
    let alias = directory.path().join("alias");
    fs::write(&key, b"original key sentinel").unwrap();
    fs::hard_link(&key, &alias).unwrap();
    for command in &COMMANDS {
        let mut prompts = 0;
        let mut output = Vec::new();
        let error = run_with(
            [command.name.into()],
            &Environment {
                working_directory: directory.path(),
                executable: Path::new("missing"),
            },
            &mut Cursor::new("17\n"),
            &mut output,
            |_| {
                prompts += 1;
                if prompts == 1 {
                    Ok("synthetic password".into())
                } else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert_eq!(prompts, 2);
        assert_eq!(fs::read(&key).unwrap(), b"original key sentinel");
        assert_eq!(fs::read(&alias).unwrap(), b"original key sentinel");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("Generating key.key")
        );
    }
    fs::write(&alias, b"still linked").unwrap();
    assert_eq!(fs::read(&key).unwrap(), b"still linked");
}
