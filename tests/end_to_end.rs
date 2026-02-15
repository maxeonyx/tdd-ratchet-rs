// tests/end_to_end.rs
//
// Story 1: Full TDD workflow enforced by the ratchet binary.

mod common;

use common::TestDir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_bin() -> PathBuf {
    // Build path to our binary
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("cargo-ratchet");
    path
}

/// Resolve RUSTUP_HOME for subprocess isolation. When HOME is overridden
/// for git config isolation, Rustup can't find the toolchain unless we
/// explicitly pass through the real RUSTUP_HOME / CARGO_HOME.
fn rustup_home() -> PathBuf {
    if let Ok(val) = std::env::var("RUSTUP_HOME") {
        PathBuf::from(val)
    } else {
        // Default: $HOME/.rustup
        let real_home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(real_home).join(".rustup")
    }
}

fn cargo_home() -> PathBuf {
    if let Ok(val) = std::env::var("CARGO_HOME") {
        PathBuf::from(val)
    } else {
        let real_home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(real_home).join(".cargo")
    }
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
        .env("RUSTUP_HOME", rustup_home())
        .env("CARGO_HOME", cargo_home())
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
        .env("RUSTUP_HOME", rustup_home())
        .env("CARGO_HOME", cargo_home())
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), out)
}

/// Add the gatekeeper test to a test project. Must be called before the
/// first `run_ratchet()` — the ratchet requires the gatekeeper to be present.
fn add_gatekeeper(dir: &Path) {
    fs::write(
        dir.join("tests/gatekeeper.rs"),
        r#"
#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!("Run tdd-ratchet instead of cargo test.");
    }
}
"#,
    )
    .unwrap();
}

#[test]
fn init_creates_empty_status_file() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    assert!(
        dir.path().join(".test-status.json").exists(),
        "Status file should be created"
    );
    dir.pass();
}

#[test]
fn happy_path_tdd_workflow() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    // Step 1: Init ratchet
    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    add_gatekeeper(dir.path());
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
    dir.pass();
}

#[test]
fn rejects_test_that_passes_immediately() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    add_gatekeeper(dir.path());
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
    dir.pass();
}

#[test]
fn rejects_regression() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    add_gatekeeper(dir.path());
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
    dir.pass();
}

#[test]
fn rejects_disappeared_test() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    add_gatekeeper(dir.path());
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
    dir.pass();
}

#[test]
fn zero_tests_project_succeeds() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "Init should succeed: {out}");
    add_gatekeeper(dir.path());
    git_add_commit(dir.path(), "Init ratchet");

    // Only the gatekeeper test exists — should succeed
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should succeed with only gatekeeper: {out}");
    dir.pass();
}

#[test]
fn two_new_tests_one_passes_one_fails() {
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    let (ok, _) = run_ratchet_init(dir.path());
    assert!(ok);
    add_gatekeeper(dir.path());
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
    // good_test should NOT be in the TDD violations (it may appear in nextest
    // output since it fails, which is expected)
    assert!(
        !out.contains("✗ test-project::two_tests$good_test"),
        "good_test should not be flagged as a violation: {out}"
    );
    dir.pass();
}

#[test]
fn rejects_bad_git_history_skipped_pending() {
    // A test that appears as "passing" in the status file without ever
    // having been "pending" in a prior commit should be rejected.
    // This is the core enforcement: you can't squash "add failing test" +
    // "make it pass" into one commit.
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    // Init ratchet
    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    add_gatekeeper(dir.path());
    git_add_commit(dir.path(), "Init ratchet");

    // Manually write the status file with a test marked as "passing"
    // that was never "pending" — simulating someone who edited the
    // status file by hand or squashed commits.
    fs::write(
        dir.path().join("tests/sneaky.rs"),
        r#"
#[test]
fn sneaky_test() {
    assert_eq!(1 + 1, 2);
}
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join(".test-status.json"),
        r#"{
  "tests": {
    "sneaky_test": "passing",
    "tdd_ratchet_gatekeeper": "passing"
  }
}
"#,
    )
    .unwrap();
    git_add_commit(dir.path(), "Add test as passing without pending");

    // Run ratchet — should reject because history shows sneaky_test
    // appeared as "passing" without a prior "pending" commit
    let (ok, out) = run_ratchet(dir.path());
    assert!(!ok, "Ratchet should reject bad git history: {out}");
    assert!(
        out.contains("sneaky_test"),
        "Should name the test that skipped pending: {out}"
    );
    dir.pass();
}

#[test]
fn adoption_existing_project_grandfathers_tests() {
    // When adopting tdd-ratchet into an existing project that already has
    // passing tests, --init should record a baseline so those tests are
    // grandfathered and don't trigger history violations.
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    // Add a passing test BEFORE ratchet is initialized — this is the
    // "existing project" scenario
    fs::write(
        dir.path().join("tests/existing.rs"),
        r#"
#[test]
fn legacy_test() {
    assert!(true);
}
"#,
    )
    .unwrap();
    git_add_commit(dir.path(), "Add existing test (pre-ratchet)");

    // Now adopt tdd-ratchet
    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");

    // Add gatekeeper test (required by tdd-ratchet)
    fs::write(
        dir.path().join("tests/gatekeeper.rs"),
        r#"
#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!("Run tdd-ratchet instead of cargo test.");
    }
}
"#,
    )
    .unwrap();
    git_add_commit(dir.path(), "Adopt tdd-ratchet");

    // Run ratchet — legacy_test passes immediately but should be
    // accepted because it predates the baseline
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept grandfathered test: {out}");

    // Now add a NEW test that passes immediately — this should be
    // rejected even though legacy_test was allowed
    fs::write(
        dir.path().join("tests/new_cheater.rs"),
        r#"
#[test]
fn new_cheater_test() {
    assert!(true);
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(
        !ok,
        "Ratchet should reject new test that passes immediately: {out}"
    );
    assert!(
        out.contains("new_cheater_test"),
        "Should name the new offending test: {out}"
    );
    dir.pass();
}

#[test]
fn full_setup_and_tdd_workflow_from_scratch() {
    // Simulate the complete user journey, starting from the README.
    // At every step, the user is guided by tdd-ratchet's error messages.
    build_ratchet_binary();
    let dir = TestDir::new();
    create_test_project(dir.path());

    // Step 1: User tries to run ratchet without init.
    // Ratchet fails with instructions to run --init.
    let (ok, _out) = run_ratchet(dir.path());
    assert!(!ok, "Should fail without status file");

    // Step 2: User runs --init (as instructed by step 1's error message).
    let (ok, out) = run_ratchet_init(dir.path());
    assert!(ok, "init should succeed: {out}");
    git_add_commit(dir.path(), "Initialize tdd-ratchet");

    // Step 3: User runs ratchet. It fails because there's no gatekeeper
    // test. The error message tells them exactly what to add.
    let (ok, _out) = run_ratchet(dir.path());
    assert!(!ok, "Should fail without gatekeeper test");

    // Step 4: User adds gatekeeper test (as instructed by step 3's error).
    fs::write(
        dir.path().join("tests/gatekeeper.rs"),
        r#"
#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!(
            "This project uses tdd-ratchet for strict TDD.\n\
             Run `tdd-ratchet` instead of `cargo test`."
        );
    }
}
"#,
    )
    .unwrap();

    // Step 5: Ratchet succeeds. The gatekeeper is special-cased — it's
    // allowed to pass immediately (no pending state required).
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept gatekeeper: {out}");
    git_add_commit(dir.path(), "Add gatekeeper test");

    // Step 6: Write a failing test for feature A.
    fs::write(
        dir.path().join("tests/feature_a.rs"),
        r#"
#[test]
fn feature_a_works() {
    panic!("TODO: implement feature A");
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept new failing test: {out}");
    git_add_commit(dir.path(), "Add failing test for feature A");

    // Step 7: Implement feature A — test now passes, promoted.
    fs::write(
        dir.path().join("tests/feature_a.rs"),
        r#"
#[test]
fn feature_a_works() {
    assert_eq!(2 + 2, 4);
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should promote feature_a to passing: {out}");
    git_add_commit(dir.path(), "Implement feature A");

    // Step 8: Second feature — same cycle.
    fs::write(
        dir.path().join("tests/feature_b.rs"),
        r#"
#[test]
fn feature_b_works() {
    panic!("TODO: implement feature B");
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should accept second failing test: {out}");
    git_add_commit(dir.path(), "Add failing test for feature B");

    // Step 9: Implement feature B.
    fs::write(
        dir.path().join("tests/feature_b.rs"),
        r#"
#[test]
fn feature_b_works() {
    assert!(true);
}
"#,
    )
    .unwrap();
    let (ok, out) = run_ratchet(dir.path());
    assert!(ok, "Ratchet should promote feature_b to passing: {out}");
    git_add_commit(dir.path(), "Implement feature B");

    // Verify final status file has both tests as passing (full nextest names)
    let status_content = fs::read_to_string(dir.path().join(".test-status.json")).unwrap();
    assert!(
        status_content.contains("feature_a_works") && status_content.contains("\"passing\""),
        "feature_a should be passing: {status_content}"
    );
    assert!(
        status_content.contains("feature_b_works") && status_content.contains("\"passing\""),
        "feature_b should be passing: {status_content}"
    );
    dir.pass();
}
