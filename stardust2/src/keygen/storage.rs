//! Key-file transaction; this intentionally retains VersaKey replacement semantics.
use super::model::GenerateError;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};
use tempfile::Builder;

pub(super) fn write_atomically<F>(
    directory: &Path,
    output_path: &Path,
    write: F,
) -> Result<(), GenerateError>
where
    F: FnOnce(&mut fs::File) -> Result<(), GenerateError>,
{
    let mut temporary = Builder::new().prefix(".key.key.").tempfile_in(directory)?;

    write(temporary.as_file_mut())?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;

    temporary
        .persist(output_path)
        .map_err(|error| GenerateError::Io(error.error))?;
    sync_directory(directory).map_err(GenerateError::CommittedButDirectorySyncFailed)?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> io::Result<()> {
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> io::Result<()> {
    Ok(())
}
