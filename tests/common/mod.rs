// tests/common/mod.rs
//
// Shared test helpers. Imported via `mod common;` in integration test files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

const TEMP_PREFIX: &str = "tdd-ratchet-test-";

/// A temp directory that persists on test failure (for debugging) but
/// cleans up on success. Call `.pass()` at the end of a passing test.
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new() -> Self {
        static CLEANUP: Once = Once::new();
        CLEANUP.call_once(cleanup_stale_test_dirs);

        let dir = tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .tempdir()
            .unwrap();
        let path = dir.keep();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Clean up the temp dir. Call at the very end of a test — if an
    /// assertion panics before this line, the directory is preserved.
    pub fn pass(self) {
        fs::remove_dir_all(&self.path).ok();
    }
}

/// Remove stale temp dirs from previous test runs.
fn cleanup_stale_test_dirs() {
    let tmp = std::env::temp_dir();
    if let Ok(entries) = fs::read_dir(&tmp) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(TEMP_PREFIX) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }
    }
}
