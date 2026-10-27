use astra_xor::{Action, USAGE, VERSION, parse_args, run};
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(Action::Help) => {
            let _ = writeln!(io::stdout().lock(), "{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Action::Version) => {
            let _ = writeln!(io::stdout().lock(), "{VERSION}");
            ExitCode::SUCCESS
        }
        Ok(Action::Run(args)) => match run(&args) {
            Ok(n) => {
                // A closed status pipe must not turn a saved file into a panic.
                let _ = writeln!(
                    io::stdout().lock(),
                    "wrote {n} bytes to {} (beside the executable)",
                    args.output_name().to_string_lossy()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                let _ = writeln!(io::stderr().lock(), "astra-xor: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "astra-xor: {error}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
