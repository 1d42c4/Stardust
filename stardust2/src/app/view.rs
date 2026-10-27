use crate::keygen;
use std::io::{self, Write};

pub(super) fn write_help(output: &mut impl Write) -> io::Result<()> {
    keygen::cli::write_help(output, None)?;
    writeln!(output, "\nFile processing (Astra):")?;
    writeln!(output, "  stardust xor [--] <FILE_A> <OUTPUT>")?;
    writeln!(output, "  stardust xor --in-place [--] <FILE_A>")?;
    writeln!(
        output,
        "\nXOR uses key.key and filenames beside the executable."
    )?;
    writeln!(
        output,
        "Key generation writes key.key in the current directory."
    )?;
    writeln!(
        output,
        "Run the generator from the executable's directory to use its key for XOR."
    )?;
    writeln!(
        output,
        "Use 'stardust xor --help' for file safety and location rules."
    )
}
