// Core ratchet logic: compare status file against test results, produce violations.

use crate::history::{check_history_snapshots, HistorySnapshot, HistoryViolation};
use crate::runner::{TestOutcome, TestResult};
use crate::status::{StatusFile, TestEntry, TestState};
use std::collections::BTreeSet;

/// The gatekeeper test name. This test is special-cased: it's allowed to
/// pass immediately without going through the pending state, because the
/// ratchet itself sets TDD_RATCHET=1 when running tests.
pub const GATEKEEPER_TEST_NAME: &str = "tdd_ratchet_gatekeeper";

/// The complete result of evaluating the ratchet. Contains all violations
/// (ratchet rules, history, gatekeeper) and the updated status file.
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub violations: Vec<Violation>,
    pub updated: StatusFile,
}

/// A unified violation type covering all ratchet checks.
#[derive(Debug, Clone)]
pub enum Violation {
    /// A new test passed without being pending first
    NewTestPassed { test: String },
    /// A passing test now fails — regression
    Regression { test: String },
    /// A tracked test disappeared from the run
    TestDisappeared { test: String },
    /// A test appeared as passing in git history without prior pending state
    SkippedPending { test: String, commit: String },
    /// No gatekeeper test found in the test run
    MissingGatekeeper,
}

/// Evaluate all ratchet rules. Pure function — no IO.
///
/// Takes the current status file, test results, and git history snapshots.
/// Returns all violations and the updated status file with valid transitions
/// applied (new pending tests, promotions to passing).
pub fn evaluate(
    status: &StatusFile,
    results: &[TestResult],
    history_snapshots: &[HistorySnapshot],
) -> EvalResult {
    let mut violations = Vec::new();
    let mut updated = status.clone();

    // 1. Check gatekeeper presence
    let has_gatekeeper = results
        .iter()
        .any(|r| r.name.ends_with(GATEKEEPER_TEST_NAME));
    if !has_gatekeeper {
        violations.push(Violation::MissingGatekeeper);
    }

    // 2. Apply ratchet rules (state transitions)
    let seen_names: BTreeSet<&str> = results.iter().map(|r| r.name.as_str()).collect();

    for result in results {
        match (
            status.tests.get(&result.name).map(|e| e.state()),
            result.outcome,
        ) {
            // New test (not in status file)
            (None, TestOutcome::Failed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestEntry::Simple(TestState::Pending));
            }
            (None, TestOutcome::Passed) => {
                if result.name.ends_with(GATEKEEPER_TEST_NAME) {
                    updated
                        .tests
                        .insert(result.name.clone(), TestEntry::Simple(TestState::Passing));
                } else {
                    violations.push(Violation::NewTestPassed {
                        test: result.name.clone(),
                    });
                }
            }
            (None, TestOutcome::Ignored) => {}

            // Pending test
            (Some(TestState::Pending), TestOutcome::Failed) => {}
            (Some(TestState::Pending), TestOutcome::Passed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestEntry::Simple(TestState::Passing));
            }
            (Some(TestState::Pending), TestOutcome::Ignored) => {}

            // Passing test
            (Some(TestState::Passing), TestOutcome::Passed) => {}
            (Some(TestState::Passing), TestOutcome::Failed) => {
                violations.push(Violation::Regression {
                    test: result.name.clone(),
                });
            }
            (Some(TestState::Passing), TestOutcome::Ignored) => {}
        }
    }

    // Check for disappeared tests
    for name in status.tests.keys() {
        if !seen_names.contains(name.as_str()) {
            violations.push(Violation::TestDisappeared { test: name.clone() });
        }
    }

    // 3. Check git history
    let history_violations = check_history_snapshots(history_snapshots, status.baseline.is_some());
    for hv in history_violations {
        match hv {
            HistoryViolation::SkippedPending { test, commit } => {
                violations.push(Violation::SkippedPending { test, commit });
            }
        }
    }

    EvalResult {
        violations,
        updated,
    }
}

// --- Legacy API kept for existing unit tests ---

#[derive(Debug, Clone)]
pub struct RatchetOutcome {
    pub violations: Vec<RatchetViolation>,
    pub updated: StatusFile,
}

#[derive(Debug, Clone)]
pub enum RatchetViolation {
    /// A new test passed without being pending first (story 5)
    NewTestPassed { test: String },
    /// A passing test now fails — regression (story 6)
    Regression { test: String },
    /// A tracked test disappeared from the run (story 7)
    TestDisappeared { test: String },
}

/// Check test results against the status file. Returns violations and the updated status file.
///
/// This is the original per-rule check without history or gatekeeper.
/// Used by unit tests in state_transitions.rs.
pub fn check_ratchet(status: &StatusFile, results: &[TestResult]) -> RatchetOutcome {
    let mut violations = Vec::new();
    let mut updated = status.clone();

    let seen_names: BTreeSet<&str> = results.iter().map(|r| r.name.as_str()).collect();

    for result in results {
        match (
            status.tests.get(&result.name).map(|e| e.state()),
            result.outcome,
        ) {
            (None, TestOutcome::Failed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestEntry::Simple(TestState::Pending));
            }
            (None, TestOutcome::Passed) => {
                if result.name.ends_with(GATEKEEPER_TEST_NAME) {
                    updated
                        .tests
                        .insert(result.name.clone(), TestEntry::Simple(TestState::Passing));
                } else {
                    violations.push(RatchetViolation::NewTestPassed {
                        test: result.name.clone(),
                    });
                }
            }
            (None, TestOutcome::Ignored) => {}
            (Some(TestState::Pending), TestOutcome::Failed) => {}
            (Some(TestState::Pending), TestOutcome::Passed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestEntry::Simple(TestState::Passing));
            }
            (Some(TestState::Pending), TestOutcome::Ignored) => {}
            (Some(TestState::Passing), TestOutcome::Passed) => {}
            (Some(TestState::Passing), TestOutcome::Failed) => {
                violations.push(RatchetViolation::Regression {
                    test: result.name.clone(),
                });
            }
            (Some(TestState::Passing), TestOutcome::Ignored) => {}
        }
    }

    for name in status.tests.keys() {
        if !seen_names.contains(name.as_str()) {
            violations.push(RatchetViolation::TestDisappeared { test: name.clone() });
        }
    }

    RatchetOutcome {
        violations,
        updated,
    }
}
