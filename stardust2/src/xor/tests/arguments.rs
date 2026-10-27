use super::*;

fn parse(args: &[&str]) -> Result<Action, String> {
    parse_args(args.iter().map(OsString::from))
}

#[test]
fn two_filenames_select_fixed_key_processing() {
    assert_eq!(
        parse(&["a.bin", "out.bin"]).unwrap(),
        Action::Run(Args {
            a: "a.bin".into(),
            out: Some("out.bin".into())
        })
    );
}

#[test]
fn in_place_is_explicit_and_has_one_filename() {
    for args in [vec!["--in-place", "a.bin"], vec!["a.bin", "--in-place"]] {
        let action = parse(&args).unwrap();
        assert_eq!(
            action,
            Action::Run(Args {
                a: "a.bin".into(),
                out: None
            })
        );
        let Action::Run(args) = action else {
            unreachable!()
        };
        assert_eq!(args.output_name(), "a.bin");
    }
    for args in [vec!["--in-place"], vec!["--in-place", "a", "out"]] {
        assert!(parse(&args).unwrap_err().contains("expected 1 filename"));
    }
    assert!(parse(&["--in-place", "--in-place", "a"]).is_err());
}

#[test]
fn in_place_terminator_preserves_literal_option_and_native_names() {
    for name in [
        OsString::from("--in-place"),
        crate::xor::test_support::native_name(),
    ] {
        assert_eq!(
            parse_args(["--in-place".into(), "--".into(), name.clone()]).unwrap(),
            Action::Run(Args { a: name, out: None })
        );
    }
    assert_eq!(
        parse(&["--", "--in-place", "out"]).unwrap(),
        Action::Run(Args {
            a: "--in-place".into(),
            out: Some("out".into())
        })
    );
}

#[test]
fn in_place_uses_the_same_filename_validation() {
    for name in [
        "../a",
        "a/b",
        "a\\b",
        "",
        "NUL",
        "a:stream",
        ".astra-xor-temp",
    ] {
        assert!(parse(&["--in-place", name]).is_err(), "{name}");
    }
}

#[test]
fn help_aliases() {
    for flag in ["-h", "--help"] {
        assert_eq!(parse(&[flag]).unwrap(), Action::Help);
    }
}

#[test]
fn version_aliases() {
    for flag in ["-V", "--version"] {
        assert_eq!(parse(&[flag]).unwrap(), Action::Version);
    }
}

#[test]
fn rejects_wrong_argument_counts_including_old_cli() {
    for args in [
        vec![],
        vec!["a"],
        vec!["a", "key.key", "out"],
        vec!["a", "b", "c", "d"],
    ] {
        assert!(parse(&args).unwrap_err().contains("expected 2 filenames"));
    }
}

#[test]
fn rejects_all_removed_modes() {
    for flag in ["-t", "--truncate", "-r", "--repeat", "--unknown"] {
        assert!(
            parse(&[flag, "a", "out"])
                .unwrap_err()
                .contains("unknown option")
        );
    }
}

#[test]
fn terminator_allows_option_like_names() {
    assert_eq!(
        parse(&["--", "--help", "--"]).unwrap(),
        Action::Run(Args {
            a: "--help".into(),
            out: Some("--".into())
        })
    );
}

#[test]
fn lone_dash_is_a_filename() {
    assert!(matches!(parse(&["-", "out"]), Ok(Action::Run(_))));
}

#[test]
fn spaces_and_unicode_are_preserved() {
    assert_eq!(
        parse(&["input one 鍵.bin", "résultat.bin"]).unwrap(),
        Action::Run(Args {
            a: "input one 鍵.bin".into(),
            out: Some("résultat.bin".into())
        })
    );
}

#[test]
fn native_non_unicode_names_are_preserved() {
    let name = crate::xor::test_support::native_name();
    assert_eq!(
        parse_args([name.clone(), "out".into()]).unwrap(),
        Action::Run(Args {
            a: name,
            out: Some("out".into())
        })
    );
}

macro_rules! reject_names {
    ($test:ident, $($name:expr),+ $(,)?) => {
        #[test]
        fn $test() {
            for name in [$($name),+] {
                assert!(parse(&[name, "out"]).is_err(), "input {name:?}");
                assert!(parse(&["a", name]).is_err(), "output {name:?}");
            }
        }
    };
}

reject_names!(empty_and_dot_names, "", ".", "..");
reject_names!(unix_paths, "./a", "../a", "/tmp/a", "folder/a", "a/");
reject_names!(
    windows_paths,
    "C:\\a",
    "C:a",
    "\\a",
    "folder\\a",
    "..\\a",
    "\\\\server\\share\\a",
    "\\\\?\\C:\\a"
);
reject_names!(
    device_names,
    "NUL",
    "nul.txt",
    "CON",
    "con .txt",
    "AUX",
    "PRN",
    "CONIN$",
    "CONOUT$",
    "COM1",
    "LPT9.bin",
    "COM¹",
    "LPT².bin",
    "COM³"
);
reject_names!(
    windows_special_characters,
    "file:stream",
    "a<b",
    "a>b",
    "a\"b",
    "a|b",
    "a?b",
    "a*b"
);
reject_names!(
    control_characters,
    "a\0b",
    "a\nb",
    "a\rb",
    "a\tb",
    "a\u{1f}b"
);
reject_names!(trailing_spaces_and_dots, "a ", "a.", "a..", "a. ");
reject_names!(
    internal_temporary_names,
    ".astra-xor-1.tmp",
    ".ASTRA-XOR-old"
);

#[test]
fn ordinary_device_like_names_are_allowed() {
    for name in [
        "COM10",
        "LPT0",
        "null",
        "conifer.txt",
        "key.key",
        ".hidden",
        "a..b",
        " leading",
    ] {
        validate_name(OsStr::new(name)).unwrap();
    }
}

#[test]
fn help_describes_the_complete_new_contract() {
    for text in [
        "<FILE_A> <OUTPUT>",
        "key.key",
        "beside the executable",
        "at least as long",
        "existing output",
        "--in-place [--] <FILE_A>",
        "temporary disk",
    ] {
        assert!(USAGE.contains(text), "{text}");
    }
    assert!(!USAGE.contains("<FILE_B>"));
}
