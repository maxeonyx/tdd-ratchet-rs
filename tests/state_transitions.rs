// tests/state_transitions.rs
//
// Stories 5, 6, 7: The core ratchet rules.

use tdd_ratchet::ratchet::{check_ratchet, RatchetViolation};
use tdd_ratchet::runner::{TestOutcome, TestResult};
use tdd_ratchet::status::{StatusFile, TestState};

fn status(tests: &[(&str, TestState)]) -> StatusFile {
    StatusFile {
        tests: tests.iter().map(|(n, s)| (n.to_string(), *s)).collect(),
        baseline: None,
    }
}

fn results(tests: &[(&str, TestOutcome)]) -> Vec<TestResult> {
    tests
        .iter()
        .map(|(n, o)| TestResult {
            name: n.to_string(),
            outcome: *o,
        })
        .collect()
}

// --- Story 5: New tests must fail first ---

#[test]
fn new_test_that_fails_is_accepted_as_pending() {
    let sf = status(&[]);
    let tr = results(&[("new_test", TestOutcome::Failed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(
        outcome.violations.is_empty(),
        "Should accept: {:?}",
        outcome.violations
    );
    assert_eq!(outcome.updated.tests["new_test"], TestState::Pending,);
}

#[test]
fn new_test_that_passes_is_rejected() {
    let sf = status(&[]);
    let tr = results(&[("new_test", TestOutcome::Passed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, RatchetViolation::NewTestPassed { .. })),
        "Should reject new passing test: {:?}",
        outcome.violations
    );
}

// --- Story 5 continued: Pending → Passing ---

#[test]
fn pending_test_that_now_passes_is_promoted() {
    let sf = status(&[("my_test", TestState::Pending)]);
    let tr = results(&[("my_test", TestOutcome::Passed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.updated.tests["my_test"], TestState::Passing);
}

#[test]
fn pending_test_still_failing_is_ok() {
    let sf = status(&[("my_test", TestState::Pending)]);
    let tr = results(&[("my_test", TestOutcome::Failed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.updated.tests["my_test"], TestState::Pending);
}

// --- Story 6: Passing tests must keep passing ---

#[test]
fn passing_test_still_passing_is_ok() {
    let sf = status(&[("my_test", TestState::Passing)]);
    let tr = results(&[("my_test", TestOutcome::Passed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.updated.tests["my_test"], TestState::Passing);
}

#[test]
fn passing_test_now_fails_is_rejected() {
    let sf = status(&[("my_test", TestState::Passing)]);
    let tr = results(&[("my_test", TestOutcome::Failed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, RatchetViolation::Regression { .. })),
        "Should reject regression: {:?}",
        outcome.violations
    );
}

// --- Story 7: Tracked tests must not disappear ---

#[test]
fn tracked_test_missing_from_run_is_rejected() {
    let sf = status(&[("existing_test", TestState::Passing)]);
    let tr = results(&[]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, RatchetViolation::TestDisappeared { .. })),
        "Should reject disappeared test: {:?}",
        outcome.violations
    );
}

// --- Edge cases ---

#[test]
fn multiple_violations_all_reported() {
    let sf = status(&[("tracked", TestState::Passing)]);
    let tr = results(&[("new_passing", TestOutcome::Passed)]);
    // Two violations: "tracked" disappeared AND "new_passing" passes without being pending
    let outcome = check_ratchet(&sf, &tr);
    assert!(
        outcome.violations.len() >= 2,
        "Expected multiple violations: {:?}",
        outcome.violations
    );
}

#[test]
fn empty_status_all_tests_pass_all_rejected() {
    let sf = status(&[]);
    let tr = results(&[("a", TestOutcome::Passed), ("b", TestOutcome::Passed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert_eq!(
        outcome
            .violations
            .iter()
            .filter(|v| matches!(v, RatchetViolation::NewTestPassed { .. }))
            .count(),
        2,
    );
}

#[test]
fn empty_status_all_tests_fail_all_accepted_as_pending() {
    let sf = status(&[]);
    let tr = results(&[("a", TestOutcome::Failed), ("b", TestOutcome::Failed)]);
    let outcome = check_ratchet(&sf, &tr);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.updated.tests["a"], TestState::Pending);
    assert_eq!(outcome.updated.tests["b"], TestState::Pending);
}

#[test]
fn empty_results_nonempty_status_all_rejected_as_missing() {
    let sf = status(&[("a", TestState::Passing), ("b", TestState::Pending)]);
    let tr = results(&[]);
    let outcome = check_ratchet(&sf, &tr);
    assert_eq!(
        outcome
            .violations
            .iter()
            .filter(|v| matches!(v, RatchetViolation::TestDisappeared { .. }))
            .count(),
        2,
    );
}

#[test]
fn ignored_tests_are_not_counted_as_disappeared() {
    let sf = status(&[("my_test", TestState::Passing)]);
    let tr = results(&[("my_test", TestOutcome::Ignored)]);
    let outcome = check_ratchet(&sf, &tr);
    // Ignored tests should be tolerated — they're still present, just skipped
    assert!(
        outcome.violations.is_empty(),
        "Ignored tests should not be violations: {:?}",
        outcome.violations
    );
}
