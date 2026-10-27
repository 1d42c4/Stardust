use super::*;
use crate::file_safety::open_input;
use crate::test_support::{TestDir, data};
use std::io::Read;

#[test]
fn original_survives_partial_work_and_existing_readers_keep_old_bytes() {
    let dir = TestDir::new();
    let original = data(131_079, 123);
    let path = dir.write("a", &original);
    let mut reader = File::open(&path).unwrap();
    let old_id = file_identity(&reader).unwrap();
    let source = open_input(&path).unwrap();
    let bytes = overwrite_atomically(&path, source, &[], |input, out| {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        assert_eq!(bytes, original);
        out.write_all(b"new")?;
        assert_eq!(fs::read(&path)?, original);
        Ok(3)
    })
    .unwrap();
    assert_eq!(bytes, 3);
    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert_ne!(file_identity(&File::open(&path).unwrap()).unwrap(), old_id);
    let mut old_bytes = Vec::new();
    reader.read_to_end(&mut old_bytes).unwrap();
    assert_eq!(old_bytes, original);
    dir.assert_clean();
}

#[test]
fn short_read_after_a_full_block_preserves_source_and_releases_handle() {
    let dir = TestDir::new();
    let original = data(crate::BUF_SIZE + 1, 42);
    let path = dir.write("a", &original);
    let key = vec![0xFF; crate::BUF_SIZE];
    let error = overwrite_atomically(&path, open_input(&path).unwrap(), &[], |input, out| {
        crate::xor_streams(input, &mut key.as_slice(), out, original.len() as u64)
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(fs::read(&path).unwrap(), original);
    dir.assert_clean();
    // In particular, an error must release Windows' non-delete-sharing reader.
    fs::rename(&path, dir.path.join("moved")).unwrap();
}

#[test]
fn write_failure_after_partial_progress_preserves_source() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    let error = overwrite_atomically(&path, open_input(&path).unwrap(), &[], |_, out| {
        out.write_all(&vec![0xAB; 131_079])?;
        Err(io::Error::other("injected disk full"))
    })
    .unwrap_err();
    assert!(error.to_string().contains("injected disk full"));
    assert_eq!(fs::read(&path).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn file_sync_failure_after_complete_transformation_preserves_source() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    let old_id = file_identity(&File::open(&path).unwrap()).unwrap();
    let error = overwrite_with_sync(
        &path,
        open_input(&path).unwrap(),
        &[],
        |_, out| {
            out.write_all(b"complete replacement")?;
            Ok(20)
        },
        |file| {
            assert_eq!(file.metadata()?.len(), 20);
            assert_eq!(fs::read(&path)?, b"original");
            Err(io::Error::other("injected file sync failure"))
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected file sync failure"));
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(file_identity(&File::open(&path).unwrap()).unwrap(), old_id);
    dir.assert_clean();
}

#[test]
fn panic_during_transformation_preserves_source_and_releases_handle() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    let result = std::panic::catch_unwind(|| {
        let _ = overwrite_atomically(&path, open_input(&path).unwrap(), &[], |_, out| {
            out.write_all(b"partial")?;
            panic!("injected transformation panic");
        });
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original");
    dir.assert_clean();
    fs::rename(&path, dir.path.join("moved")).unwrap();
}

#[test]
fn missing_or_replaced_source_path_fails_before_transformation() {
    let dir = TestDir::new();
    for replace in [false, true] {
        let path = dir.write("a", b"original");
        let source = File::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        if replace {
            fs::write(&path, b"someone else's file").unwrap();
        }
        let error = overwrite_atomically(&path, source, &[], |_, _| {
            panic!("a stale input must not be transformed")
        })
        .unwrap_err();
        assert!(error.to_string().contains("FILE_A path changed"));
        if replace {
            assert_eq!(fs::read(&path).unwrap(), b"someone else's file");
        } else {
            assert!(!path.exists());
        }
        dir.assert_clean();
    }
}

#[test]
fn source_replaced_during_transformation_is_not_overwritten() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    let backup = dir.path.join("backup");
    let result = overwrite_atomically(&path, File::open(&path).unwrap(), &[], |_, out| {
        out.write_all(b"ours")?;
        fs::rename(&path, &backup)?;
        fs::write(&path, b"someone else's file")?;
        Ok(4)
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), b"someone else's file");
    assert_eq!(fs::read(backup).unwrap(), b"original");
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn same_length_source_edit_during_transformation_is_detected() {
    let dir = TestDir::new();
    let path = dir.write("a", b"old");
    let error = overwrite_atomically(&path, File::open(&path).unwrap(), &[], |_, out| {
        out.write_all(b"ours")?;
        fs::write(&path, b"new")?;
        File::options().write(true).open(&path)?.set_times(
            fs::FileTimes::new().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1000)),
        )?;
        Ok(4)
    })
    .unwrap_err();
    assert!(error.to_string().contains("FILE_A changed"));
    assert_eq!(fs::read(&path).unwrap(), b"new");
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn source_changed_after_reading_but_before_publication_is_detected() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    let error = overwrite_with_sync(
        &path,
        open_input(&path).unwrap(),
        &[],
        |_, out| {
            out.write_all(b"ours")?;
            Ok(4)
        },
        |file| {
            file.sync_all()?;
            // The source reader has closed, but the final metadata check must
            // still catch edits made before publication on Unix.
            fs::write(&path, b"someone else's edit")
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(&path).unwrap(), b"someone else's edit");
    dir.assert_clean();
}

#[cfg(windows)]
#[test]
fn source_stays_write_protected_after_transformation() {
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    overwrite_with_sync(
        &path,
        open_input(&path).unwrap(),
        &[],
        |_, out| {
            out.write_all(b"new")?;
            Ok(3)
        },
        |file| {
            // The input reader has closed to allow rename. The inspection
            // handle must continue protecting its contents through file sync.
            assert_eq!(
                OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap_err()
                    .raw_os_error(),
                Some(32)
            );
            assert_eq!(fs::read(&path)?, b"original");
            file.sync_all()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new");
    fs::write(path, b"next writer").unwrap();
    dir.assert_clean();
}

#[test]
fn protected_source_is_refused_before_transformation() {
    let dir = TestDir::new();
    let path = dir.write("key", b"protected");
    let source = open_input(&path).unwrap();
    let id = file_identity(&source).unwrap();
    assert!(
        overwrite_atomically(&path, source, &[id], |_, _| {
            panic!("protected files must never be transformed")
        })
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), b"protected");
    dir.assert_clean();
}

#[test]
fn long_parent_paths_support_atomic_overwrite() {
    let dir = TestDir::new();
    let nested = dir
        .path
        .join("a".repeat(100))
        .join("b".repeat(100))
        .join("c".repeat(100));
    fs::create_dir_all(&nested).unwrap();
    let path = nested.join("file 鍵");
    fs::write(&path, b"old").unwrap();
    overwrite_atomically(&path, open_input(&path).unwrap(), &[], |_, out| {
        out.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert_eq!(fs::read(path).unwrap(), b"new");
    assert_eq!(fs::read_dir(nested).unwrap().count(), 1);
}

#[test]
fn concurrent_readers_only_observe_complete_versions() {
    let dir = TestDir::new();
    let old = vec![0x11; 128 * 1024];
    let new = vec![0xEE; old.len()];
    let path = dir.write("a", &old);
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            start.wait();
            for _ in 0..256 {
                let bytes = fs::read(&path).expect("destination must never disappear");
                assert!(bytes == old || bytes == new, "reader saw incomplete output");
            }
        });
        start.wait();
        overwrite_atomically(&path, open_input(&path).unwrap(), &[], |_, out| {
            for chunk in new.chunks(4096) {
                out.write_all(chunk)?;
            }
            Ok(new.len() as u64)
        })
        .unwrap();
        reader.join().unwrap();
    });
    assert_eq!(fs::read(path).unwrap(), new);
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn in_place_preserves_access_bits_and_clears_special_bits() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TestDir::new();
    let path = dir.write("a", b"original");
    fs::set_permissions(&path, Permissions::from_mode(0o6750)).unwrap();
    overwrite_atomically(&path, open_input(&path).unwrap(), &[], |_, out| {
        out.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o750
    );
    assert_eq!(fs::read(path).unwrap(), b"new");
    dir.assert_clean();
}
