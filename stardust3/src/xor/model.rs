//! Arguments, filename rules, and the inherited XOR command contract.
use std::ffi::{OsStr, OsString};

pub const VERSION: &str = concat!("stardust xor ", env!("CARGO_PKG_VERSION"));
pub const USAGE: &str = "\
stardust xor - XOR FILE_A with key.key into OUTPUT

USAGE:
    stardust xor [--] <FILE_A> <OUTPUT>
    stardust xor --in-place [--] <FILE_A>
    stardust xor --help
    stardust xor --version

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
    pub(super) a: OsString,
    // None explicitly selects in-place replacement; never infer it from aliases.
    pub(super) out: Option<OsString>,
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

pub(super) fn validate_name(name: &OsStr) -> Result<(), String> {
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
