use tdd_ratchet::errors::format_report;
use tdd_ratchet::ratchet::{EvalResult, Violation};
use tdd_ratchet::status::{StatusFile, TestState};

const WHY_PREFIX: &str = "This project uses tdd-ratchet to enforce test-first discipline.";

fn report_with_violations(violations: Vec<Violation>) -> String {
    let mut updated = StatusFile::empty();
    updated.set_test_state("suite::passing_test", TestState::Passing);

    format_report(&EvalResult {
        violations,
        warnings: Vec::new(),
        updated,
    })
}

fn assert_story_14_fields(report: &str) {
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
        report.contains(WHY_PREFIX),
        "report should explain the test-first discipline context: {report}"
    );
}

#[test]
fn new_test_passed_report_uses_common_explanatory_fields() {
    let report = report_with_violations(vec![Violation::NewTestPassed {
        test: "suite::new_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert!(
        report.contains("suite::new_test"),
        "report should name the violating test: {report}"
    );
    assert!(
        report.contains("must fail before it is allowed to pass"),
        "report should explain the failing-first rule: {report}"
    );
    assert!(
        report.contains("Rebase your branch so the failing test is committed before the implementation that makes it pass."),
        "report should give explicit rebase guidance: {report}"
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
    assert_story_14_fields(&report);
    assert!(
        report.contains("was previously tracked as passing"),
        "report should explain why the regression matters: {report}"
    );
    assert!(
        report.contains("Fix the failing test, or if the change is intentional, update `.test-status.json` to match the new reality."),
        "report should give explicit regression guidance: {report}"
    );
}

#[test]
fn disappeared_test_report_explains_the_rule_and_cleanup() {
    let report = report_with_violations(vec![Violation::TestDisappeared {
        test: "suite::removed_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert!(
        report.contains("suite::removed_test"),
        "report should name the missing test: {report}"
    );
    assert!(
        report.contains("tracked in `.test-status.json` but missing from the current test run"),
        "report should explain the missing-test rule: {report}"
    );
    assert!(
        report
            .contains("If you removed it intentionally, also remove it from `.test-status.json`."),
        "report should explain the cleanup step: {report}"
    );
}

#[test]
fn rename_violation_report_explains_identity_bridge_requirements() {
    let report = report_with_violations(vec![Violation::RenameNewNameMissing {
        new_name: "suite::new_name".into(),
        old_name: "suite::old_name".into(),
    }]);

    assert_story_14_fields(&report);
    assert!(
        report.contains("suite::new_name") && report.contains("suite::old_name"),
        "report should name both ends of the rename: {report}"
    );
    assert!(
        report.contains("rename instruction is invalid"),
        "report should explain the rename rule: {report}"
    );
    assert!(
        report.contains("correct the `renames` entry so it bridges one committed old name to one observed new name"),
        "report should explain how to repair the rename mapping: {report}"
    );
}

#[test]
fn missing_gatekeeper_report_explains_bypass_prevention() {
    let report = report_with_violations(vec![Violation::MissingGatekeeper]);

    assert_story_14_fields(&report);
    assert!(
        report.contains("`tdd_ratchet_gatekeeper`"),
        "report should name the required gatekeeper test: {report}"
    );
    assert!(
        report.contains("without it, someone can run `cargo test` directly and bypass the ratchet"),
        "report should explain why the gatekeeper matters: {report}"
    );
    assert!(
        report.contains("add the gatekeeper test below"),
        "report should tell the user to add the gatekeeper snippet: {report}"
    );
}
