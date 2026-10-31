use super::*;
use std::cell::Cell;
use std::io::Read;

fn assert_only(directory: &Path, names: &[&str]) {
    let mut actual: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    actual.sort();
    let mut expected: Vec<_> = names.iter().map(std::ffi::OsString::from).collect();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn file_sync_failure_preserves_existing_key_and_cleans_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::write(&path, b"original").unwrap();
    let error = write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            file.write_all(b"replacement")?;
            Ok(())
        },
        |_| Err(io::ErrorKind::StorageFull.into()),
        |_| panic!("failed file sync must prevent publication"),
    )
    .unwrap_err();
    assert!(matches!(error, GenerateError::Io(ref e) if e.kind() == io::ErrorKind::StorageFull));
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn file_sync_failure_does_not_publish_a_new_key() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    let error = write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            file.write_all(b"replacement")?;
            Ok(())
        },
        |_| Err(io::Error::other("injected sync failure")),
        |_| panic!("unpublished file"),
    )
    .unwrap_err();
    assert!(matches!(error, GenerateError::Io(_)));
    assert_only(directory.path(), &[]);
}

#[test]
fn directory_sync_failure_retains_the_committed_key() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::write(&path, b"original").unwrap();
    let error = write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            file.write_all(b"replacement")?;
            Ok(())
        },
        fs::File::sync_all,
        |parent| {
            assert_eq!(parent, directory.path());
            assert_eq!(fs::read(&path)?, b"replacement");
            Err(io::ErrorKind::PermissionDenied.into())
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GenerateError::CommittedButDirectorySyncFailed(_)
    ));
    assert!(error.to_string().contains("key.key was replaced"));
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn producer_failure_skips_both_sync_stages() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    let error = write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            file.write_all(b"partial")?;
            Err(io::Error::from(io::ErrorKind::WriteZero).into())
        },
        |_| panic!("failed production must not sync"),
        |_| panic!("failed production must not publish"),
    )
    .unwrap_err();
    assert!(matches!(error, GenerateError::Io(ref e) if e.kind() == io::ErrorKind::WriteZero));
    assert_only(directory.path(), &[]);
}

#[test]
fn complete_data_is_synced_before_replacement_and_directory_after() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::write(&path, b"original").unwrap();
    let step = Cell::new(0);
    write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            assert_eq!(step.replace(1), 0);
            file.write_all(b"complete")?;
            assert_eq!(fs::read(&path)?, b"original");
            Ok(())
        },
        |file| {
            assert_eq!(step.replace(2), 1);
            assert_eq!(file.metadata()?.len(), 8);
            assert_eq!(fs::read(&path)?, b"original");
            file.sync_all()
        },
        |parent| {
            assert_eq!(step.replace(3), 2);
            assert_eq!(fs::read(&path)?, b"complete");
            sync_directory(parent)
        },
    )
    .unwrap();
    assert_eq!(step.get(), 3);
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn persist_failure_skips_directory_sync_and_preserves_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), b"original").unwrap();
    let error = write_atomically_with_sync(
        directory.path(),
        &path,
        |file| {
            file.write_all(b"new")?;
            Ok(())
        },
        fs::File::sync_all,
        |_| panic!("persist failed"),
    )
    .unwrap_err();
    assert!(matches!(error, GenerateError::Io(_)));
    assert_eq!(fs::read(path.join("keep")).unwrap(), b"original");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn file_sync_panic_preserves_old_key_and_cleans_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::write(&path, b"original").unwrap();
    let result = std::panic::catch_unwind(|| {
        let _ = write_atomically_with_sync(
            directory.path(),
            &path,
            |file| {
                file.write_all(b"new")?;
                Ok(())
            },
            |_| panic!("injected sync panic"),
            |_| Ok(()),
        );
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn directory_sync_panic_does_not_remove_committed_key() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    let result = std::panic::catch_unwind(|| {
        let _ = write_atomically_with_sync(
            directory.path(),
            &path,
            |file| {
                file.write_all(b"complete")?;
                Ok(())
            },
            fs::File::sync_all,
            |_| panic!("injected directory sync panic"),
        );
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), b"complete");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn missing_directory_fails_before_producer_runs() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    assert!(
        write_atomically(&missing, &missing.join("key.key"), |_| panic!(
            "no output directory"
        ))
        .is_err()
    );
    assert_only(directory.path(), &[]);
}

#[test]
fn file_as_parent_fails_without_truncating_it() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("parent");
    fs::write(&parent, b"keep").unwrap();
    assert!(
        write_atomically(&parent, &parent.join("key.key"), |_| panic!(
            "parent is not a directory"
        ))
        .is_err()
    );
    assert_eq!(fs::read(parent).unwrap(), b"keep");
}

#[test]
fn stale_temporary_files_survive_success_and_failure() {
    let directory = tempfile::tempdir().unwrap();
    let stale = directory.path().join(".key.key.another-process");
    fs::write(&stale, b"owned by someone else").unwrap();
    let path = directory.path().join("key.key");
    write_atomically(directory.path(), &path, |file| {
        file.write_all(b"committed")?;
        Ok(())
    })
    .unwrap();
    assert!(
        write_atomically(directory.path(), &path, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("fail").into())
        })
        .is_err()
    );
    assert_eq!(fs::read(&stale).unwrap(), b"owned by someone else");
    assert_eq!(fs::read(&path).unwrap(), b"committed");
    assert_only(directory.path(), &["key.key", ".key.key.another-process"]);
}

#[test]
fn nested_failure_cannot_remove_another_active_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    write_atomically(directory.path(), &path, |outer| {
        outer.write_all(b"outer")?;
        let first: Vec<_> = fs::read_dir(directory.path())?
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(first.len(), 1);
        assert!(
            write_atomically(directory.path(), &path, |inner| {
                inner.write_all(b"inner")?;
                assert_eq!(fs::read_dir(directory.path())?.count(), 2);
                Err(io::Error::other("inner failure").into())
            })
            .is_err()
        );
        assert!(first[0].exists());
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        outer.write_all(b" complete")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"outer complete");
    assert_only(directory.path(), &["key.key"]);
}

#[test]
fn key_replacement_preserves_hard_links_and_platform_reader_policy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    let alias = directory.path().join("old-key");
    fs::write(&path, b"original").unwrap();
    fs::hard_link(&path, &alias).unwrap();
    let mut reader = fs::File::open(&path).unwrap();
    let result = write_atomically(directory.path(), &path, |file| {
        file.write_all(b"new")?;
        Ok(())
    });
    #[cfg(unix)]
    result.unwrap();
    // VersaKey's persist policy can refuse replacement while a reader is open
    // on Windows. Preserve that policy and verify safe failure and retry.
    #[cfg(windows)]
    {
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_only(directory.path(), &["key.key", "old-key"]);
    }
    let mut old = Vec::new();
    reader.read_to_end(&mut old).unwrap();
    assert_eq!(old, b"original");
    drop(reader);
    #[cfg(windows)]
    write_atomically(directory.path(), &path, |file| {
        file.write_all(b"new")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(alias).unwrap(), b"original");
    assert_eq!(fs::read(path).unwrap(), b"new");
    assert_only(directory.path(), &["key.key", "old-key"]);
}

#[test]
fn unicode_directory_supports_atomic_key_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("keys with spaces \u{937} \u{1f511}");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("key.key");
    fs::write(&path, b"old").unwrap();
    write_atomically(&parent, &path, |file| {
        file.write_all(b"new")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(path).unwrap(), b"new");
    assert_only(&parent, &["key.key"]);
}

#[cfg(unix)]
#[test]
fn key_symlink_is_replaced_without_modifying_its_target() {
    let directory = tempfile::tempdir().unwrap();
    let referent = directory.path().join("keep");
    let path = directory.path().join("key.key");
    fs::write(&referent, b"original").unwrap();
    std::os::unix::fs::symlink(&referent, &path).unwrap();
    write_atomically(directory.path(), &path, |file| {
        file.write_all(b"new")?;
        Ok(())
    })
    .unwrap();
    assert!(
        !fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert_eq!(fs::read(referent).unwrap(), b"original");
}

#[cfg(windows)]
#[test]
fn windows_delete_lock_preserves_existing_key_and_cleans_temporary() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key.key");
    fs::write(&path, b"original").unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    assert!(
        write_atomically(directory.path(), &path, |file| {
            file.write_all(b"new")?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_only(directory.path(), &["key.key"]);
    drop(lock);
}
