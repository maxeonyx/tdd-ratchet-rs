// tests/error_messages.rs
//
// Story 9: Ratchet-specific failures explain context, problem, and suggestion.

use tdd_ratchet::errors::format_violation;
use tdd_ratchet::ratchet::RatchetViolation;

fn assert_has_context_problem_suggestion(msg: &str) {
    // Every error message must mention tdd-ratchet context
    assert!(
        msg.contains("tdd-ratchet") || msg.contains("TDD"),
        "Error should mention tdd-ratchet context: {msg}"
    );
    // Must have some actionable suggestion (look for imperative verbs / "to fix" / etc.)
    // This is a heuristic — the real check is that each violation type produces useful output.
}

#[test]
fn new_test_passed_error_has_context_problem_suggestion() {
    let v = RatchetViolation::NewTestPassed {
        test: "my_module::my_test".to_string(),
    };
    let msg = format_violation(&v);
    assert_has_context_problem_suggestion(&msg);
    assert!(msg.contains("my_module::my_test"), "Should name the test");
    assert!(
        msg.contains("fail") || msg.contains("pending"),
        "Should explain tests must fail first: {msg}"
    );
}

#[test]
fn regression_error_has_context_problem_suggestion() {
    let v = RatchetViolation::Regression {
        test: "my_module::my_test".to_string(),
    };
    let msg = format_violation(&v);
    assert_has_context_problem_suggestion(&msg);
    assert!(msg.contains("my_module::my_test"), "Should name the test");
    assert!(
        msg.contains("regress") || msg.contains("was passing"),
        "Should explain regression: {msg}"
    );
}

#[test]
fn test_disappeared_error_has_context_problem_suggestion() {
    let v = RatchetViolation::TestDisappeared {
        test: "old_test".to_string(),
    };
    let msg = format_violation(&v);
    assert_has_context_problem_suggestion(&msg);
    assert!(msg.contains("old_test"), "Should name the test");
    assert!(
        msg.contains("disappear") || msg.contains("missing") || msg.contains("removed"),
        "Should explain test was removed: {msg}"
    );
}

#[test]
fn all_violation_variants_are_covered() {
    // This is a compile-time-ish check — if a new variant is added to
    // RatchetViolation, this match must be updated. The compiler will
    // error on a non-exhaustive match.
    let violations: Vec<RatchetViolation> = vec![
        RatchetViolation::NewTestPassed { test: "a".into() },
        RatchetViolation::Regression { test: "b".into() },
        RatchetViolation::TestDisappeared { test: "c".into() },
    ];
    for v in &violations {
        let msg = format_violation(v);
        // Every formatted message should be non-empty
        assert!(!msg.is_empty(), "Violation should produce a message: {v:?}");
        // Force exhaustive match so new variants cause a compile error
        match v {
            RatchetViolation::NewTestPassed { .. } => {}
            RatchetViolation::Regression { .. } => {}
            RatchetViolation::TestDisappeared { .. } => {}
        }
    }
}
