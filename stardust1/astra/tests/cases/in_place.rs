use super::*;

#[test]
fn known_vector_and_status() {
    let app = App::fixture();
    let result = app.invoke(&["--in-place", "a.bin"]);
    assert_code(&result, 0);
    assert!(result.stderr.is_empty());
    assert!(String::from_utf8_lossy(&result.stdout).contains("wrote 3 bytes to a.bin"));
    assert_eq!(app.read("a.bin"), [240, 0, 255]);
    assert_eq!(app.read(KEY_NAME), [15, 15, 85]);
    app.dir.assert_clean();
}

#[test]
fn round_trips_at_empty_and_buffer_boundaries_with_longer_keys() {
    let app = App::new();
    for length in [
        0,
        1,
        BUF_SIZE - 1,
        BUF_SIZE,
        BUF_SIZE + 1,
        BUF_SIZE * 3 + 17,
    ] {
        let original = data(length, 0x2345);
        let key = data(length + 23, 0x9876);
        let expected: Vec<_> = original.iter().zip(&key).map(|(a, b)| a ^ b).collect();
        app.dir.write("a", &original);
        app.dir.write(KEY_NAME, &key);
        assert_code(&app.invoke(&["--in-place", "a"]), 0);
        assert_eq!(app.read("a"), expected);
        assert_code(&app.invoke(&["--in-place", "a"]), 0);
        assert_eq!(app.read("a"), original);
        assert_eq!(app.read(KEY_NAME), key);
        app.dir.assert_clean();
    }
    app.dir.write("a", []);
    app.dir.write(KEY_NAME, []);
    assert_code(&app.invoke(&["--in-place", "a"]), 0);
    assert!(app.read("a").is_empty());
}

#[test]
fn missing_short_or_empty_key_preserves_original() {
    let app = App::new();
    app.dir.write("a", b"original");
    for key in [None, Some(vec![]), Some(vec![1, 2])] {
        if let Some(bytes) = &key {
            app.dir.write(KEY_NAME, bytes);
        }
        let result = app.invoke(&["--in-place", "a"]);
        assert_code(&result, 1);
        assert!(result.stdout.is_empty());
        assert_eq!(app.read("a"), b"original");
        if let Some(bytes) = key {
            assert_eq!(app.read(KEY_NAME), bytes);
        }
        app.dir.assert_clean();
    }
    fs::remove_file(app.dir.path.join(KEY_NAME)).unwrap();
    app.dir.write("a", []);
    assert_code(&app.invoke(&["--in-place", "a"]), 1);
    assert!(app.read("a").is_empty());
    app.dir.assert_clean();
}

#[test]
fn missing_input_is_not_created() {
    let app = App::new();
    app.dir.write(KEY_NAME, [1]);
    assert_code(&app.invoke(&["--in-place", "missing"]), 1);
    assert!(!app.dir.path.join("missing").exists());
    app.dir.assert_clean();
}

#[test]
fn key_and_its_hard_links_are_never_overwritten() {
    let app = App::fixture();
    fs::hard_link(app.dir.path.join(KEY_NAME), app.dir.path.join("key-alias")).unwrap();
    for name in [KEY_NAME, "key-alias"] {
        assert_code(&app.invoke(&["--in-place", name]), 1);
    }
    assert_eq!(app.read("key-alias"), [15, 15, 85]);
    app.assert_inputs();
}

#[test]
fn executable_and_its_hard_links_are_never_overwritten() {
    let app = App::new();
    let original = fs::read(&app.executable).unwrap();
    let key = vec![0x55; original.len()];
    app.dir.write(KEY_NAME, &key);
    fs::hard_link(&app.executable, app.dir.path.join("app-alias")).unwrap();
    for name in [
        app.executable.file_name().unwrap(),
        std::ffi::OsStr::new("app-alias"),
    ] {
        assert_code(
            &app.command()
                .arg("--in-place")
                .arg(name)
                .test_output()
                .unwrap(),
            1,
        );
    }
    assert_eq!(fs::read(&app.executable).unwrap(), original);
    assert_eq!(app.read("app-alias"), original);
    assert_eq!(app.read(KEY_NAME), key);
    app.dir.assert_clean();
}

#[test]
fn other_hard_links_keep_original_bytes() {
    let app = App::fixture();
    fs::hard_link(app.dir.path.join("a.bin"), app.dir.path.join("alias")).unwrap();
    assert_code(&app.invoke(&["--in-place", "a.bin"]), 0);
    assert_eq!(app.read("a.bin"), [240, 0, 255]);
    assert_eq!(app.read("alias"), [255, 15, 170]);
    assert_eq!(app.read(KEY_NAME), [15, 15, 85]);
    app.dir.assert_clean();
}

#[test]
fn readonly_original_is_preserved_and_readonly_key_is_usable() {
    let app = App::fixture();
    let path = app.dir.path.join("a.bin");
    let original_mode = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original_mode.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = app.invoke(&["--in-place", "a.bin"]);
    fs::set_permissions(&path, original_mode).unwrap();
    assert_code(&result, 1);
    app.assert_inputs();

    let key = app.dir.path.join(KEY_NAME);
    let original_mode = fs::metadata(&key).unwrap().permissions();
    let mut readonly = original_mode.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&key, readonly).unwrap();
    let result = app.invoke(&["--in-place", "a.bin"]);
    fs::set_permissions(&key, original_mode).unwrap();
    assert_code(&result, 0);
    assert_eq!(app.read("a.bin"), [240, 0, 255]);
    assert_eq!(app.read(KEY_NAME), [15, 15, 85]);
    app.dir.assert_clean();
}

#[test]
fn executable_directory_controls_in_place_target() {
    let app = App::fixture();
    let elsewhere = TestDir::new();
    elsewhere.write("a.bin", b"decoy");
    elsewhere.write(KEY_NAME, b"decoy key");
    assert_code(
        &app.command()
            .current_dir(&elsewhere.path)
            .args(["--in-place", "a.bin"])
            .test_output()
            .unwrap(),
        0,
    );
    assert_eq!(app.read("a.bin"), [240, 0, 255]);
    assert_eq!(fs::read(elsewhere.path.join("a.bin")).unwrap(), b"decoy");
    assert_eq!(
        fs::read(elsewhere.path.join(KEY_NAME)).unwrap(),
        b"decoy key"
    );
    app.dir.assert_clean();
    elsewhere.assert_clean();
}

#[test]
fn spaces_unicode_dashes_and_native_names_work_in_place() {
    let app = App::fixture();
    for name in [
        "input 鍵.bin".into(),
        "--in-place".into(),
        "-".into(),
        native_name(),
    ] {
        app.dir.write(&name, [255, 15, 170]);
        assert_code(
            &app.command()
                .args(["--in-place", "--"])
                .arg(&name)
                .test_output()
                .unwrap(),
            0,
        );
        assert_eq!(app.read(&name), [240, 0, 255]);
    }
    assert_eq!(app.read(KEY_NAME), [15, 15, 85]);
    app.dir.assert_clean();
}

#[test]
fn invalid_in_place_arguments_do_not_modify_files() {
    let app = App::fixture();
    for args in [
        vec!["--in-place"],
        vec!["--in-place", "a.bin", "out"],
        vec!["--in-place", "--in-place", "a.bin"],
        vec!["--in-place", "../a.bin"],
        vec!["--in-place", "key.key:stream"],
    ] {
        assert_code(&app.invoke(&args), 2);
    }
    assert!(!app.dir.path.join("out").exists());
    app.assert_inputs();
}

#[test]
fn directory_input_is_preserved() {
    let app = App::fixture();
    fs::create_dir(app.dir.path.join("folder")).unwrap();
    app.dir.write("folder/keep", b"original");
    assert_code(&app.invoke(&["--in-place", "folder"]), 1);
    assert_eq!(app.read("folder/keep"), b"original");
    app.assert_inputs();
}

#[cfg(windows)]
#[test]
fn windows_locked_input_rename_failure_preserves_original() {
    use std::os::windows::fs::OpenOptionsExt;
    let app = App::fixture();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(app.dir.path.join("a.bin"))
        .unwrap();
    let result = app.invoke(&["--in-place", "a.bin"]);
    drop(locked);
    assert_code(&result, 1);
    app.assert_inputs();
}

#[cfg(windows)]
#[test]
fn windows_active_writer_is_refused() {
    let app = App::fixture();
    for name in ["a.bin", KEY_NAME] {
        let writer = OpenOptions::new()
            .write(true)
            .open(app.dir.path.join(name))
            .unwrap();
        let result = app.invoke(&["--in-place", "a.bin"]);
        drop(writer);
        assert_code(&result, 1);
        app.assert_inputs();
    }
}

#[cfg(windows)]
#[test]
fn windows_case_aliases_preserve_key_protection() {
    let app = App::fixture();
    assert_code(&app.invoke(&["--in-place", "KEY.KEY"]), 1);
    app.assert_inputs();
    assert_code(&app.invoke(&["--in-place", "A.BIN"]), 0);
    assert_eq!(app.read("a.bin"), [240, 0, 255]);
    app.dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn in_place_symlinks_and_dangling_links_are_refused() {
    use std::os::unix::fs::symlink;
    let app = App::fixture();
    for target in ["a.bin", KEY_NAME, "missing"] {
        let link = app.dir.path.join("link");
        symlink(target, &link).unwrap();
        assert_code(&app.invoke(&["--in-place", "link"]), 1);
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(link).unwrap();
        app.assert_inputs();
    }
    assert!(!app.dir.path.join("missing").exists());
}
