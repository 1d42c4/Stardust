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
    write_atomically_with_sync(
        directory,
        output_path,
        write,
        fs::File::sync_all,
        sync_directory,
    )
}

// Keep the production ordering and replacement policy in one path. These two
// callbacks let tests inject failures immediately before and after publication.
fn write_atomically_with_sync(
    directory: &Path,
    output_path: &Path,
    write: impl FnOnce(&mut fs::File) -> Result<(), GenerateError>,
    sync_file: impl FnOnce(&fs::File) -> io::Result<()>,
    sync_parent: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), GenerateError> {
    let mut temporary = Builder::new().prefix(".key.key.").tempfile_in(directory)?;

    write(temporary.as_file_mut())?;
    temporary.as_file_mut().flush()?;
    sync_file(temporary.as_file())?;

    temporary
        .persist(output_path)
        .map_err(|error| GenerateError::Io(error.error))?;
    sync_parent(directory).map_err(GenerateError::CommittedButDirectorySyncFailed)?;
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

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
