// tests/git_history.rs
//
// Story 5 (enforcement): Verify via git history that no test skipped pending state.

mod common;

use common::TestDir;
use std::fs;
use std::path::Path;
use std::process::Command;

use tdd_ratchet::history::{
    BaselineViolation, HistoryViolation, check_adoption_baseline, check_history,
};

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

fn head_sha(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
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
fn bootstrap_no_baseline_trusts_first_snapshot() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // No baseline anywhere → bootstrap. The first status snapshot is trusted,
    // exactly as the old first-snapshot grandfathering behaved.
    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "First status snapshot");

    // A test first passing in a later snapshot without a prior pending must
    // still be flagged.
    write_status(
        dir.path(),
        r#"{"tests":{"existing":"passing","cheater":"passing"}}"#,
    );
    commit(dir.path(), "Add cheater after first snapshot");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "existing")
        ),
        "First-snapshot test should be trusted in bootstrap: {violations:?}"
    );
    assert!(
        violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "cheater")
        ),
        "Test first passing after the first snapshot should be flagged: {violations:?}"
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
fn status_file_deletion_after_adoption_is_rejected() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Adopt the ledger");

    fs::remove_file(dir.path().join(".test-status.json")).unwrap();
    commit(dir.path(), "Delete the ledger");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.is_empty(),
        "Deleting the ledger after adoption must remain a history violation"
    );
    dir.pass();
}

#[test]
fn reinitialising_after_deletion_is_not_a_new_adoption() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Adopt the ledger");

    fs::remove_file(dir.path().join(".test-status.json")).unwrap();
    commit(dir.path(), "Delete the ledger");

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Reinitialise the ledger");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.is_empty(),
        "Recreating the same ledger must not hide the intervening deletion"
    );
    dir.pass();
}

#[test]
fn committed_passing_to_pending_rewrite_is_rejected() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Adopt the ledger");

    write_status(dir.path(), r#"{"tests":{"existing":"pending"}}"#);
    commit(dir.path(), "Rewrite passing as pending");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.is_empty(),
        "Committed passing-to-pending edits must not repair history"
    );
    dir.pass();
}

#[test]
fn removing_a_skipped_pending_violation_does_not_repair_history() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Adopt the ledger");

    write_status(
        dir.path(),
        r#"{"tests":{"existing":"passing","cheater":"passing"}}"#,
    );
    commit(dir.path(), "Skip the pending state");

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Try to erase the violation");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        violations.iter().any(
            |violation| matches!(violation, HistoryViolation::SkippedPending { test, .. } if test == "cheater")
        ),
        "A bad committed transition must remain bad until history is rewritten: {violations:?}"
    );
    dir.pass();
}

#[test]
fn committed_rename_bridges_history_identity() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(
        dir.path(),
        r#"{"tests":{"old_test":"pending","tdd_ratchet_gatekeeper":"passing"}}"#,
    );
    commit(dir.path(), "Add pending test");

    write_status(
        dir.path(),
        r#"{"tests":{"new_test":"passing","tdd_ratchet_gatekeeper":"passing"},"renames":{"new_test":"old_test"}}"#,
    );
    commit(dir.path(), "Rename and pass test");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "new_test")
        ),
        "Committed rename should bridge history for new_test: {violations:?}"
    );
    dir.pass();
}

#[test]
fn historical_snapshots_ignore_unknown_top_level_fields() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(
        dir.path(),
        r#"{"tests":{"legacy":"passing"},"baseline":"0123456789abcdef0123456789abcdef01234567","future_field":{"note":"keep going"}}"#,
    );
    commit(dir.path(), "Add legacy status snapshot");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        violations.is_empty(),
        "Historical unknown fields should be ignored: {violations:?}"
    );
    dir.pass();
}

#[test]
fn removed_tests_stop_participating_in_history_checks() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"retired_test":"passing"}}"#);
    commit(dir.path(), "Track retired test as passing");

    write_status(dir.path(), r#"{"tests":{}}"#);
    commit(dir.path(), "Remove retired test from status file");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        violations.is_empty(),
        "Tests removed from the latest status file should stop affecting history checks: {violations:?}"
    );
    dir.pass();
}

#[test]
fn later_removed_tests_do_not_keep_old_history_violations_alive() {
    let dir = TestDir::new();
    init_repo(dir.path());

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Initial tracked tests");

    write_status(
        dir.path(),
        r#"{"tests":{"existing":"passing","temporary_cheater":"passing"}}"#,
    );
    commit(dir.path(), "Add temporary cheater");

    write_status(dir.path(), r#"{"tests":{"existing":"passing"}}"#);
    commit(dir.path(), "Remove temporary cheater");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        violations.is_empty(),
        "Removed tests should not keep old skipped-pending violations alive: {violations:?}"
    );
    dir.pass();
}

#[test]
fn tests_in_adoption_snapshot_are_trusted() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: a first status snapshot (so the adoption snapshot is NOT the
    // first snapshot — that is what distinguishes the new rule from the old
    // first-snapshot grandfathering).
    write_status(dir.path(), r#"{"tests":{"early":"passing"}}"#);
    commit(dir.path(), "First status snapshot");
    let baseline = head_sha(dir.path());

    // Commit 2: declares baseline = commit 1. This is the first status snapshot
    // strictly after the baseline, i.e. the adoption snapshot. A test appearing
    // passing here with no prior pending must be trusted.
    write_status(
        dir.path(),
        &format!(
            r#"{{"tests":{{"early":"passing","adopted":"passing"}},"baseline":"{baseline}"}}"#
        ),
    );
    commit(dir.path(), "Adoption snapshot");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "adopted")
        ),
        "Tests in the adoption snapshot should be trusted: {violations:?}"
    );
    dir.pass();
}

#[test]
fn test_first_passing_after_adoption_snapshot_is_flagged() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1: first status snapshot.
    write_status(dir.path(), r#"{"tests":{"early":"passing"}}"#);
    commit(dir.path(), "First status snapshot");
    let baseline = head_sha(dir.path());

    // Commit 2: adoption snapshot (baseline = commit 1). "adopted" is trusted.
    write_status(
        dir.path(),
        &format!(
            r#"{{"tests":{{"early":"passing","adopted":"passing"}},"baseline":"{baseline}"}}"#
        ),
    );
    commit(dir.path(), "Adoption snapshot");

    // Commit 3: a brand-new test appears passing AFTER the adoption snapshot
    // with no prior pending — this must be flagged.
    write_status(
        dir.path(),
        &format!(
            r#"{{"tests":{{"early":"passing","adopted":"passing","late_cheater":"passing"}},"baseline":"{baseline}"}}"#
        ),
    );
    commit(dir.path(), "Add late cheater");

    let violations = check_history(dir.path()).unwrap();
    assert!(
        !violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "adopted")
        ),
        "Adoption-snapshot test should stay trusted: {violations:?}"
    );
    assert!(
        violations.iter().any(
            |v| matches!(v, HistoryViolation::SkippedPending { test, .. } if test == "late_cheater")
        ),
        "Test first passing after the adoption snapshot should be flagged: {violations:?}"
    );
    dir.pass();
}

#[test]
fn two_commit_check_detects_moved_baseline() {
    let dir = TestDir::new();
    init_repo(dir.path());

    // Commit 1 (C1): carries its own baseline value X.
    write_status(
        dir.path(),
        r#"{"tests":{"a":"passing"},"baseline":"1111111111111111111111111111111111111111"}"#,
    );
    commit(dir.path(), "C1 with baseline X");
    let c1 = head_sha(dir.path());

    // Commit 2 (HEAD): baseline points at C1, but declares a different value Y.
    write_status(
        dir.path(),
        &format!(r#"{{"tests":{{"a":"passing"}},"baseline":"{c1}"}}"#),
    );
    commit(dir.path(), "HEAD baseline points at C1");

    let violation = check_adoption_baseline(dir.path()).unwrap();
    assert!(
        matches!(violation, Some(BaselineViolation::Moved { .. })),
        "Moved baseline should be detected: {violation:?}"
    );
    dir.pass();
}

#[test]
fn adoption_baseline_bootstrap_passes() {
    // (a) HEAD has no baseline → pass.
    let dir = TestDir::new();
    init_repo(dir.path());
    write_status(dir.path(), r#"{"tests":{"a":"passing"}}"#);
    commit(dir.path(), "No baseline");
    assert!(
        check_adoption_baseline(dir.path()).unwrap().is_none(),
        "HEAD with no baseline should pass"
    );
    dir.pass();

    // (b) HEAD baseline points at a commit with no baseline field → pass.
    let dir = TestDir::new();
    init_repo(dir.path());
    write_status(dir.path(), r#"{"tests":{"a":"passing"}}"#);
    commit(dir.path(), "Pointed-at commit, no baseline");
    let c1 = head_sha(dir.path());
    write_status(
        dir.path(),
        &format!(r#"{{"tests":{{"a":"passing"}},"baseline":"{c1}"}}"#),
    );
    commit(dir.path(), "HEAD points at baseline-less commit");
    assert!(
        check_adoption_baseline(dir.path()).unwrap().is_none(),
        "Pointed-at commit with no baseline should pass"
    );
    dir.pass();

    // (c) HEAD baseline is an unresolvable SHA → pass.
    let dir = TestDir::new();
    init_repo(dir.path());
    write_status(
        dir.path(),
        r#"{"tests":{"a":"passing"},"baseline":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}"#,
    );
    commit(dir.path(), "Unresolvable baseline");
    assert!(
        check_adoption_baseline(dir.path()).unwrap().is_none(),
        "Unresolvable baseline SHA should pass"
    );
    dir.pass();
}
