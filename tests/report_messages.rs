use tdd_ratchet::errors::format_report;
use tdd_ratchet::ratchet::{EvalResult, Violation};
use tdd_ratchet::status::{StatusFile, TestState};

fn report_with_violations(violations: Vec<Violation>) -> String {
    let mut updated = StatusFile::empty();
    updated.set_test_state("suite::passing_test", TestState::Passing);

    format_report(&EvalResult {
        violations,
        warnings: Vec::new(),
        updated,
    })
}

#[test]
fn new_test_passed_report_uses_common_explanatory_fields() {
    let report = report_with_violations(vec![Violation::NewTestPassed {
        test: "suite::new_test".into(),
    }]);

    assert!(
        report.contains("Why:"),
        "report should explain why the ratchet exists: {report}"
    );
    assert!(
        report.contains("Problem:"),
        "report should identify the specific violation: {report}"
    );
    assert!(
        report.contains("What to do:"),
        "report should tell the user how to fix it: {report}"
    );
    assert!(
        report.contains("suite::new_test"),
        "report should name the violating test: {report}"
    );
}

#[test]
fn regression_report_names_the_regressed_tests_and_explains_the_fix() {
    let report = report_with_violations(vec![Violation::Regression {
        test: "suite::fragile_test".into(),
    }]);

    assert!(
        report.contains("suite::fragile_test"),
        "report should name the regressed test: {report}"
    );
    assert!(
        report.contains("Why:"),
        "report should explain why regressions matter: {report}"
    );
    assert!(
        report.contains("Problem:"),
        "report should describe the regression: {report}"
    );
    assert!(
        report.contains("What to do:"),
        "report should give recovery guidance: {report}"
    );
}
