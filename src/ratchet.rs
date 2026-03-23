// Core ratchet logic: compare status file against test results, produce violations.

use crate::history::{check_history_snapshots, HistorySnapshot, HistoryViolation};
use crate::runner::{TestOutcome, TestResult};
use crate::status::{StatusFile, TestState};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
struct TransitionOutcome {
    violations: Vec<TransitionViolation>,
    updated: StatusFile,
}

#[derive(Debug, Clone)]
enum TransitionViolation {
    NewTestPassed { test: String },
    Regression { test: String },
    TestDisappeared { test: String },
}

/// The gatekeeper test name. This test is special-cased: it's allowed to
/// pass immediately without going through the pending state, because the
/// ratchet itself sets TDD_RATCHET=1 when running tests.
pub const GATEKEEPER_TEST_NAME: &str = "tdd_ratchet_gatekeeper";

/// The complete result of evaluating the ratchet. Contains all violations
/// (ratchet rules, history, gatekeeper) and the updated status file.
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub violations: Vec<Violation>,
    pub warnings: Vec<Warning>,
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
    /// Rename declared for an old test name not present in committed status
    RenameOldNameMissing { new_name: String, old_name: String },
    /// Rename declared for a new test name not present in current results
    RenameNewNameMissing { new_name: String, old_name: String },
    /// Rename declared but old test name still appears in current results
    RenameOldNameStillPresent { new_name: String, old_name: String },
    /// Rename declared to a name already tracked independently
    RenameNewNameAlreadyTracked { new_name: String, old_name: String },
    /// Multiple rename declarations target the same old name
    RenameOldNameMappedMultipleTimes { old_name: String },
}

#[derive(Debug, Clone)]
pub enum Warning {
    RenameApplied { new_name: String, old_name: String },
    StaleRename { new_name: String, old_name: String },
}

#[derive(Debug, Clone)]
struct IdentityResolution {
    status: StatusFile,
    results: Vec<TestResult>,
    violations: Vec<Violation>,
    warnings: Vec<Warning>,
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
    let mut warnings = Vec::new();

    // 1. Check gatekeeper presence
    let has_gatekeeper = results
        .iter()
        .any(|r| r.name.ends_with(GATEKEEPER_TEST_NAME));
    if !has_gatekeeper {
        violations.push(Violation::MissingGatekeeper);
    }

    let identity = resolve_identities(status, results);
    violations.extend(identity.violations);
    warnings.extend(identity.warnings);

    // 2. Apply ratchet rules (state transitions)
    let transition_outcome = apply_transitions(&identity.status, &identity.results);
    violations.extend(
        transition_outcome
            .violations
            .into_iter()
            .map(map_transition_violation),
    );

    // 3. Check git history
    let history_violations = check_history_snapshots(history_snapshots);
    for hv in history_violations {
        match hv {
            HistoryViolation::SkippedPending { test, commit } => {
                violations.push(Violation::SkippedPending { test, commit });
            }
        }
    }

    EvalResult {
        violations,
        warnings,
        updated: transition_outcome.updated,
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
    let identity = resolve_identities(status, results);
    let transition_outcome = apply_transitions(&identity.status, &identity.results);

    let violations = transition_outcome
        .violations
        .into_iter()
        .map(|violation| match violation {
            TransitionViolation::NewTestPassed { test } => RatchetViolation::NewTestPassed { test },
            TransitionViolation::Regression { test } => RatchetViolation::Regression { test },
            TransitionViolation::TestDisappeared { test } => {
                RatchetViolation::TestDisappeared { test }
            }
        })
        .collect();

    RatchetOutcome {
        violations,
        updated: transition_outcome.updated,
    }
}

fn resolve_identities(status: &StatusFile, results: &[TestResult]) -> IdentityResolution {
    let mut updated_status = status.clone();
    let mut result_name_map: BTreeMap<String, String> = BTreeMap::new();
    let result_names = observed_test_names(results);
    let mut violations = Vec::new();
    let mut warnings = Vec::new();
    let mut old_name_sources = BTreeMap::<String, Vec<String>>::new();

    for (new_name, old_name) in &status.renames {
        old_name_sources
            .entry(old_name.clone())
            .or_default()
            .push(new_name.clone());
    }

    for (old_name, new_names) in old_name_sources {
        if new_names.len() > 1 {
            violations.push(Violation::RenameOldNameMappedMultipleTimes { old_name });
        }
    }

    for (new_name, old_name) in &status.renames {
        let old_in_status = updated_status.tests.contains_key(old_name);
        let new_in_status = updated_status.tests.contains_key(new_name);
        let old_in_results = result_names.contains(old_name.as_str());
        let new_in_results = result_names.contains(new_name.as_str());

        if !old_in_status {
            if new_in_status && !old_in_results {
                warnings.push(Warning::StaleRename {
                    new_name: new_name.clone(),
                    old_name: old_name.clone(),
                });
            } else {
                violations.push(Violation::RenameOldNameMissing {
                    new_name: new_name.clone(),
                    old_name: old_name.clone(),
                });
            }
            continue;
        }

        if new_in_status {
            violations.push(Violation::RenameNewNameAlreadyTracked {
                new_name: new_name.clone(),
                old_name: old_name.clone(),
            });
            continue;
        }

        if !new_in_results {
            violations.push(Violation::RenameNewNameMissing {
                new_name: new_name.clone(),
                old_name: old_name.clone(),
            });
            continue;
        }

        if old_in_results {
            violations.push(Violation::RenameOldNameStillPresent {
                new_name: new_name.clone(),
                old_name: old_name.clone(),
            });
            continue;
        }

        let entry = updated_status
            .tests
            .remove(old_name)
            .expect("validated old name should exist in status");
        updated_status.tests.insert(new_name.clone(), entry);
        result_name_map.insert(old_name.clone(), new_name.clone());
        warnings.push(Warning::RenameApplied {
            new_name: new_name.clone(),
            old_name: old_name.clone(),
        });
    }

    let rewritten_results = results
        .iter()
        .map(|result| TestResult {
            name: result_name_map
                .get(&result.name)
                .cloned()
                .unwrap_or_else(|| result.name.clone()),
            outcome: result.outcome,
        })
        .collect();

    IdentityResolution {
        status: updated_status,
        results: rewritten_results,
        violations,
        warnings,
    }
}

fn observed_test_names(results: &[TestResult]) -> BTreeSet<&str> {
    results.iter().map(|result| result.name.as_str()).collect()
}

fn tracked_test_state(status: &StatusFile, test_name: &str) -> Option<TestState> {
    status.tests.get(test_name).map(|entry| entry.state())
}

fn missing_tracked_tests<'a>(
    status: &'a StatusFile,
    seen_names: &BTreeSet<&str>,
) -> impl Iterator<Item = &'a String> {
    status
        .tests
        .keys()
        .filter(move |name| !seen_names.contains(name.as_str()))
}

fn map_transition_violation(violation: TransitionViolation) -> Violation {
    match violation {
        TransitionViolation::NewTestPassed { test } => Violation::NewTestPassed { test },
        TransitionViolation::Regression { test } => Violation::Regression { test },
        TransitionViolation::TestDisappeared { test } => Violation::TestDisappeared { test },
    }
}

fn apply_transitions(status: &StatusFile, results: &[TestResult]) -> TransitionOutcome {
    let mut violations = Vec::new();
    let mut updated = status.clone();

    let seen_names = observed_test_names(results);

    for result in results {
        match (tracked_test_state(status, &result.name), result.outcome) {
            (None, TestOutcome::Failed) => {
                updated.set_test_state(result.name.clone(), TestState::Pending);
            }
            (None, TestOutcome::Passed) => {
                if result.name.ends_with(GATEKEEPER_TEST_NAME) {
                    updated.set_test_state(result.name.clone(), TestState::Passing);
                } else {
                    violations.push(TransitionViolation::NewTestPassed {
                        test: result.name.clone(),
                    });
                }
            }
            (None, TestOutcome::Ignored) => {}
            (Some(TestState::Pending), TestOutcome::Failed) => {}
            (Some(TestState::Pending), TestOutcome::Passed) => {
                updated.set_test_state(result.name.clone(), TestState::Passing);
            }
            (Some(TestState::Pending), TestOutcome::Ignored) => {}
            (Some(TestState::Passing), TestOutcome::Passed) => {}
            (Some(TestState::Passing), TestOutcome::Failed) => {
                violations.push(TransitionViolation::Regression {
                    test: result.name.clone(),
                });
            }
            (Some(TestState::Passing), TestOutcome::Ignored) => {}
        }
    }

    violations.extend(
        missing_tracked_tests(status, &seen_names)
            .map(|test| TransitionViolation::TestDisappeared { test: test.clone() }),
    );

    TransitionOutcome {
        violations,
        updated,
    }
}
