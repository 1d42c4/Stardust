//! Explicit generator subcommands and the shared interactive interface.

use crate::{
    GeneratorConfig, GeneratorSuite, MAX_KEY_BYTES, OUTPUT_FILENAME, generate_key_file_with_suite,
    parse_size,
};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use zeroize::Zeroizing;

const MAX_SIZE_INPUT_BYTES: u64 = 128;

// These values and each command's domain preserve the five original binaries'
// output. Renaming a command must never change its cryptographic configuration.
const APPLICATION_SALT: &[u8] = b"versakey application salt 2026-08-11 v1";
const APPLICATION_PEPPER: &[u8] = b"VersaKey-build-pepper-change-me-v1";

struct GeneratorCommand {
    name: &'static str,
    suite: GeneratorSuite,
    domain: &'static [u8],
}

impl GeneratorCommand {
    fn config(&self) -> GeneratorConfig<'static> {
        GeneratorConfig {
            application_salt: APPLICATION_SALT,
            application_pepper: APPLICATION_PEPPER,
            domain: self.domain,
        }
    }
}

const COMMANDS: [GeneratorCommand; 5] = [
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

enum Action {
    Generate(&'static GeneratorCommand),
    Help(Option<&'static GeneratorCommand>),
    Version,
}

fn command_named(name: &OsStr) -> Result<&'static GeneratorCommand, CliError> {
    COMMANDS
        .iter()
        .find(|command| name == command.name)
        .ok_or(CliError::Usage("unknown generator subcommand"))
}

fn parse_args<I, S>(args: I) -> Result<Action, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter();
    let first = args
        .next()
        .ok_or(CliError::Usage("a generator subcommand is required"))?;
    let second = args.next();
    let unexpected = CliError::Usage(
        "unexpected arguments; enter the size and password at the interactive prompts",
    );
    if args.next().is_some() {
        return Err(unexpected);
    }
    let second = second.as_ref().map(AsRef::as_ref);
    match first.as_ref().to_str() {
        Some("--help" | "-h") if second.is_none() => Ok(Action::Help(None)),
        Some("--version" | "-V") if second.is_none() => Ok(Action::Version),
        Some("help") => Ok(Action::Help(second.map(command_named).transpose()?)),
        Some("--help" | "-h" | "--version" | "-V") => Err(unexpected),
        _ => {
            let command = command_named(first.as_ref())?;
            match second {
                None => Ok(Action::Generate(command)),
                Some(argument) if argument == "--help" || argument == "-h" => {
                    Ok(Action::Help(Some(command)))
                }
                Some(_) => Err(unexpected),
            }
        }
    }
}

fn write_help<W: Write>(output: &mut W, command: Option<&GeneratorCommand>) -> io::Result<()> {
    writeln!(output, "VersaKey {}", env!("CARGO_PKG_VERSION"))?;
    match command {
        Some(command) => {
            writeln!(output, "\nUsage: versakey {}", command.name)?;
            writeln!(output, "\nSuite: {}", command.suite.display_name())?;
        }
        None => {
            writeln!(output, "\nUsage: versakey <subcommand>")?;
            writeln!(output, "\nSubcommands:")?;
            for command in &COMMANDS {
                writeln!(
                    output,
                    "  {:<18} {}",
                    command.name,
                    command.suite.display_name()
                )?;
            }
            writeln!(output, "\n  -h, --help         Show help")?;
            writeln!(output, "  -V, --version      Show version")?;
            writeln!(
                output,
                "\nUse 'versakey <subcommand> --help' for command help."
            )?;
        }
    }
    writeln!(
        output,
        "\nPrompts for a byte count (1 to {MAX_KEY_BYTES}) and a hidden password twice."
    )?;
    writeln!(
        output,
        "Creates or replaces {OUTPUT_FILENAME} in the current directory."
    )?;
    writeln!(output, "Record the subcommand and size to reproduce a key.")
}

#[derive(Debug)]
enum CliError {
    Usage(&'static str),
    Io(io::Error),
    Size(crate::SizeError),
    SizeInputTooLong,
    EmptyPassword,
    PasswordMismatch,
    Generate(crate::GenerateError),
    CommittedButStatusWriteFailed(io::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => {
                write!(formatter, "{message}\nRun 'versakey --help' for usage.")
            }
            Self::Io(error) => write!(formatter, "console input/output failed: {error}"),
            Self::Size(error) => error.fmt(formatter),
            Self::SizeInputTooLong => write!(formatter, "key size input is too long"),
            Self::EmptyPassword => write!(formatter, "password cannot be empty"),
            Self::PasswordMismatch => write!(formatter, "passwords did not match"),
            Self::Generate(error) => error.fmt(formatter),
            Self::CommittedButStatusWriteFailed(error) => write!(
                formatter,
                "key.key was created, but reporting success to the console failed: {error}"
            ),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Size(error) => Some(error),
            Self::Generate(error) => Some(error),
            Self::CommittedButStatusWriteFailed(error) => Some(error),
            Self::Usage(_)
            | Self::SizeInputTooLong
            | Self::EmptyPassword
            | Self::PasswordMismatch => None,
        }
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<crate::SizeError> for CliError {
    fn from(error: crate::SizeError) -> Self {
        Self::Size(error)
    }
}

impl From<crate::GenerateError> for CliError {
    fn from(error: crate::GenerateError) -> Self {
        Self::Generate(error)
    }
}

/// Parse an explicit generator subcommand, then run the interactive CLI.
pub fn run_cli() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let stderr = io::stderr();
            let mut error_output = stderr.lock();
            let _ = writeln!(error_output, "Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let command = match parse_args(std::env::args_os().skip(1))? {
        Action::Help(command) => {
            write_help(&mut io::stdout().lock(), command)?;
            return Ok(());
        }
        Action::Version => {
            writeln!(
                io::stdout().lock(),
                "versakey {}",
                env!("CARGO_PKG_VERSION")
            )?;
            return Ok(());
        }
        Action::Generate(command) => command,
    };
    let current_directory = std::env::current_dir()?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    run_command_with(
        command,
        &current_directory,
        &mut input,
        &mut output,
        |prompt| rpassword::prompt_password(prompt),
    )
}

fn run_command_with<R, W, P>(
    command: &GeneratorCommand,
    directory: &Path,
    input: &mut R,
    output: &mut W,
    prompt_password: P,
) -> Result<(), CliError>
where
    R: BufRead,
    W: Write,
    P: FnMut(&str) -> io::Result<String>,
{
    writeln!(output, "VersaKey suite: {}", command.suite.display_name())?;
    run_with(input, output, prompt_password, |size, password| {
        generate_key_file_with_suite(directory, password, size, command.config(), command.suite)?;
        Ok(())
    })
}

fn run_with<R, W, P, G>(
    input: &mut R,
    output: &mut W,
    mut prompt_password: P,
    generate: G,
) -> Result<(), CliError>
where
    R: BufRead,
    W: Write,
    P: FnMut(&str) -> io::Result<String>,
    G: FnOnce(u64, Zeroizing<String>) -> Result<(), crate::GenerateError>,
{
    writeln!(
        output,
        "VersaKey deterministic key maker (1 to {MAX_KEY_BYTES} bytes)"
    )?;
    write!(output, "Key size in bytes: ")?;
    output.flush()?;

    let mut size_input = String::new();
    let bytes_read = input
        .take(MAX_SIZE_INPUT_BYTES + 1)
        .read_line(&mut size_input)?;
    if bytes_read == 0 {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "no key size was provided",
        )));
    }
    if bytes_read as u64 > MAX_SIZE_INPUT_BYTES {
        return Err(CliError::SizeInputTooLong);
    }
    let size = parse_size(&size_input)?;

    let password = Zeroizing::new(prompt_password("Password: ")?);
    let confirmation = Zeroizing::new(prompt_password("Confirm password: ")?);
    if password.is_empty() {
        return Err(CliError::EmptyPassword);
    }
    if password.as_bytes() != confirmation.as_bytes() {
        return Err(CliError::PasswordMismatch);
    }
    drop(confirmation);

    writeln!(output, "Generating {OUTPUT_FILENAME}...")?;
    output.flush()?;
    generate(size, password)?;
    writeln!(output, "Created {OUTPUT_FILENAME} ({size} bytes).")
        .map_err(CliError::CommittedButStatusWriteFailed)?;
    Ok(())
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "command_tests.rs"]
mod command_tests;
