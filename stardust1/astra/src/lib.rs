//! File XOR with a required key and transactional output replacement.

mod atomic_file;
mod file_safety;

use atomic_file::{overwrite_atomically, write_atomically};
use file_safety::{check_unchanged, file_identity, open_input};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

const KEY_NAME: &str = "key.key";
const BUF_SIZE: usize = 64 * 1024;
pub const VERSION: &str = concat!("astra-xor ", env!("CARGO_PKG_VERSION"));
pub const USAGE: &str = "\
astra-xor - XOR FILE_A with key.key into OUTPUT

USAGE:
    astra-xor [--] <FILE_A> <OUTPUT>
    astra-xor --in-place [--] <FILE_A>
    astra-xor --help
    astra-xor --version

Use filenames only. All files must be beside the executable, regardless of
the current working directory. key.key is required, even for empty FILE_A.
Before creating any output, key.key must be at least as long as FILE_A.
The result is exactly FILE_A's length; unused key bytes are ignored.
No repeating or truncating modes. File symlinks and special files are refused.
An existing output is replaced only after successful processing and file sync.
Use --in-place to atomically replace FILE_A at the same filename.
Otherwise output cannot be an input. The key and executable are always
protected, including hard links. In-place replacement needs temporary disk
space for the full result; other hard links keep the original bytes.

OPTIONS:
    --in-place      Replace FILE_A only after the complete result is synced
    -h, --help       Show this help
    -V, --version    Show the version
    --              Treat remaining arguments as filenames

XOR alone does not authenticate data or manage keys.";

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    a: OsString,
    // None explicitly selects in-place replacement; never infer it from aliases.
    out: Option<OsString>,
}

impl Args {
    pub fn output_name(&self) -> &OsStr {
        self.out.as_deref().unwrap_or(&self.a)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Help,
    Version,
    Run(Args),
}

pub fn parse_args(arguments: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut names = Vec::new();
    let mut options = true;
    let mut in_place = false;
    for arg in arguments {
        if options {
            match arg.to_str() {
                Some("--") => {
                    options = false;
                    continue;
                }
                Some("-h" | "--help") => return Ok(Action::Help),
                Some("-V" | "--version") => return Ok(Action::Version),
                Some("--in-place") => {
                    if in_place {
                        return Err("--in-place may only be supplied once".into());
                    }
                    in_place = true;
                    continue;
                }
                _ => {}
            }
            let text = arg.to_string_lossy();
            if text.starts_with('-') && text.len() > 1 {
                return Err(format!(
                    "unknown option: {text}; use -- before filenames starting with '-'"
                ));
            }
        }
        validate_name(&arg)?;
        names.push(arg);
    }
    if in_place {
        let [a]: [OsString; 1] = names.try_into().map_err(|names: Vec<OsString>| {
            format!(
                "--in-place expected 1 filename (FILE_A), got {}; do not supply OUTPUT",
                names.len()
            )
        })?;
        return Ok(Action::Run(Args { a, out: None }));
    }
    let [a, out]: [OsString; 2] = names.try_into().map_err(|names: Vec<OsString>| {
        format!(
            "expected 2 filenames (FILE_A OUTPUT), got {}; the key is always key.key",
            names.len()
        )
    })?;
    Ok(Action::Run(Args { a, out: Some(out) }))
}

fn validate_name(name: &OsStr) -> Result<(), String> {
    // Apply the same conservative rules on both systems. Paths retain their
    // original OS-native units; lossy text is only for validation/diagnostics.
    let text = name.to_string_lossy();
    if text.is_empty()
        || text.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        || text.ends_with([' ', '.'])
    {
        return Err(format!(
            "use a plain filename beside the executable: {text:?}; paths, Windows-special characters, and trailing spaces/dots are not allowed"
        ));
    }
    let stem = text
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if reserved || text.to_ascii_lowercase().starts_with(".astra-xor-") {
        return Err(format!("reserved filename: {text:?}"));
    }
    Ok(())
}

pub fn run(args: &Args) -> Result<u64, String> {
    let executable = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| format!("cannot locate the executable: {e}"))?;
    run_beside_executable(args, &executable)
}

fn run_beside_executable(args: &Args, executable: &Path) -> Result<u64, String> {
    validate_name(&args.a)?;
    validate_name(args.output_name())?;
    let directory = executable
        .parent()
        .ok_or("executable has no parent directory")?;
    let a_path = directory.join(&args.a);
    let key_path = directory.join(KEY_NAME);
    let output = directory.join(args.output_name());
    let mut a =
        open_input(&a_path).map_err(|e| format!("cannot open {}: {e}", a_path.display()))?;
    let mut key =
        open_input(&key_path).map_err(|e| format!("cannot open {}: {e}", key_path.display()))?;
    let a_before = a.metadata().map_err(|e| e.to_string())?;
    let key_before = key.metadata().map_err(|e| e.to_string())?;
    if key_before.len() < a_before.len() {
        return Err(format!(
            "key.key is too short ({} bytes); FILE_A requires at least {} bytes",
            key_before.len(),
            a_before.len()
        ));
    }
    // Keep the handles alive until publication so file IDs cannot be recycled.
    let program = File::open(executable).map_err(|e| format!("cannot open executable: {e}"))?;
    let mut protected = [file_identity(&key), file_identity(&program)]
        .into_iter()
        .collect::<io::Result<Vec<_>>>()
        .map_err(|e| format!("cannot identify protected files: {e}"))?;
    let mut transform = |a: &mut File, out: &mut File| {
        let bytes = xor_streams(a, &mut key, out, a_before.len())?;
        check_unchanged(&a_before, &a.metadata()?, "FILE_A")?;
        check_unchanged(&key_before, &key.metadata()?, KEY_NAME)?;
        Ok(bytes)
    };
    let result = if args.out.is_none() {
        // The transaction owns and closes this reader before Windows renames it.
        overwrite_atomically(&output, a, &protected, transform)
    } else {
        protected.push(file_identity(&a).map_err(|e| format!("cannot identify FILE_A: {e}"))?);
        write_atomically(&output, &protected, |out| transform(&mut a, out))
    };
    result.map_err(|e| format!("{}: {e}", output.display()))
}

fn xor_streams(
    a: &mut impl Read,
    key: &mut impl Read,
    out: &mut impl Write,
    length: u64,
) -> io::Result<u64> {
    let mut buf_a = vec![0; BUF_SIZE];
    let mut buf_key = vec![0; BUF_SIZE];
    let mut remaining = length;
    while remaining != 0 {
        let n = remaining.min(BUF_SIZE as u64) as usize;
        a.read_exact(&mut buf_a[..n])
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read FILE_A: {e}")))?;
        key.read_exact(&mut buf_key[..n])
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read key.key: {e}")))?;
        for (x, y) in buf_a[..n].iter_mut().zip(&buf_key[..n]) {
            *x ^= *y;
        }
        out.write_all(&buf_a[..n])?;
        remaining -= n as u64;
    }
    // Detect A growing; never read the key's unused tail.
    loop {
        match a.read(&mut [0]) {
            Ok(0) => return Ok(length),
            Ok(_) => return Err(io::Error::other("FILE_A grew while processing")),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
#[path = "tests/arguments.rs"]
mod arguments;
#[cfg(test)]
#[path = "tests/file_checks.rs"]
mod file_checks;
#[cfg(test)]
#[path = "tests/streams.rs"]
mod streams;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;
