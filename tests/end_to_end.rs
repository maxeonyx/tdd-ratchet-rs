// tests/end_to_end.rs
//
// Story 1: Full TDD workflow enforced by the ratchet binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn cargo_bin() -> PathBuf {
    // Build path to our binary
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("tdd-ratchet");
    path
}

fn build_ratchet_binary() {
    let status = Command::new("cargo")
        .args(["build"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap();
    assert!(status.success(), "Failed to build tdd-ratchet binary");
}

/// Create a minimal Rust project with git repo in the given dir.
fn create_test_project(dir: &Path) {
    // Init git repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();

    // Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "").unwrap();
    fs::create_dir_all(dir.join("tests")).unwrap();

    // Initial commit
    git_add_commit(dir, "Initial project");
}

fn git_add_commit(dir: &Path, msg: &str) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", msg, "--allow-empty"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
}

fn run_ratchet(dir: &Path) -> (bool, String) {
    let output = Command::new(cargo_bin())
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), out)
}

fn run_ratchet_init(dir: &Path) -> (bool, String) {
    let output = Command::new(cargo_bin())
        .arg("--init")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), out)
}

#[test]
fn init_creates_empty_status_file() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    assert!(
        dir.path().join(".test-status.json").exists(),
        "Status file should be created"
    );
}

#[test]
fn happy_path_tdd_workflow() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    // Step 1: Init ratchet
    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    git_add_commit(dir.path(), "Add ratchet status file");

    // Step 2: Add a FAILING test
    fs::write(
        dir.path().join("tests/my_feature.rs"),
        r#"
#[test]
fn my_feature_test() {
    panic!("Not yet implemented");
}
"#,
    )
    .unwrap();

    // Step 3: Run ratchet — should succeed, test tracked as pending
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept failing new test: {out}");
    git_add_commit(dir.path(), "Add failing test");

    // Step 4: Make the test pass
    fs::write(
        dir.path().join("tests/my_feature.rs"),
        r#"
#[test]
fn my_feature_test() {
    assert_eq!(2 + 2, 4);
}
"#,
    )
    .unwrap();

    // Step 5: Run ratchet — should succeed, test promoted to passing
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept now-passing test: {out}");
    git_add_commit(dir.path(), "Implement feature");
}

#[test]
fn rejects_test_that_passes_immediately() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Init ratchet");

    // Add a test that passes immediately
    fs::write(
        dir.path().join("tests/cheater.rs"),
        r#"
#[test]
fn cheater_test() {
    assert!(true);
}
"#,
    )
    .unwrap();

    let (ok, out) = run_ratchet(dir.path());
    assert!(
        !ok,
        "Ratchet should reject test that passes immediately: {out}"
    );
    assert!(
        out.contains("cheater_test"),
        "Should name the offending test: {out}"
    );
}

#[test]
fn rejects_regression() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Init ratchet");

    // Add failing test
    fs::write(
        dir.path().join("tests/will_regress.rs"),
        r#"
#[test]
fn fragile_test() {
    panic!("not done");
}
"#,
    )
    .unwrap();
    let (ok, _) = run_ratchet(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Add failing test");

    // Make it pass
    fs::write(
        dir.path().join("tests/will_regress.rs"),
        r#"
#[test]
fn fragile_test() {
    assert_eq!(1, 1);
}
"#,
    )
    .unwrap();
    let (ok, _) = run_ratchet(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Make test pass");

    // Now break it (regression)
    fs::write(
        dir.path().join("tests/will_regress.rs"),
        r#"
#[test]
fn fragile_test() {
    panic!("broken again");
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(!ok, "Ratchet should reject regression: {out}");
    assert!(
        out.contains("fragile_test"),
        "Should name the regressed test: {out}"
    );
}

#[test]
fn rejects_disappeared_test() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Init ratchet");

    // Add and complete a test through the full cycle
    fs::write(
        dir.path().join("tests/temp_test.rs"),
        r#"
#[test]
fn temporary() {
    panic!("wip");
}
"#,
    )
    .unwrap();
    let (ok, _) = run_ratchet(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Add failing test");

    fs::write(
        dir.path().join("tests/temp_test.rs"),
        r#"
#[test]
fn temporary() {
    assert!(true);
}
"#,
    )
    .unwrap();
    let (ok, _) = run_ratchet(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Make test pass");

    // Now remove the test file without updating status file
    fs::remove_file(dir.path().join("tests/temp_test.rs")).unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(!ok, "Ratchet should reject disappeared test: {out}");
    assert!(
        out.contains("temporary"),
        "Should name the disappeared test: {out}"
    );
}

#[test]
fn zero_tests_project_succeeds() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "Init should succeed: {out}");
    git_add_commit(dir.path(), "Init ratchet");

    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should succeed with zero tests: {out}");
}

#[test]
fn two_new_tests_one_passes_one_fails() {
    build_ratchet_binary();
    let dir = TempDir::new().unwrap();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    git_add_commit(dir.path(), "Init ratchet");

    // Add two tests in same commit — one fails, one passes
    fs::write(
        dir.path().join("tests/two_tests.rs"),
        r#"
#[test]
fn good_test() {
    panic!("properly failing first");
}

#[test]
fn bad_test() {
    assert!(true); // passes immediately — violation
}
"#,
    )
    .unwrap();

    let (ok, out) = run_ratchet(dir.path());
    assert!(!ok, "Should reject the passing test: {out}");
    assert!(out.contains("bad_test"), "Should name bad_test: {out}");
    // good_test should NOT be in the violations
    assert!(
        !out.contains("good_test") || out.contains("pending"),
        "good_test should be accepted: {out}"
    );
}
