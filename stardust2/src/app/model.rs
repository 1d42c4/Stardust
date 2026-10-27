use crate::{keygen, xor};
use std::{error::Error, ffi::OsString, fmt, io};

pub enum Action {
    Help,
    Version,
    KeyHelp(&'static keygen::commands::GeneratorCommand),
    Generate(&'static keygen::commands::GeneratorCommand),
    Xor(xor::Action),
}

/// Preserve native filenames. Only the first argument selects a domain; it is
/// never guessed from a file's extension, existence, or the working directory.
pub fn parse_args(arguments: impl IntoIterator<Item = OsString>) -> Result<Action, AppError> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    if arguments.first().is_some_and(|arg| arg == "xor") {
        return xor::parse_args(arguments.into_iter().skip(1))
            .map(Action::Xor)
            .map_err(AppError::XorUsage);
    }
    if arguments.len() == 2 && arguments[0] == "help" && arguments[1] == "xor" {
        return Ok(Action::Xor(xor::Action::Help));
    }
    match keygen::cli::parse_args(&arguments).map_err(AppError::Keygen)? {
        keygen::cli::Action::Help(None) => Ok(Action::Help),
        keygen::cli::Action::Help(Some(command)) => Ok(Action::KeyHelp(command)),
        keygen::cli::Action::Version => Ok(Action::Version),
        keygen::cli::Action::Generate(command) => Ok(Action::Generate(command)),
    }
}

#[derive(Debug)]
pub enum AppError {
    Keygen(keygen::cli::CliError),
    XorUsage(String),
    Xor(String),
    Io(io::Error),
}

impl AppError {
    /// Astra retains exit 2 for invalid XOR arguments, 1 for processing errors.
    /// VersaKey retains exit 1 for all generator errors.
    pub fn exit_code(&self) -> u8 {
        if matches!(self, Self::XorUsage(_)) {
            2
        } else {
            1
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keygen(error) => write!(f, "Error: {error}"),
            Self::XorUsage(error) => write!(f, "stardust xor: {error}\n\n{}", xor::USAGE),
            Self::Xor(error) => write!(f, "stardust xor: {error}"),
            Self::Io(error) => write!(f, "stardust: {error}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Keygen(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::XorUsage(_) | Self::Xor(_) => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
