// Error formatting: each ratchet violation gets context, problem, and suggestion.

use crate::ratchet::{RatchetViolation, Violation, GATEKEEPER_TEST_NAME};

/// Format a unified violation into a user-facing error message.
pub fn format_eval_violation(violation: &Violation) -> String {
    match violation {
        Violation::NewTestPassed { test } => {
            format!(
                "tdd-ratchet: new test `{test}` passed on first appearance.\n\
                 New tests must fail first (pending state) before they can pass.\n\
                 Write the test so it fails, commit, then implement to make it pass."
            )
        }
        Violation::Regression { test } => {
            format!(
                "tdd-ratchet: test `{test}` was passing but now fails (regression).\n\
                 A test marked as passing must continue to pass.\n\
                 Fix the regression or, if the test is obsolete, remove it from both code and .test-status.json."
            )
        }
        Violation::TestDisappeared { test } => {
            format!(
                "tdd-ratchet: tracked test `{test}` is missing from the test run.\n\
                 A test in .test-status.json disappeared without being removed from the status file.\n\
                 If you removed the test intentionally, also remove it from .test-status.json in the same commit."
            )
        }
        Violation::SkippedPending { test, commit } => {
            let short = &commit[..8.min(commit.len())];
            format!(
                "tdd-ratchet: test `{test}` appeared as passing in commit {short} without a prior pending state.\n\
                 Git history shows this test skipped the TDD workflow (fail first, then pass).\n\
                 Rebase to split the offending commit: add the test as failing in one commit, then make it pass in the next."
            )
        }
        Violation::MissingGatekeeper => {
            format!(
                "tdd-ratchet: no gatekeeper test found.\n\
                 \n\
                 Add a test named `{GATEKEEPER_TEST_NAME}` to your project to prevent\n\
                 `cargo test` from being run directly (bypassing the ratchet).\n\
                 \n\
                 Example (add to tests/gatekeeper.rs):\n\
                 \n\
                 #[test]\n\
                 fn {GATEKEEPER_TEST_NAME}() {{\n\
                     if std::env::var(\"TDD_RATCHET\").is_err() {{\n\
                         panic!(\n\
                             \"This project uses tdd-ratchet for strict TDD.\\n\\\n\
                              Run `tdd-ratchet` instead of `cargo test`.\"\n\
                         );\n\
                     }}\n\
                 }}"
            )
        }
    }
}

// --- Legacy API kept for existing unit tests ---

/// Format a ratchet violation into a user-facing error message.
pub fn format_violation(violation: &RatchetViolation) -> String {
    match violation {
        RatchetViolation::NewTestPassed { test } => {
            format!(
                "tdd-ratchet: new test `{test}` passed on first appearance.\n\
                 New tests must fail first (pending state) before they can pass.\n\
                 Write the test so it fails, commit, then implement to make it pass."
            )
        }
        RatchetViolation::Regression { test } => {
            format!(
                "tdd-ratchet: test `{test}` was passing but now fails (regression).\n\
                 A test marked as passing must continue to pass.\n\
                 Fix the regression or, if the test is obsolete, remove it from both code and .test-status.json."
            )
        }
        RatchetViolation::TestDisappeared { test } => {
            format!(
                "tdd-ratchet: tracked test `{test}` is missing from the test run.\n\
                 A test in .test-status.json disappeared without being removed from the status file.\n\
                 If you removed the test intentionally, also remove it from .test-status.json in the same commit."
            )
        }
    }
}

/// Format a history violation into a user-facing error message.
pub fn format_history_violation(violation: &crate::history::HistoryViolation) -> String {
    match violation {
        crate::history::HistoryViolation::SkippedPending { test, commit } => {
            let short = &commit[..8.min(commit.len())];
            format!(
                "tdd-ratchet: test `{test}` appeared as passing in commit {short} without a prior pending state.\n\
                 Git history shows this test skipped the TDD workflow (fail first, then pass).\n\
                 Rebase to split the offending commit: add the test as failing in one commit, then make it pass in the next."
            )
        }
    }
}
