use super::*;
use crate::xor::file_safety::{check_unchanged, is_plain_file, open_input};
use crate::xor::test_support::TestDir;
use std::fs::OpenOptions;
#[cfg(unix)]
use std::path::Path;
use std::time::UNIX_EPOCH;

#[test]
fn stable_metadata_passes() {
    let dir = TestDir::new();
    let path = dir.write("a", [1, 2]);
    check_unchanged(
        &fs::metadata(&path).unwrap(),
        &fs::metadata(&path).unwrap(),
        "a",
    )
    .unwrap();
}

#[test]
fn length_changes_are_detected_in_both_directions() {
    let dir = TestDir::new();
    for next in [vec![], vec![3], vec![3, 4, 5]] {
        let path = dir.write("a", [1, 2]);
        let before = fs::metadata(&path).unwrap();
        fs::write(&path, next).unwrap();
        assert!(check_unchanged(&before, &fs::metadata(path).unwrap(), "a").is_err());
    }
}

#[test]
fn same_length_timestamp_changes_are_detected() {
    let dir = TestDir::new();
    let path = dir.write("a", [1, 2]);
    let before = fs::metadata(&path).unwrap();
    let file = OpenOptions::new().write(true).open(&path).unwrap();
    file.set_times(
        std::fs::FileTimes::new().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1000)),
    )
    .unwrap();
    assert!(check_unchanged(&before, &file.metadata().unwrap(), "a").is_err());
}

#[test]
fn plain_file_check_rejects_directories() {
    let dir = TestDir::new();
    assert!(!is_plain_file(&fs::symlink_metadata(&dir.path).unwrap()));
    assert!(open_input(&dir.path).is_err());
}

#[test]
fn direct_api_cannot_bypass_name_validation() {
    let dir = TestDir::new();
    let executable = dir.write("app", b"program");
    for (a, out) in [("../a", "out"), ("a", "../out"), ("a", "CON")] {
        let error = run_beside_executable(
            &Args {
                a: a.into(),
                out: Some(out.into()),
            },
            &executable,
        )
        .unwrap_err();
        assert!(error.contains("filename"));
    }
    assert_eq!(fs::read(executable).unwrap(), b"program");
    dir.assert_clean();
}

#[test]
fn short_key_fails_before_opening_executable_or_creating_temporary() {
    let dir = TestDir::new();
    dir.write("a", [1, 2]);
    dir.write(KEY_NAME, [3]);
    let error = run_beside_executable(
        &Args {
            a: "a".into(),
            out: Some("out".into()),
        },
        &dir.path.join("missing-executable"),
    )
    .unwrap_err();
    assert!(error.contains("key.key is too short"));
    assert!(!dir.path.join("out").exists());
    dir.assert_clean();
}

#[test]
fn streaming_failure_after_full_block_preserves_destination() {
    let dir = TestDir::new();
    let path = dir.write("out", b"original");
    let a = vec![1; BUF_SIZE + 1];
    let key = vec![2; BUF_SIZE];
    let error = write_atomically(&path, &[], |out| {
        xor_streams(&mut a.as_slice(), &mut key.as_slice(), out, a.len() as u64)
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(fs::read(path).unwrap(), b"original");
    dir.assert_clean();
}

#[test]
fn growth_detected_after_writes_does_not_publish_new_output() {
    let dir = TestDir::new();
    let path = dir.path.join("out");
    assert!(
        write_atomically(&path, &[], |out| xor_streams(
            &mut &[1, 2, 3][..],
            &mut &[4, 5][..],
            out,
            2
        ))
        .is_err()
    );
    assert!(!path.exists());
    dir.assert_clean();
}

#[cfg(windows)]
#[test]
fn windows_input_handle_denies_writes_and_removal_until_closed() {
    let dir = TestDir::new();
    let path = dir.write("a", [1, 2]);
    let file = open_input(&path).unwrap();
    assert!(OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(open_input(&path).is_ok());
    drop(file);
    fs::write(path, [3]).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_device_is_rejected_before_open() {
    assert!(open_input(Path::new("/dev/null")).is_err());
}
