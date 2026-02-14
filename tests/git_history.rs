// tests/git_history.rs
//
// Story 5 (enforcement): Verify via git history that no test skipped pending state.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use tdd_ratchet::history::{check_history, HistoryViolation};

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.email", "test@test.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

fn write_status(dir: &Path, json: &str) {
    fs::write(dir.join(".test-status.json"), json).unwrap();
}

fn commit(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", msg, "--allow-empty"]);
}

#[test]
fn test_appeared_as_pending_then_passing_is_ok() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());

    // Commit 1: test appears as pending
    write_status(dir.path(), r#"{"tests":{"my_test":"pending"}}"#);
    commit(dir.path(), "Add pending test");

    // Commit 2: test promoted to passing
    write_status(dir.path(), r#"{"tests":{"my_test":"passing"}}"#);
    commit(dir.path(), "Test now passes");

    let violations = check_history(dir.path(), None).unwrap();
    assert!(violations.is_empty(), "Should be ok: {violations:?}");
}

#[test]
fn test_appeared_as_passing_without_pending_is_rejected() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());

    // Commit 1: no status file
    fs::write(dir.path().join("README.md"), "hello").unwrap();
    commit(dir.path(), "Initial");

    // Commit 2: test appears directly as passing (skipped pending)
    write_status(dir.path(), r#"{"tests":{"cheater":"passing"}}"#);
    commit(dir.path(), "Add passing test");

    let violations = check_history(dir.path(), None).unwrap();
    assert!(
        violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "cheater")
        ),
        "Should reject: {violations:?}"
    );
}

#[test]
fn test_pending_for_multiple_commits_then_passing_is_ok() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"slow_test":"pending"}}"#);
    commit(dir.path(), "Add pending test");

    // Another commit, still pending
    fs::write(dir.path().join("notes.txt"), "wip").unwrap();
    commit(dir.path(), "Work in progress");

    write_status(dir.path(), r#"{"tests":{"slow_test":"passing"}}"#);
    commit(dir.path(), "Test now passes");

    let violations = check_history(dir.path(), None).unwrap();
    assert!(violations.is_empty(), "Should be ok: {violations:?}");
}

#[test]
fn baseline_commit_grandfathers_existing_tests() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());

    // Commit 1: test appears as passing (before baseline)
    write_status(dir.path(), r#"{"tests":{"old_test":"passing"}}"#);
    commit(dir.path(), "Old test");

    // Get that commit hash
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.path())
        .output()
        .unwrap();
    let baseline = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Commit 2: new test appears as passing (after baseline — violation)
    write_status(
        dir.path(),
        r#"{"tests":{"old_test":"passing","new_cheater":"passing"}}"#,
    );
    commit(dir.path(), "Add cheater after baseline");

    let violations = check_history(dir.path(), Some(&baseline)).unwrap();
    // old_test should be grandfathered, new_cheater should be flagged
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "old_test")
        ),
        "old_test should be grandfathered: {violations:?}"
    );
    assert!(
        violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "new_cheater")
        ),
        "new_cheater should be flagged: {violations:?}"
    );
}

#[test]
fn no_status_file_in_history_is_ok() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());

    fs::write(dir.path().join("README.md"), "hello").unwrap();
    commit(dir.path(), "Initial");

    let violations = check_history(dir.path(), None).unwrap();
    assert!(violations.is_empty());
}
