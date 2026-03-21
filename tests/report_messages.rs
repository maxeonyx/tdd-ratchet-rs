use tdd_ratchet::errors::format_report;
use tdd_ratchet::ratchet::{EvalResult, Violation, Warning};
use tdd_ratchet::status::{StatusFile, TestState};

const WHY_PREFIX: &str = "This project uses tdd-ratchet to enforce test-first discipline.";

fn report(violations: Vec<Violation>, warnings: Vec<Warning>) -> String {
    let mut updated = StatusFile::empty();
    updated.set_test_state("suite::passing_test", TestState::Passing);

    format_report(&EvalResult {
        violations,
        warnings,
        updated,
    })
}

fn report_with_violations(violations: Vec<Violation>) -> String {
    report(violations, Vec::new())
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

fn assert_contains_all(report: &str, expected: &[&str]) {
    for snippet in expected {
        assert!(
            report.contains(snippet),
            "report should contain `{snippet}`: {report}"
        );
    }
}

#[test]
fn new_test_passed_report_uses_common_explanatory_fields() {
    let report = report_with_violations(vec![Violation::NewTestPassed {
        test: "suite::new_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "suite::new_test",
            "must fail before it is allowed to pass",
            "Rebase your branch so the failing test is committed before the implementation that makes it pass.",
        ],
    );
}

#[test]
fn regression_report_names_the_regressed_tests_and_explains_the_fix() {
    let report = report_with_violations(vec![Violation::Regression {
        test: "suite::fragile_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "suite::fragile_test",
            "was previously tracked as passing",
            "Fix the failing test, or if the change is intentional, update `.test-status.json` to match the new reality.",
        ],
    );
}

#[test]
fn disappeared_test_report_explains_the_rule_and_cleanup() {
    let report = report_with_violations(vec![Violation::TestDisappeared {
        test: "suite::removed_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "suite::removed_test",
            "listed in `.test-status.json` but missing from the current test run",
            "If you removed it intentionally, also remove it from `.test-status.json`.",
        ],
    );
}

#[test]
fn rename_violation_report_explains_identity_bridge_requirements() {
    let report = report_with_violations(vec![Violation::RenameNewNameMissing {
        new_name: "suite::new_name".into(),
        old_name: "suite::old_name".into(),
    }]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "suite::new_name",
            "suite::old_name",
            "rename instruction is invalid",
            "correct the `renames` entry so it bridges one committed old name to one observed new name",
        ],
    );
}

#[test]
fn missing_gatekeeper_report_explains_bypass_prevention() {
    let report = report_with_violations(vec![Violation::MissingGatekeeper]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "`tdd_ratchet_gatekeeper`",
            "without it, someone can run `cargo test` directly and bypass the ratchet",
            "add the gatekeeper test below",
        ],
    );
}

#[test]
fn rename_warning_report_is_also_self_documenting() {
    let report = report(
        Vec::new(),
        vec![Warning::RenameApplied {
            new_name: "suite::new_name".into(),
            old_name: "suite::old_name".into(),
        }],
    );

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "rename warning",
            "suite::new_name",
            "suite::old_name",
            "the temporary `renames` entry has done its job",
            "Remove the `renames` entry in your next commit",
        ],
    );
}
