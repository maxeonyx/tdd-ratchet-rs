// Error formatting: each ratchet violation gets context, problem, and suggestion.

use crate::ratchet::RatchetViolation;

/// Format a ratchet violation into a user-facing error message.
///
/// Each message includes:
/// 1. Context — this project uses tdd-ratchet
/// 2. Problem — what specifically went wrong
/// 3. Suggestion — what to do about it
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
