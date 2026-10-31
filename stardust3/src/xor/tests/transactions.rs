use super::*;
use crate::xor::test_support::TestDir;
#[cfg(windows)]
use crate::xor::validate_name;

fn fixture() -> (TestDir, [Identity; 2]) {
    let dir = TestDir::new();
    let a = dir.write("a", [1, 2, 3]);
    let b = dir.write("b", [4, 5, 6]);
    let ids = [
        file_identity(&File::open(a).unwrap()).unwrap(),
        file_identity(&File::open(b).unwrap()).unwrap(),
    ];
    (dir, ids)
}

#[test]
fn new_output_appears_only_after_success() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    let n = write_atomically(&out, &ids, |file| {
        file.write_all(b"finished")?;
        assert!(!out.exists());
        Ok(8)
    })
    .unwrap();
    assert_eq!(n, 8);
    assert_eq!(fs::read(out).unwrap(), b"finished");
    dir.assert_clean();
}

#[test]
fn existing_output_is_unchanged_until_success() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    write_atomically(&out, &ids, |file| {
        file.write_all(b"new")?;
        assert_eq!(fs::read(&out)?, b"original");
        Ok(3)
    })
    .unwrap();
    assert_eq!(fs::read(out).unwrap(), b"new");
    dir.assert_clean();
}

#[test]
fn failure_after_partial_writes_preserves_existing_output() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let error = write_atomically(&out, &ids, |file| {
        file.write_all(&vec![8; 131079])?;
        Err(io::Error::other("simulated disk failure"))
    })
    .unwrap_err();
    assert!(error.to_string().contains("disk failure"));
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn failure_does_not_create_new_output() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("read failed"))
        })
        .is_err()
    );
    assert!(!out.exists());
    dir.assert_clean();
}

#[test]
fn directory_sync_failure_reports_that_output_was_published() {
    let (dir, _) = fixture();
    let out = dir.write("out", b"old");
    let mut temp = Temporary::create(&out).unwrap();
    let file = temp.file.as_mut().unwrap();
    file.write_all(b"finished").unwrap();
    file.sync_all().unwrap();
    let error = temp
        .publish(&out, || {
            assert_eq!(fs::read(&out)?, b"finished");
            Err(io::Error::other("injected directory sync failure"))
        })
        .unwrap_err();
    assert!(error.to_string().contains("output was published"));
    assert!(
        error
            .to_string()
            .contains("injected directory sync failure")
    );
    drop(temp);
    assert_eq!(fs::read(out).unwrap(), b"finished");
    dir.assert_clean();
}

#[test]
fn failed_rename_does_not_sync_directory_or_remove_destination() {
    let (dir, _) = fixture();
    let out = dir.path.join("out");
    fs::create_dir(&out).unwrap();
    dir.write("out/keep", b"original");
    let mut temp = Temporary::create(&out).unwrap();
    temp.file.as_mut().unwrap().write_all(b"new").unwrap();
    assert!(
        temp.publish(&out, || panic!("cannot sync an unpublished output"))
            .is_err()
    );
    drop(temp);
    assert_eq!(fs::read(out.join("keep")).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn panic_unwinds_without_deleting_existing_output() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let result = std::panic::catch_unwind(|| {
        let _ = write_atomically(&out, &ids, |file| {
            file.write_all(b"partial")?;
            panic!("injected producer panic");
        });
    });
    assert!(result.is_err());
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn refuses_new_destination_created_during_processing() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    let error = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        fs::write(&out, b"other writer")?;
        Ok(4)
    })
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(out).unwrap(), b"other writer");
    dir.assert_clean();
}

#[test]
fn refuses_destination_replaced_during_processing() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let backup = dir.path.join("backup");
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"ours")?;
            fs::rename(&out, &backup)?;
            fs::write(&out, b"other writer")?;
            Ok(4)
        })
        .is_err()
    );
    assert_eq!(fs::read(out).unwrap(), b"other writer");
    assert_eq!(fs::read(backup).unwrap(), b"original");
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn refuses_destination_edited_in_place_during_processing() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let identity = file_identity(&File::open(&out).unwrap()).unwrap();
    let error = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        fs::write(&out, b"another application's edit")?;
        assert_eq!(file_identity(&File::open(&out)?)?, identity);
        Ok(4)
    })
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(out).unwrap(), b"another application's edit");
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn refuses_same_length_destination_edit_during_processing() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"old");
    let error = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        fs::write(&out, b"new")?;
        // Force an observable timestamp difference even on coarse filesystems.
        File::options().write(true).open(&out)?.set_times(
            fs::FileTimes::new().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1000)),
        )?;
        Ok(4)
    })
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(out).unwrap(), b"new");
    dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn refuses_destination_permission_changes_during_processing() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, ids) = fixture();
    let out = dir.write("out", b"old");
    fs::set_permissions(&out, Permissions::from_mode(0o640)).unwrap();
    let error = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        fs::set_permissions(&out, Permissions::from_mode(0o600))?;
        Ok(4)
    })
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(&out).unwrap(), b"old");
    assert_eq!(
        fs::metadata(out).unwrap().permissions().mode() & 0o777,
        0o600
    );
    dir.assert_clean();
}

#[test]
fn refuses_destination_removed_during_processing() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"ours")?;
            fs::remove_file(&out)?;
            Ok(4)
        })
        .is_err()
    );
    assert!(!out.exists());
    dir.assert_clean();
}

#[test]
fn rechecks_hard_link_identity_before_publish() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"ours")?;
            fs::hard_link(dir.path.join("a"), &out)?;
            Ok(4)
        })
        .is_err()
    );
    assert_eq!(fs::read(&out).unwrap(), [1, 2, 3]);
    assert_eq!(fs::read(dir.path.join("a")).unwrap(), [1, 2, 3]);
    dir.assert_clean();
}

#[test]
fn directory_appearing_before_publish_is_preserved() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"ours")?;
            fs::create_dir(&out)?;
            Ok(4)
        })
        .is_err()
    );
    assert!(out.is_dir());
    dir.assert_clean();
}

#[test]
fn read_only_output_is_not_replaced() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let original = fs::metadata(&out).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&out, readonly).unwrap();
    let result = write_atomically(&out, &ids, |_| panic!("must fail before writing"));
    fs::set_permissions(&out, original).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn output_becoming_read_only_fails_without_data_loss() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let original = fs::metadata(&out).unwrap().permissions();
    let result = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&out, readonly)?;
        Ok(4)
    });
    fs::set_permissions(&out, original).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn independent_temporary_files_have_unique_names() {
    let (dir, _) = fixture();
    let out = dir.path.join("out");
    let a = Temporary::create(&out).unwrap();
    let b = Temporary::create(&out).unwrap();
    assert_ne!(a.path, b.path);
    assert!(a.path.exists());
    assert!(b.path.exists());
    drop(a);
    drop(b);
    dir.assert_clean();
}

#[test]
fn unrelated_hard_link_to_old_output_is_not_modified() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"old");
    let alias = dir.path.join("old-alias");
    fs::hard_link(&out, &alias).unwrap();
    write_atomically(&out, &ids, |file| {
        file.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert_eq!(fs::read(out).unwrap(), b"new");
    assert_eq!(fs::read(alias).unwrap(), b"old");
    dir.assert_clean();
}

#[test]
fn missing_parent_is_not_created() {
    let (dir, ids) = fixture();
    let out = dir.path.join("missing/out");
    assert!(write_atomically(&out, &ids, |_| panic!("must fail before writing")).is_err());
    assert!(!dir.path.join("missing").exists());
    dir.assert_clean();
}

#[test]
fn file_sync_failure_preserves_existing_output() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let error = write_atomically_checked(
        &out,
        &ids,
        None,
        |file| {
            file.write_all(b"complete replacement")?;
            Ok(20)
        },
        |_| Err(io::ErrorKind::StorageFull.into()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn file_sync_failure_keeps_new_output_absent() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    let error = write_atomically_checked(
        &out,
        &ids,
        None,
        |file| {
            file.write_all(b"complete")?;
            Ok(8)
        },
        |_| Err(io::ErrorKind::StorageFull.into()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    assert!(!out.exists());
    dir.assert_clean();
}

#[test]
fn producer_failure_never_reaches_file_sync() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let error = write_atomically_checked(
        &out,
        &ids,
        None,
        |file| {
            file.write_all(b"partial")?;
            Err(io::ErrorKind::UnexpectedEof.into())
        },
        |_| panic!("must not sync incomplete output"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn destination_appearing_during_file_sync_is_not_overwritten() {
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    let error = write_atomically_checked(
        &out,
        &ids,
        None,
        |file| {
            file.write_all(b"ours")?;
            Ok(4)
        },
        |file| {
            file.sync_all()?;
            fs::write(&out, b"other writer")
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("output changed"));
    assert_eq!(fs::read(out).unwrap(), b"other writer");
    dir.assert_clean();
}

#[test]
fn protected_hard_link_appearing_during_sync_is_not_overwritten() {
    let dir = TestDir::new();
    let key = dir.write("key.key", b"protected");
    let handle = File::open(&key).unwrap();
    let identity = file_identity(&handle).unwrap();
    let out = dir.path.join("out");
    let error = write_atomically_checked(
        &out,
        &[identity],
        None,
        |file| {
            file.write_all(b"ours")?;
            Ok(4)
        },
        |file| {
            file.sync_all()?;
            fs::hard_link(&key, &out)
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("protected"));
    assert_eq!(fs::read(out).unwrap(), b"protected");
    assert_eq!(fs::read(key).unwrap(), b"protected");
    dir.assert_clean();
}

#[test]
fn file_sync_panic_cleans_temporary_without_replacing_output() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    let result = std::panic::catch_unwind(|| {
        let _ = write_atomically_checked(
            &out,
            &ids,
            None,
            |file| {
                file.write_all(b"complete")?;
                Ok(8)
            },
            |_| panic!("injected sync panic"),
        );
    });
    assert!(result.is_err());
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn stale_temporary_is_preserved_across_success_and_failure() {
    let (dir, ids) = fixture();
    let stale = dir.write(".astra-xor-other-process.tmp", b"still owned");
    let out = dir.write("out", b"original");
    write_atomically(&out, &ids, |file| {
        file.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert!(
        write_atomically(&out, &ids, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected failure"))
        })
        .is_err()
    );
    assert_eq!(fs::read(&stale).unwrap(), b"still owned");
    assert_eq!(fs::read(out).unwrap(), b"new");
    // Delete only this test's sentinel before checking for transaction leaks.
    fs::remove_file(stale).unwrap();
    dir.assert_clean();
}

#[test]
fn identity_distinguishes_files_and_recognizes_hard_links() {
    let (dir, ids) = fixture();
    assert_ne!(ids[0], ids[1]);
    let alias = dir.path.join("alias");
    fs::hard_link(dir.path.join("a"), &alias).unwrap();
    assert_eq!(file_identity(&File::open(alias).unwrap()).unwrap(), ids[0]);
}

#[cfg(windows)]
#[test]
fn windows_active_output_writer_is_refused_before_production() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    // Default Rust writers share deletion, so rename alone does not protect
    // their edits. Their last-write time may also remain stale until close.
    let writer = OpenOptions::new().write(true).open(&out).unwrap();
    let result = write_atomically(&out, &ids, |_| {
        panic!("an active output writer must be refused before producing bytes")
    });
    drop(writer);
    assert_eq!(result.unwrap_err().raw_os_error(), Some(32));
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[cfg(windows)]
#[test]
fn windows_output_denies_writers_until_publication() {
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    write_atomically_checked(
        &out,
        &ids,
        None,
        |file| {
            assert_eq!(
                OpenOptions::new()
                    .write(true)
                    .open(&out)
                    .unwrap_err()
                    .raw_os_error(),
                Some(32)
            );
            assert_eq!(fs::read(&out)?, b"original");
            file.write_all(b"new")?;
            Ok(3)
        },
        |file| {
            assert_eq!(
                OpenOptions::new()
                    .write(true)
                    .open(&out)
                    .unwrap_err()
                    .raw_os_error(),
                Some(32)
            );
            file.sync_all()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&out).unwrap(), b"new");
    // The guard must not leak after the transaction.
    fs::write(out, b"next writer").unwrap();
    dir.assert_clean();
}

#[cfg(windows)]
#[test]
fn failed_windows_rename_preserves_output_and_cleans_temp() {
    use std::os::windows::fs::OpenOptionsExt;
    let (dir, ids) = fixture();
    let out = dir.write("out", b"original");
    // Permit reading/writing but deny deletion/renaming for this open handle.
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&out)
        .unwrap();
    let result = write_atomically(&out, &ids, |file| {
        file.write_all(b"ours")?;
        Ok(4)
    });
    drop(locked);
    assert!(result.is_err());
    assert_eq!(fs::read(out).unwrap(), b"original");
    dir.assert_clean();
}

#[cfg(windows)]
#[test]
fn windows_device_and_stream_names_are_refused() {
    for name in [
        "NUL",
        "nul.txt",
        "CON",
        "COM1",
        "LPT3.log",
        "COM¹",
        "file:stream",
        "out.",
        "out ",
        "a?b",
    ] {
        assert!(validate_name(std::ffi::OsStr::new(name)).is_err(), "{name}");
    }
    for name in ["output.bin", "COM10", "null", "鍵.bin"] {
        validate_name(std::ffi::OsStr::new(name)).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn new_unix_output_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, ids) = fixture();
    let out = dir.path.join("out");
    write_atomically(&out, &ids, |file| {
        file.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert_eq!(
        fs::metadata(out).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn unix_access_bits_are_preserved_without_special_bits() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, ids) = fixture();
    let out = dir.write("out", b"old");
    fs::set_permissions(&out, Permissions::from_mode(0o6750)).unwrap();
    write_atomically(&out, &ids, |file| {
        file.write_all(b"new")?;
        Ok(3)
    })
    .unwrap();
    assert_eq!(
        fs::metadata(out).unwrap().permissions().mode() & 0o7777,
        0o750
    );
}
