use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn data(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

pub fn native_name() -> std::ffi::OsString {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0x61, 0xD800, 0x62])
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![b'a', 0xFF, b'b'])
    }
}

pub struct TestDir {
    pub path: PathBuf,
    base: PathBuf,
}

impl TestDir {
    pub fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!(
            "astra-xor-test-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path, base }
    }

    pub fn write(&self, name: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    pub fn assert_clean(&self) {
        for entry in fs::read_dir(&self.path).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                !name.to_string_lossy().starts_with(".astra-xor-"),
                "temporary output leaked: {name:?}"
            );
        }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // Recursive cleanup is confined to the exact temporary directory that
        // this test created. Resolve and verify before deleting on any platform.
        if let Ok(resolved) = self.path.canonicalize()
            && resolved.parent() == Some(self.base.as_path())
            && resolved
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("astra-xor-test-"))
        {
            let _ = fs::remove_dir_all(resolved);
        }
    }
}
