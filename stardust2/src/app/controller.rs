use super::{Action, AppError, VERSION, parse_args, view};
use crate::{keygen, xor};
use std::{
    ffi::OsString,
    fs,
    io::{self, BufRead, Write},
    path::Path,
    process::ExitCode,
};

/// Explicit location policy, also used in integration tests without changing
/// process-global current_dir. Supply the resolved executable path for XOR.
pub struct Environment<'a> {
    pub working_directory: &'a Path,
    pub executable: &'a Path,
}

/// The same parse/dispatch/prompt/transaction path as the executable, with a
/// terminal password reader injected. Passwords are never CLI arguments.
pub fn run_with<R: BufRead, W: Write, P: FnMut(&str) -> io::Result<String>>(
    arguments: impl IntoIterator<Item = OsString>,
    environment: &Environment<'_>,
    input: &mut R,
    output: &mut W,
    password_reader: P,
) -> Result<(), AppError> {
    execute(
        parse_args(arguments)?,
        environment,
        input,
        output,
        password_reader,
    )
}

pub fn execute<R: BufRead, W: Write, P: FnMut(&str) -> io::Result<String>>(
    action: Action,
    environment: &Environment<'_>,
    input: &mut R,
    output: &mut W,
    password_reader: P,
) -> Result<(), AppError> {
    match action {
        Action::Help => view::write_help(output).map_err(AppError::Io),
        Action::Version => writeln!(output, "{VERSION}").map_err(AppError::Io),
        Action::KeyHelp(command) => {
            keygen::cli::write_help(output, Some(command)).map_err(AppError::Io)
        }
        Action::Generate(command) => keygen::cli::run_command_with(
            command,
            environment.working_directory,
            input,
            output,
            password_reader,
        )
        .map_err(AppError::Keygen),
        // Preserve Astra's best-effort status writes, including after commit.
        Action::Xor(xor::Action::Help) => {
            let _ = writeln!(output, "{}", xor::USAGE);
            Ok(())
        }
        Action::Xor(xor::Action::Version) => {
            let _ = writeln!(output, "{}", xor::VERSION);
            Ok(())
        }
        Action::Xor(xor::Action::Run(args)) => {
            let bytes =
                xor::run_beside_executable(&args, environment.executable).map_err(AppError::Xor)?;
            let _ = writeln!(
                output,
                "wrote {bytes} bytes to {} (beside the executable)",
                args.output_name().to_string_lossy()
            );
            Ok(())
        }
    }
}

fn run_process() -> Result<(), AppError> {
    // Invalid commands and help must not depend on file/terminal availability.
    let action = parse_args(std::env::args_os().skip(1))?;
    let working_directory = if matches!(action, Action::Generate(_)) {
        std::env::current_dir()?
    } else {
        Default::default()
    };
    let executable = if matches!(action, Action::Xor(xor::Action::Run(_))) {
        std::env::current_exe()
            .and_then(fs::canonicalize)
            .map_err(|error| AppError::Xor(format!("cannot locate the executable: {error}")))?
    } else {
        Default::default()
    };
    execute(
        action,
        &Environment {
            working_directory: &working_directory,
            executable: &executable,
        },
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        |prompt| rpassword::prompt_password(prompt),
    )
}

pub fn run_cli() -> ExitCode {
    match run_process() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
