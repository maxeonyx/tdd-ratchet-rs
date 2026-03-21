// tests/git_history.rs
//
// Story 5 (enforcement): Verify via git history that no test skipped pending state.

mod common;

use common::TestDir;
use std::fs;
use std::path::Path;
use std::process::Command;

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
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: test appears as pending
    write_status(dir.path(), r#"{"tests":{"my_test":"pending"}}"#);
    commit(dir.path(), "Add pending test");

    // Commit 2: test promoted to passing
    write_status(dir.path(), r#"{"tests":{"my_test":"passing"}}"#);
    commit(dir.path(), "Test now passes");

    let violations = check_history(dir.path()).unwrap();
    assert!(violations.is_empty(), "Should be ok: {violations:?}");
    dir.pass();
}

#[test]
fn test_appeared_as_passing_in_first_status_snapshot_is_grandfathered() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: no status file
    fs::write(dir.path().join("README.md"), "hello").unwrap();
    commit(dir.path(), "Initial");

    // Commit 2: first committed status file contains an existing passing test.
    // Under story 13, the first status snapshot is the implicit baseline.
    write_status(dir.path(), r#"{"tests":{"cheater":"passing"}}"#);
    commit(dir.path(), "Add passing test");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        violations.is_empty(),
        "First status snapshot should be grandfathered: {violations:?}"
    );
    dir.pass();
}

#[test]
fn test_pending_for_multiple_commits_then_passing_is_ok() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"slow_test":"pending"}}"#);
    commit(dir.path(), "Add pending test");

    // Another commit, still pending
    fs::write(dir.path().join("notes.txt"), "wip").unwrap();
    commit(dir.path(), "Work in progress");

    write_status(dir.path(), r#"{"tests":{"slow_test":"passing"}}"#);
    commit(dir.path(), "Test now passes");

    let violations = check_history(dir.path()).unwrap();
    assert!(violations.is_empty(), "Should be ok: {violations:?}");
    dir.pass();
}

#[test]
fn first_status_snapshot_grandfathers_existing_tests() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: first committed status file already contains an existing passing test.
    // Under the story 13 model, this first snapshot is the implicit baseline.
    write_status(dir.path(), r#"{"tests":{"old_test":"passing"}}"#);
    commit(dir.path(), "Old test");

    // Commit 2: new test appears as passing after the first status snapshot — violation.
    write_status(
        dir.path(),
        r#"{"tests":{"old_test":"passing","new_cheater":"passing"}}"#,
    );
    commit(dir.path(), "Add cheater after first snapshot");

    let violations = check_history(dir.path()).unwrap();
    // old_test should be grandfathered by the first snapshot, new_cheater should be flagged
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
    dir.pass();
}

#[test]
fn no_status_file_in_history_is_ok() {
    let dir = TestDir::new();
    init_repo(dir.path());

    fs::write(dir.path().join("README.md"), "hello").unwrap();
    commit(dir.path(), "Initial");

    let violations = check_history(dir.path()).unwrap();
    assert!(violations.is_empty());
    dir.pass();
}

#[test]
fn per_test_baseline_grandfathers_individual_test() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: no status file yet.
    fs::write(dir.path().join("README.md"), "hello").unwrap();
    commit(dir.path(), "Initial");

    // Commit 2: first status snapshot. This is the implicit global baseline.
    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Add first status snapshot");

    // Get a commit hash before the test appears, to use as a per-test baseline.
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.path())
        .output()
        .unwrap();
    let baseline_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Commit 3: grandfathered_test appears as passing with per-test baseline,
    // cheater_test appears as passing without any baseline.
    let status_json = format!(
        r#"{{"tests":{{"existing":"passing","grandfathered":{{"state":"passing","baseline":"{baseline_commit}"}},"cheater":"passing"}}}}"#
    );
    write_status(dir.path(), &status_json);
    commit(dir.path(), "Add tests");

    let violations = check_history(dir.path()).unwrap();

    // grandfathered should NOT be flagged (has per-test baseline)
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "grandfathered")
        ),
        "grandfathered should not be flagged: {violations:?}"
    );
    // cheater SHOULD be flagged (no baseline, skipped pending)
    assert!(
        violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "cheater")
        ),
        "cheater should be flagged: {violations:?}"
    );
    dir.pass();
}
