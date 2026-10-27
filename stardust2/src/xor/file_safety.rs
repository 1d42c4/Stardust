//! Regular-file validation, change detection, and native file identities.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::path::Path;

pub(crate) fn is_plain_file(metadata: &Metadata) -> bool {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reject all reparse points, including links and cloud placeholders.
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

pub(crate) fn open_input(path: &Path) -> io::Result<File> {
    // Check without following links before opening a possible FIFO or device.
    if !is_plain_file(&fs::symlink_metadata(path)?) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input must be a regular file, not a link, directory, or device",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ: refuse concurrent writers/deletion.
    }
    let file = options.open(path)?;
    if !is_plain_file(&file.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input is not a regular file",
        ));
    }
    Ok(file)
}

pub(crate) fn check_unchanged(before: &Metadata, after: &Metadata, name: &str) -> io::Result<()> {
    let changed = before.len() != after.len()
        || before.permissions() != after.permissions()
        || before.modified()? != after.modified()?;
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        // ctime also detects edits whose writer restores the previous mtime.
        changed || (before.ctime(), before.ctime_nsec()) != (after.ctime(), after.ctime_nsec())
    };
    if changed {
        return Err(io::Error::other(format!("{name} changed while processing")));
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity(u64, u64);

#[cfg(unix)]
pub(crate) fn file_identity(file: &File) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(Identity(metadata.dev(), metadata.ino()))
}

// Rust 1.88 does not expose Windows file IDs. Retain full 128-bit IDs for ReFS
// as well as NTFS; comparing paths alone cannot recognize hard links.
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    volume: u64,
    file: [u8; 16],
}

#[cfg(windows)]
pub(crate) fn file_identity(file: &File) -> io::Result<Identity> {
    use std::ffi::c_void;
    use std::mem::{MaybeUninit, size_of};
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandleEx(
            handle: *mut c_void,
            class: i32,
            info: *mut c_void,
            size: u32,
        ) -> i32;
    }
    let mut info = MaybeUninit::<Identity>::uninit();
    // SAFETY: File owns a live handle. Class 18 is FileIdInfo; Identity matches
    // FILE_ID_INFO's C layout and size. The writable buffer outlives this call.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            18,
            info.as_mut_ptr().cast(),
            size_of::<Identity>() as u32,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: the successful call initialized the complete structure.
        Ok(unsafe { info.assume_init() })
    }
}

#[cfg(not(any(unix, windows)))]
compile_error!("astra-xor supports Windows and Unix platforms");
