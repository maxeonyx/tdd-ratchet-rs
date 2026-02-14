// Core ratchet logic: compare status file against test results, produce violations.

use crate::runner::{TestOutcome, TestResult};
use crate::status::{StatusFile, TestState};
use std::collections::BTreeSet;

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
pub fn check_ratchet(status: &StatusFile, results: &[TestResult]) -> RatchetOutcome {
    let mut violations = Vec::new();
    let mut updated = status.clone();

    let seen_names: BTreeSet<&str> = results.iter().map(|r| r.name.as_str()).collect();

    // Check each test result against current status
    for result in results {
        match (status.tests.get(&result.name), result.outcome) {
            // New test (not in status file)
            (None, TestOutcome::Failed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestState::Pending);
            }
            (None, TestOutcome::Passed) => {
                violations.push(RatchetViolation::NewTestPassed {
                    test: result.name.clone(),
                });
            }
            (None, TestOutcome::Ignored) => {
                // New ignored test — nothing to track yet
            }

            // Pending test
            (Some(TestState::Pending), TestOutcome::Failed) => {
                // Still failing, that's fine
            }
            (Some(TestState::Pending), TestOutcome::Passed) => {
                updated
                    .tests
                    .insert(result.name.clone(), TestState::Passing);
            }
            (Some(TestState::Pending), TestOutcome::Ignored) => {
                // Pending but ignored — tolerate
            }

            // Passing test
            (Some(TestState::Passing), TestOutcome::Passed) => {
                // Still passing, good
            }
            (Some(TestState::Passing), TestOutcome::Failed) => {
                violations.push(RatchetViolation::Regression {
                    test: result.name.clone(),
                });
            }
            (Some(TestState::Passing), TestOutcome::Ignored) => {
                // Passing but ignored this run — tolerate (user used --ignored or similar)
            }
        }
    }

    // Check for disappeared tests (in status but not in results)
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
