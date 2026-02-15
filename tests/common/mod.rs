// tests/common/mod.rs
//
// Shared test helpers. Imported via `mod common;` in integration test files.

use std::fs;
use std::path::{Path, PathBuf};

/// A temp directory that persists on test failure (for debugging) but
/// cleans up on success. Call `.pass()` at the end of a passing test.
///
/// Dirs use a recognizable prefix (`tdd-ratchet-test-`) so stale ones
/// from failed runs can be identified and cleaned manually:
///   rm -rf /tmp/tdd-ratchet-test-*
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("tdd-ratchet-test-")
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
