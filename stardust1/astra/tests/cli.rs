#[path = "cases/in_place.rs"]
mod in_place;
mod support;
use std::fs;
#[cfg(windows)]
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

const KEY_NAME: &str = "key.key";
const BUF_SIZE: usize = 64 * 1024;

// A child forked by another test can temporarily inherit a writable handle
// to a fixture binary, even with close-on-exec. Linux then refuses to execute
// that binary with ETXTBSY. Serialize copying with spawning, but release the
// lock before waiting so the application processes can still run in parallel.
static EXECUTABLE_SETUP: Mutex<()> = Mutex::new(());

pub fn copy_executable(source: &Path, destination: &Path) -> io::Result<u64> {
    let _guard = EXECUTABLE_SETUP.lock().unwrap();
    fs::copy(source, destination)
}

pub trait TestCommandExt {
    fn test_output(&mut self) -> io::Result<Output>;
}

impl TestCommandExt for Command {
    fn test_output(&mut self) -> io::Result<Output> {
        let child = {
            let _guard = EXECUTABLE_SETUP.lock().unwrap();
            self.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        };
        child.wait_with_output()
    }
}

use std::process::{Command, Output};
use support::{TestDir, data, native_name};

struct App {
    dir: TestDir,
    executable: PathBuf,
}
impl App {
    fn new() -> Self {
        let dir = TestDir::new();
        let executable = dir
            .path
            .join(format!("astra-xor{}", std::env::consts::EXE_SUFFIX));
        let binary = PathBuf::from(env!("CARGO_BIN_EXE_astra-xor"));
        copy_executable(&binary, &executable).unwrap();
        Self { dir, executable }
    }
    fn fixture() -> Self {
        let app = Self::new();
        app.dir.write("a.bin", [255, 15, 170]);
        app.dir.write(KEY_NAME, [15, 15, 85]);
        app
    }
    fn invoke(&self, args: &[&str]) -> Output {
        self.command().args(args).test_output().unwrap()
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.current_dir(&self.dir.path);
        command
    }
    fn read(&self, name: impl AsRef<Path>) -> Vec<u8> {
        fs::read(self.dir.path.join(name)).unwrap()
    }
    fn assert_inputs(&self) {
        assert_eq!(self.read("a.bin"), [255, 15, 170]);
        assert_eq!(self.read(KEY_NAME), [15, 15, 85]);
        self.dir.assert_clean();
    }
}
fn assert_code(result: &Output, code: i32) {
    assert_eq!(
        result.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn known_vector_and_status() {
    let app = App::fixture();
    let result = app.invoke(&["a.bin", "out.bin"]);
    assert_code(&result, 0);
    assert!(result.stderr.is_empty());
    assert!(String::from_utf8_lossy(&result.stdout).contains("wrote 3 bytes"));
    assert_eq!(app.read("out.bin"), [240, 0, 255]);
    app.assert_inputs();
}

#[test]
fn longer_key_produces_exactly_a_length() {
    let app = App::fixture();
    app.dir.write(KEY_NAME, [15, 15, 85, 99, 22]);
    assert_code(&app.invoke(&["a.bin", "out"]), 0);
    assert_eq!(app.read("out"), [240, 0, 255]);
    assert_eq!(app.read(KEY_NAME), [15, 15, 85, 99, 22]);
    app.dir.assert_clean();
}

#[test]
fn short_key_preserves_existing_output() {
    let app = App::fixture();
    app.dir.write(KEY_NAME, [1, 2]);
    app.dir.write("out", b"important");
    let result = app.invoke(&["a.bin", "out"]);
    assert_code(&result, 1);
    assert!(result.stdout.is_empty());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("key.key is too short (2 bytes)"));
    assert!(error.contains("requires at least 3 bytes"));
    assert_eq!(app.read("out"), b"important");
    app.dir.assert_clean();
}

#[test]
fn empty_key_does_not_create_output() {
    let app = App::fixture();
    app.dir.write(KEY_NAME, []);
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}

#[test]
fn length_check_precedes_destination_inspection() {
    let app = App::fixture();
    app.dir.write(KEY_NAME, [1]);
    fs::create_dir(app.dir.path.join("out")).unwrap();
    let result = app.invoke(&["a.bin", "out"]);
    assert_code(&result, 1);
    assert!(String::from_utf8_lossy(&result.stderr).contains("key.key is too short"));
    assert!(app.dir.path.join("out").is_dir());
    app.dir.assert_clean();
}

#[test]
fn missing_key_preserves_output_even_with_another_b_file() {
    let app = App::new();
    app.dir.write("a.bin", [1, 2]);
    app.dir.write("b.bin", [4, 5]);
    app.dir.write("out", b"old");
    let result = app.invoke(&["a.bin", "out"]);
    assert_code(&result, 1);
    assert!(String::from_utf8_lossy(&result.stderr).contains("key.key"));
    assert_eq!(app.read("out"), b"old");
    app.dir.assert_clean();
}

#[test]
fn missing_key_is_required_for_empty_a_too() {
    let app = App::new();
    app.dir.write("a.bin", []);
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}

#[test]
fn empty_a_accepts_empty_or_longer_key() {
    let app = App::new();
    app.dir.write("a.bin", []);
    for key in [vec![], vec![1, 2, 3]] {
        app.dir.write(KEY_NAME, key);
        app.dir.write("out", b"old");
        assert_code(&app.invoke(&["a.bin", "out"]), 0);
        assert!(app.read("out").is_empty());
        app.dir.assert_clean();
    }
}

#[test]
fn missing_a_preserves_output() {
    let app = App::fixture();
    app.dir.write("out", b"old");
    assert_code(&app.invoke(&["missing", "out"]), 1);
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[test]
fn launch_directory_is_ignored_for_both_inputs_and_output() {
    let app = App::fixture();
    let elsewhere = TestDir::new();
    elsewhere.write("a.bin", [0, 0, 0]);
    elsewhere.write(KEY_NAME, [0, 0, 0]);
    elsewhere.write("out", b"keep");
    let result = app
        .command()
        .current_dir(&elsewhere.path)
        .args(["a.bin", "out"])
        .test_output()
        .unwrap();
    assert_code(&result, 0);
    assert_eq!(app.read("out"), [240, 0, 255]);
    assert_eq!(fs::read(elsewhere.path.join("out")).unwrap(), b"keep");
    assert_eq!(fs::read(elsewhere.path.join(KEY_NAME)).unwrap(), [0, 0, 0]);
    app.assert_inputs();
    elsewhere.assert_clean();
}

#[test]
fn never_falls_back_to_key_in_working_directory() {
    let app = App::new();
    app.dir.write("a.bin", [1]);
    let elsewhere = TestDir::new();
    elsewhere.write(KEY_NAME, [4]);
    let result = app
        .command()
        .current_dir(&elsewhere.path)
        .args(["a.bin", "out"])
        .test_output()
        .unwrap();
    assert_code(&result, 1);
    assert!(!app.dir.path.join("out").exists());
    assert!(!elsewhere.path.join("out").exists());
    app.dir.assert_clean();
}

#[test]
fn never_falls_back_to_a_in_working_directory() {
    let app = App::new();
    app.dir.write(KEY_NAME, [4]);
    let elsewhere = TestDir::new();
    elsewhere.write("a.bin", [1]);
    let result = app
        .command()
        .current_dir(&elsewhere.path)
        .args(["a.bin", "out"])
        .test_output()
        .unwrap();
    assert_code(&result, 1);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}

#[test]
fn rejects_absolute_paths_even_to_files_beside_executable() {
    let app = App::fixture();
    for args in [
        vec![app.dir.path.join("a.bin").into_os_string(), "out".into()],
        vec!["a.bin".into(), app.dir.path.join("out").into_os_string()],
    ] {
        assert_code(&app.command().args(args).test_output().unwrap(), 2);
    }
    app.assert_inputs();
}

#[test]
fn rejects_parent_paths_subdirectories_and_both_separator_styles() {
    let app = App::fixture();
    for name in [
        "./a.bin",
        ".\\a.bin",
        "../a.bin",
        "..\\a.bin",
        "sub/a.bin",
        "sub\\a.bin",
    ] {
        assert_code(&app.invoke(&[name, "out"]), 2);
        assert_code(&app.invoke(&["a.bin", name]), 2);
    }
    app.assert_inputs();
}

#[test]
fn all_old_processing_flags_and_three_file_syntax_fail() {
    let app = App::fixture();
    app.dir.write("out", b"old");
    for flag in ["-t", "--truncate", "-r", "--repeat"] {
        assert_code(&app.invoke(&[flag, "a.bin", "out"]), 2);
    }
    assert_code(&app.invoke(&["a.bin", KEY_NAME, "out"]), 2);
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[test]
fn rejects_bad_counts_unknown_options_and_empty_names() {
    let app = App::fixture();
    for args in [
        vec![],
        vec!["a.bin"],
        vec!["--unknown", "a.bin", "out"],
        vec!["", "out"],
        vec!["a.bin", ""],
    ] {
        assert_code(&app.invoke(&args), 2);
    }
    app.assert_inputs();
}

#[test]
fn device_stream_and_ambiguous_names_fail_on_every_platform() {
    let app = App::fixture();
    for name in [
        "NUL",
        "nul.txt",
        "CON",
        "COM1",
        "LPT²",
        "out:stream",
        "out.",
        "out ",
        "a?b",
    ] {
        assert_code(&app.invoke(&[name, "out"]), 2);
        assert_code(&app.invoke(&["a.bin", name]), 2);
    }
    app.assert_inputs();
}

#[test]
fn help_and_version_work_without_input_files() {
    let app = App::new();
    for flag in ["-h", "--help", "-V", "--version"] {
        let result = app.invoke(&[flag]);
        assert_code(&result, 0);
        assert!(!result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
    assert_eq!(fs::read_dir(&app.dir.path).unwrap().count(), 1);
}

#[test]
fn replacement_removes_old_trailing_bytes() {
    let app = App::fixture();
    app.dir.write("out", vec![99; 10000]);
    assert_code(&app.invoke(&["a.bin", "out"]), 0);
    assert_eq!(app.read("out"), [240, 0, 255]);
    app.assert_inputs();
}

#[test]
fn round_trip_across_multiple_buffers() {
    let app = App::new();
    let input = data(BUF_SIZE * 3 + 19, 31);
    let key = data(input.len() + 99, 92);
    app.dir.write("a.bin", &input);
    app.dir.write(KEY_NAME, &key);
    assert_code(&app.invoke(&["a.bin", "masked"]), 0);
    assert_eq!(
        app.read("masked"),
        input
            .iter()
            .zip(&key)
            .map(|(x, y)| x ^ y)
            .collect::<Vec<_>>()
    );
    assert_code(&app.invoke(&["masked", "restored"]), 0);
    assert_eq!(app.read("restored"), input);
    assert_eq!(app.read(KEY_NAME), key);
    app.dir.assert_clean();
}

#[test]
fn output_cannot_be_either_input() {
    let app = App::fixture();
    for out in ["a.bin", KEY_NAME] {
        assert_code(&app.invoke(&["a.bin", out]), 1);
    }
    app.assert_inputs();
}

#[test]
fn output_cannot_be_a_hard_link_to_either_input() {
    let app = App::fixture();
    for target in ["a.bin", KEY_NAME] {
        let alias = app.dir.path.join("alias");
        fs::hard_link(app.dir.path.join(target), &alias).unwrap();
        let result = app.invoke(&["a.bin", "alias"]);
        assert_code(&result, 1);
        assert!(String::from_utf8_lossy(&result.stderr).contains("including hard links"));
        fs::remove_file(alias).unwrap();
    }
    app.assert_inputs();
}

#[test]
fn output_cannot_replace_executable_or_its_hard_link() {
    let app = App::fixture();
    let program = fs::read(&app.executable).unwrap();
    let result = app
        .command()
        .arg("a.bin")
        .arg(app.executable.file_name().unwrap())
        .test_output()
        .unwrap();
    assert_code(&result, 1);
    fs::hard_link(&app.executable, app.dir.path.join("alias")).unwrap();
    assert_code(&app.invoke(&["a.bin", "alias"]), 1);
    assert_eq!(fs::read(&app.executable).unwrap(), program);
    assert_eq!(app.read("alias"), program);
    app.assert_inputs();
}

#[test]
fn same_file_can_supply_a_and_key() {
    let app = App::fixture();
    assert_code(&app.invoke(&[KEY_NAME, "out"]), 0);
    assert_eq!(app.read("out"), [0, 0, 0]);
    app.assert_inputs();
}

#[test]
fn directory_a_is_refused() {
    let app = App::fixture();
    fs::create_dir(app.dir.path.join("folder")).unwrap();
    app.dir.write("out", b"old");
    assert_code(&app.invoke(&["folder", "out"]), 1);
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[test]
fn directory_key_is_refused() {
    let app = App::new();
    app.dir.write("a.bin", []);
    fs::create_dir(app.dir.path.join(KEY_NAME)).unwrap();
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}

#[test]
fn directory_output_and_its_contents_are_preserved() {
    let app = App::fixture();
    fs::create_dir(app.dir.path.join("out")).unwrap();
    app.dir.write("out/keep", b"keep");
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert_eq!(app.read("out/keep"), b"keep");
    app.assert_inputs();
}

#[test]
fn readonly_output_is_preserved() {
    let app = App::fixture();
    let path = app.dir.write("out", b"old");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = app.invoke(&["a.bin", "out"]);
    fs::set_permissions(path, original).unwrap();
    assert_code(&result, 1);
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[test]
fn readonly_inputs_can_be_processed() {
    let app = App::fixture();
    for name in ["a.bin", KEY_NAME] {
        let path = app.dir.path.join(name);
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = app.invoke(&["a.bin", "out"]);
        fs::set_permissions(path, original).unwrap();
        assert_code(&result, 0);
    }
    assert_eq!(app.read("out"), [240, 0, 255]);
    app.assert_inputs();
}

#[test]
fn spaces_unicode_and_option_like_filenames() {
    let app = App::fixture();
    for (a, out) in [
        ("input 鍵.bin", "résultat file.bin"),
        ("--help", "--"),
        ("-", "-output"),
    ] {
        app.dir.write(a, [255, 15, 170]);
        assert_code(&app.invoke(&["--", a, out]), 0);
        assert_eq!(app.read(out), [240, 0, 255]);
    }
    app.dir.assert_clean();
}

#[test]
fn native_non_unicode_filenames_end_to_end() {
    let app = App::fixture();
    let a = native_name();
    let mut out = a.clone();
    out.push("-out");
    app.dir.write(&a, [255, 15, 170]);
    assert_code(&app.command().arg(&a).arg(&out).test_output().unwrap(), 0);
    assert_eq!(app.read(&out), [240, 0, 255]);
    app.dir.assert_clean();
}

#[test]
fn executable_directory_can_contain_spaces_and_unicode() {
    let app = App::fixture();
    let nested = app.dir.path.join("app folder 鍵");
    fs::create_dir(&nested).unwrap();
    let program = nested.join(app.executable.file_name().unwrap());
    copy_executable(&app.executable, &program).unwrap();
    fs::write(nested.join("a"), [1, 2]).unwrap();
    fs::write(nested.join(KEY_NAME), [4, 8]).unwrap();
    assert_code(
        &Command::new(program)
            .current_dir(&app.dir.path)
            .args(["a", "out"])
            .test_output()
            .unwrap(),
        0,
    );
    assert_eq!(fs::read(nested.join("out")).unwrap(), [5, 10]);
    assert!(!app.dir.path.join("out").exists());
}

#[test]
fn independent_parallel_invocations() {
    let app = App::fixture();
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let executable = app.executable.clone();
            let directory = app.dir.path.clone();
            std::thread::spawn(move || {
                let result = Command::new(executable)
                    .current_dir(&directory)
                    .args(["a.bin", &format!("out-{i}")])
                    .test_output()
                    .unwrap();
                assert_code(&result, 0);
                assert_eq!(
                    fs::read(directory.join(format!("out-{i}"))).unwrap(),
                    [240, 0, 255]
                );
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    app.assert_inputs();
}

#[test]
fn parallel_fixture_copy_and_launch_are_reliable() {
    let start = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                for _ in 0..8 {
                    let app = App::fixture();
                    assert_code(&app.invoke(&["a.bin", "out"]), 0);
                    assert_eq!(app.read("out"), [240, 0, 255]);
                    app.assert_inputs();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[cfg(windows)]
#[test]
fn case_aliases_of_inputs_are_rejected() {
    let app = App::fixture();
    for out in ["A.BIN", "KEY.KEY"] {
        assert_code(&app.invoke(&["a.bin", out]), 1);
    }
    app.assert_inputs();
}

#[cfg(windows)]
#[test]
fn windows_output_open_for_writing_is_preserved() {
    let app = App::fixture();
    let out = app.dir.write("out", b"original");
    let writer = OpenOptions::new().write(true).open(&out).unwrap();
    let result = app.invoke(&["a.bin", "out"]);
    assert_code(&result, 1);
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&out).unwrap(), b"original");
    app.assert_inputs();
    drop(writer);
    assert_code(&app.invoke(&["a.bin", "out"]), 0);
    assert_eq!(fs::read(out).unwrap(), [240, 0, 255]);
    app.assert_inputs();
}

#[cfg(windows)]
#[test]
fn windows_input_open_for_writing_fails_safely() {
    let app = App::fixture();
    app.dir.write("out", b"old");
    for name in ["a.bin", KEY_NAME] {
        let writer = OpenOptions::new()
            .write(true)
            .open(app.dir.path.join(name))
            .unwrap();
        assert_code(&app.invoke(&["a.bin", "out"]), 1);
        drop(writer);
    }
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[cfg(windows)]
#[test]
fn locked_output_rename_fails_without_data_loss() {
    use std::os::windows::fs::OpenOptionsExt;
    let app = App::fixture();
    let path = app.dir.write("out", b"old");
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(path)
        .unwrap();
    let result = app.invoke(&["a.bin", "out"]);
    drop(lock);
    assert_code(&result, 1);
    assert_eq!(app.read("out"), b"old");
    app.assert_inputs();
}

#[cfg(unix)]
#[test]
fn symlink_inputs_are_refused_even_inside_the_same_directory() {
    use std::os::unix::fs::symlink;
    let app = App::fixture();
    symlink("a.bin", app.dir.path.join("link")).unwrap();
    assert_code(&app.invoke(&["link", "out"]), 1);
    assert!(!app.dir.path.join("out").exists());
    app.assert_inputs();
}

#[cfg(unix)]
#[test]
fn key_symlinks_cannot_escape_executable_directory() {
    use std::os::unix::fs::symlink;
    let app = App::new();
    app.dir.write("a.bin", [1]);
    let outside = TestDir::new();
    let key = outside.write(KEY_NAME, [4]);
    symlink(&key, app.dir.path.join(KEY_NAME)).unwrap();
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert_eq!(fs::read(key).unwrap(), [4]);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}

#[cfg(unix)]
#[test]
fn output_symlinks_are_refused_including_dangling_links() {
    use std::os::unix::fs::symlink;
    let app = App::fixture();
    for target in ["a.bin", KEY_NAME, "missing"] {
        let path = app.dir.path.join("out");
        symlink(target, &path).unwrap();
        assert_code(&app.invoke(&["a.bin", "out"]), 1);
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(path).unwrap();
    }
    assert!(!app.dir.path.join("missing").exists());
    app.assert_inputs();
}

#[cfg(unix)]
#[test]
fn executable_symlink_uses_real_executable_directory() {
    use std::os::unix::fs::symlink;
    let app = App::fixture();
    let elsewhere = TestDir::new();
    let link = elsewhere.path.join("launch");
    symlink(&app.executable, &link).unwrap();
    let result = Command::new(link)
        .current_dir(&elsewhere.path)
        .args(["a.bin", "out"])
        .test_output()
        .unwrap();
    assert_code(&result, 0);
    assert_eq!(app.read("out"), [240, 0, 255]);
    assert!(!elsewhere.path.join("out").exists());
    app.assert_inputs();
}

#[cfg(unix)]
#[test]
fn key_fifo_is_rejected_without_opening_it() {
    let app = App::new();
    app.dir.write("a.bin", [1]);
    assert!(
        Command::new("mkfifo")
            .arg(app.dir.path.join(KEY_NAME))
            .test_output()
            .unwrap()
            .status
            .success()
    );
    assert_code(&app.invoke(&["a.bin", "out"]), 1);
    assert!(!app.dir.path.join("out").exists());
    app.dir.assert_clean();
}
