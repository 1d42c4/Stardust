use super::*;
use std::cell::Cell;
use std::io::{BufReader, BufWriter, Cursor};

// Every password here is synthetic test data, supplied through the hidden-input seam.
const PASSWORD: &str = "synthetic test password";

#[test]
fn final_flush_failure_reports_that_the_key_was_created() {
    struct FlushFailure<'a>(&'a Cell<bool>);
    impl Write for FlushFailure<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.0.get() {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(())
            }
        }
    }
    let committed = Cell::new(false);
    let error = run_with(
        &mut Cursor::new(b"8\n"),
        &mut FlushFailure(&committed),
        |_| Ok(PASSWORD.into()),
        |_, _| {
            committed.set(true);
            Ok(())
        },
    )
    .expect_err("a final flush failure must not be silently discarded");
    assert!(committed.get());
    assert!(matches!(error, CliError::CommittedButStatusWriteFailed(_)));
    assert!(error.to_string().contains("key.key was created"));
}

#[test]
fn success_status_reaches_a_buffered_writer_before_returning() {
    let mut writer = BufWriter::with_capacity(4096, Vec::new());
    run_with(
        &mut Cursor::new(b"8\n"),
        &mut writer,
        |_| Ok(PASSWORD.into()),
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(writer.buffer().is_empty());
    assert!(String::from_utf8_lossy(writer.get_ref()).contains("Created key.key (8 bytes)."));
}

fn successful_input(bytes: &[u8]) -> (u64, u64) {
    let mut input = Cursor::new(bytes);
    let mut size = 0;
    run_with(
        &mut input,
        &mut Vec::new(),
        |_| Ok(PASSWORD.into()),
        |actual, password| {
            assert_eq!(password.as_str(), PASSWORD);
            size = actual;
            Ok(())
        },
    )
    .unwrap();
    (size, input.position())
}

#[test]
fn crlf_size_input_is_accepted() {
    assert_eq!(successful_input(b"17\r\n"), (17, 4));
}

#[test]
fn size_at_eof_without_newline_is_accepted() {
    assert_eq!(successful_input(b"17"), (17, 2));
}

#[test]
fn size_reader_leaves_following_lines_unconsumed() {
    assert_eq!(successful_input(b"17\n999\n"), (17, 3));
}

#[test]
fn maximum_length_leading_zero_input_preserves_the_value() {
    let input = format!("{}1\n", "0".repeat(126));
    assert_eq!(successful_input(input.as_bytes()), (1, 128));
}

#[test]
fn unicode_whitespace_is_counted_in_bytes_at_the_input_limit() {
    let input = format!("{}1\n", "\u{2003}".repeat(42));
    assert_eq!(successful_input(input.as_bytes()), (1, 128));
    let too_long = format!("{} 1\n", "\u{2003}".repeat(42));
    let error = run_with(
        &mut Cursor::new(too_long),
        &mut Vec::new(),
        |_| panic!("oversized input must not prompt"),
        |_, _| panic!("oversized input must not generate"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::SizeInputTooLong));
}

#[test]
fn invalid_utf8_size_fails_before_password_or_generation() {
    let error = run_with(
        &mut Cursor::new(b"1\xff\n"),
        &mut Vec::new(),
        |_| panic!("invalid UTF-8 must not prompt"),
        |_, _| panic!("invalid UTF-8 must not generate"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::Io(ref e) if e.kind() == io::ErrorKind::InvalidData));
}

#[test]
fn infinite_size_input_stops_after_129_bytes() {
    struct Endless<'a>(&'a Cell<usize>);
    impl Read for Endless<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            bytes.fill(b'1');
            self.0.set(self.0.get() + bytes.len());
            Ok(bytes.len())
        }
    }
    let consumed = Cell::new(0);
    // Capacity one avoids read-ahead so the limit can be measured exactly.
    let mut input = BufReader::with_capacity(1, Endless(&consumed));
    let error = run_with(
        &mut input,
        &mut Vec::new(),
        |_| panic!("unbounded input must not prompt"),
        |_, _| panic!("unbounded input must not generate"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::SizeInputTooLong));
    assert_eq!(consumed.get(), 129);
}

#[test]
fn interrupted_size_reads_are_retried_without_losing_digits() {
    struct Choppy {
        bytes: &'static [u8],
        interrupt: bool,
    }
    impl Read for Choppy {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.interrupt = !self.interrupt;
            if self.interrupt {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = bytes.len().min(1);
            self.bytes.read(&mut bytes[..count])
        }
    }
    let mut input = BufReader::new(Choppy {
        bytes: b"123\n",
        interrupt: false,
    });
    run_with(
        &mut input,
        &mut Vec::new(),
        |_| Ok(PASSWORD.into()),
        |size, _| {
            assert_eq!(size, 123);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn read_failure_after_size_digits_does_not_use_a_partial_number() {
    struct Partial(bool);
    impl Read for Partial {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            self.0 = true;
            bytes[0] = b'8';
            Ok(1)
        }
    }
    let error = run_with(
        &mut BufReader::new(Partial(false)),
        &mut Vec::new(),
        |_| panic!("failed size read must not prompt"),
        |_, _| panic!("failed size read must not generate"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::Io(ref e) if e.kind() == io::ErrorKind::ConnectionReset));
}

struct FailFlush {
    calls: usize,
    fail_at: usize,
}
impl Write for FailFlush {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.calls += 1;
        if self.calls == self.fail_at {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn size_prompt_flush_failure_does_not_consume_input() {
    let mut input = Cursor::new(b"8\n");
    let error = run_with(
        &mut input,
        &mut FailFlush {
            calls: 0,
            fail_at: 1,
        },
        |_| panic!("prompt failed"),
        |_, _| panic!("prompt failed"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::Io(_)));
    assert_eq!(input.position(), 0);
}

#[test]
fn generating_message_flush_failure_prevents_generation() {
    let prompts = Cell::new(0);
    let error = run_with(
        &mut Cursor::new(b"8\n"),
        &mut FailFlush {
            calls: 0,
            fail_at: 2,
        },
        |_| {
            prompts.set(prompts.get() + 1);
            Ok(PASSWORD.into())
        },
        |_, _| panic!("generation must not start after flush failure"),
    )
    .unwrap_err();
    assert_eq!(prompts.get(), 2);
    assert!(matches!(error, CliError::Io(_)));
}

#[test]
fn partial_and_interrupted_console_writes_preserve_complete_status() {
    struct Choppy {
        bytes: Vec<u8>,
        interrupt: bool,
    }
    impl Write for Choppy {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.interrupt = !self.interrupt;
            if self.interrupt {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = bytes.len().min(3);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Choppy {
        bytes: Vec::new(),
        interrupt: false,
    };
    run_with(
        &mut Cursor::new(b"8\n"),
        &mut writer,
        |_| Ok(PASSWORD.into()),
        |_, _| Ok(()),
    )
    .unwrap();
    let status = String::from_utf8(writer.bytes).unwrap();
    assert!(status.ends_with("Created key.key (8 bytes).\n"));
    assert!(!status.contains(PASSWORD));
}

#[test]
fn zero_length_console_write_fails_before_reading_size() {
    struct Zero;
    impl Write for Zero {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Ok(0)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut input = Cursor::new(b"8\n");
    let error = run_with(
        &mut input,
        &mut Zero,
        |_| panic!("console failed"),
        |_, _| panic!("console failed"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::Io(ref e) if e.kind() == io::ErrorKind::WriteZero));
    assert_eq!(input.position(), 0);
}

#[test]
fn whitespace_only_password_is_preserved_as_nonempty() {
    run_with(
        &mut Cursor::new(b"1\n"),
        &mut Vec::new(),
        |_| Ok(" \t ".into()),
        |_, password| {
            assert_eq!(password.as_str(), " \t ");
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn embedded_nul_password_is_not_truncated() {
    run_with(
        &mut Cursor::new(b"1\n"),
        &mut Vec::new(),
        |_| Ok("before\0after".into()),
        |_, password| {
            assert_eq!(password.as_bytes(), b"before\0after");
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn canonically_equivalent_unicode_passwords_must_match_exact_bytes() {
    let mut passwords = ["\u{e9}", "e\u{301}"].into_iter();
    let error = run_with(
        &mut Cursor::new(b"1\n"),
        &mut Vec::new(),
        |_| Ok(passwords.next().unwrap().into()),
        |_, _| panic!("password normalization must not be implicit"),
    )
    .unwrap_err();
    assert!(matches!(error, CliError::PasswordMismatch));
}

#[test]
fn generator_panic_never_prints_a_success_message() {
    let mut output = Vec::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = run_with(
            &mut Cursor::new(b"1\n"),
            &mut output,
            |_| Ok(PASSWORD.into()),
            |_, _| panic!("injected generator panic"),
        );
    }));
    assert!(result.is_err());
    assert!(
        !String::from_utf8(output)
            .unwrap()
            .contains("Created key.key")
    );
}

#[test]
fn committed_directory_sync_error_is_not_reported_as_success() {
    let mut output = Vec::new();
    let error = run_with(
        &mut Cursor::new(b"1\n"),
        &mut output,
        |_| Ok(PASSWORD.into()),
        |_, _| {
            Err(
                crate::keygen::GenerateError::CommittedButDirectorySyncFailed(io::Error::other(
                    "injected sync failure",
                )),
            )
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("key.key was replaced"));
    assert!(
        !String::from_utf8(output)
            .unwrap()
            .contains("Created key.key")
    );
}
