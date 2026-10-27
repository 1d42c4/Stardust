//! Same-directory temporary files, verification, sync, and atomic publication.

use crate::xor::file_safety::{Identity, check_unchanged, file_identity, is_plain_file};
#[cfg(unix)]
use std::fs::Permissions;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Existing {
    _file: File,
    identity: Identity,
    metadata: Metadata,
}

fn inspect_output(path: &Path, protected: &[Identity]) -> io::Result<Option<Existing>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !is_plain_file(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output must be a regular file, not a link, directory, or device",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Deny writers for the entire transaction. Windows may defer updating
        // their last-write time until close, so metadata alone can miss edits.
        // Share deletion so existing readers and atomic rename still work.
        options.share_mode(1 | 4); // FILE_SHARE_READ | FILE_SHARE_DELETE
    }
    let file = options.open(path)?;
    let identity = file_identity(&file)?;
    if protected.contains(&identity) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output aliases a protected input or the executable (including hard links)",
        ));
    }
    let metadata = file.metadata()?;
    if !is_plain_file(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output is not a regular file",
        ));
    }
    if metadata.permissions().readonly() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "output is read-only",
        ));
    }
    Ok(Some(Existing {
        _file: file,
        identity,
        metadata,
    }))
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
struct Temporary {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl Temporary {
    fn create(target: &Path) -> io::Result<Self> {
        let parent = target.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "output has no parent directory",
            )
        })?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..128 {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".astra-xor-{}-{stamp}-{sequence}.tmp",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(0);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "cannot create a unique temporary output",
        ))
    }

    fn publish(
        &mut self,
        target: &Path,
        sync_directory: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        drop(self.file.take());
        // Both paths share a filesystem. Never delete the old output first.
        // std also handles Windows' FileRenameInfoEx fallback for open readers;
        // MoveFileExW alone fails even when those readers share deletion.
        // https://doc.rust-lang.org/std/fs/fn.rename.html
        fs::rename(&self.path, target)?;
        // Publication cannot be undone if the following directory sync fails.
        self.committed = true;
        sync_directory().map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("output was published, but syncing its directory failed; durability is uncertain: {e}"),
            )
        })
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.file.take()); // Windows needs the handle closed before cleanup.
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn write_atomically(
    target: &Path,
    protected: &[Identity],
    produce: impl FnOnce(&mut File) -> io::Result<u64>,
) -> io::Result<u64> {
    write_atomically_checked(target, protected, None, produce, sync_all)
}

/// Replace the source's directory entry only after its entire transformation
/// succeeds. The source handle is consumed so Windows can rename over it.
/// Other hard links and already-open readers continue to see the original file.
///
/// The caller must keep the containing directory stable and keep handles for
/// protected identities alive. Change detection is not a filesystem snapshot
/// or a compare-and-swap against hostile concurrent directory manipulation.
pub(crate) fn overwrite_atomically(
    target: &Path,
    source: File,
    protected: &[Identity],
    transform: impl FnOnce(&mut File, &mut File) -> io::Result<u64>,
) -> io::Result<u64> {
    overwrite_with_sync(target, source, protected, transform, sync_all)
}

// A narrow sync seam lets tests exercise a late disk failure through the exact
// overwrite path, after transformation but before the only publication point.
fn overwrite_with_sync(
    target: &Path,
    mut source: File,
    protected: &[Identity],
    transform: impl FnOnce(&mut File, &mut File) -> io::Result<u64>,
    sync_file: impl FnOnce(&File) -> io::Result<()>,
) -> io::Result<u64> {
    let identity = file_identity(&source)?;
    let before = source.metadata()?;
    write_atomically_checked(
        target,
        protected,
        Some((identity, &before)),
        |out| {
            let bytes = transform(&mut source, out)?;
            check_unchanged(&before, &source.metadata()?, "FILE_A")?;
            // This closure owns source, including on error/panic. The inspection
            // handle in `initial` keeps its identity alive and continues denying
            // Windows writers after this reader closes.
            drop(source);
            Ok(bytes)
        },
        sync_file,
    )
}

fn sync_all(file: &File) -> io::Result<()> {
    loop {
        match file.sync_all() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

fn write_atomically_checked(
    target: &Path,
    protected: &[Identity],
    expected_source: Option<(Identity, &Metadata)>,
    produce: impl FnOnce(&mut File) -> io::Result<u64>,
    sync_file: impl FnOnce(&File) -> io::Result<()>,
) -> io::Result<u64> {
    let initial = inspect_output(target, protected)?;
    if let Some((identity, metadata)) = expected_source {
        let existing = initial
            .as_ref()
            .filter(|file| file.identity == identity)
            .ok_or_else(|| {
                io::Error::other("FILE_A path changed before processing; refusing to replace it")
            })?;
        check_unchanged(metadata, &existing.metadata, "FILE_A")?;
    }
    #[cfg(target_os = "linux")]
    let directory = {
        // Probe support before writing so unsupported directory sync fails before
        // replacing output. Keep the same directory handle through publication.
        let parent = target.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "output has no parent directory",
            )
        })?;
        let directory = File::open(parent)?;
        sync_all(&directory)?;
        directory
    };
    let mut temp = Temporary::create(target)?;
    let file = temp.file.as_mut().expect("temporary file is open");
    let bytes = produce(file)?;
    if let Some(existing) = &initial {
        let permissions = existing.metadata.permissions();
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Permissions::from_mode(permissions.mode() & 0o777)
        };
        file.set_permissions(permissions)?;
    }
    file.flush()?;
    sync_file(file)?;
    drop(temp.file.take());
    let current = inspect_output(target, protected)?;
    if initial.as_ref().map(|x| x.identity) != current.as_ref().map(|x| x.identity) {
        return Err(io::Error::other(
            "output changed while processing; refusing to replace it",
        ));
    }
    if let (Some(before), Some(after)) = (&initial, &current) {
        check_unchanged(&before.metadata, &after.metadata, "output")?;
    }
    // These readers share deletion on Windows. Keep their writer protection
    // through the rename, including the gap after the final metadata check.
    temp.publish(target, || {
        #[cfg(target_os = "linux")]
        sync_all(&directory)?;
        Ok(())
    })?;
    drop(current);
    drop(initial);
    Ok(bytes)
}

#[cfg(test)]
#[path = "tests/transactions.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/in_place.rs"]
mod in_place_tests;
