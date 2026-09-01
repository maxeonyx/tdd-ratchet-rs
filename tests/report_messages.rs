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
            "Do not hand-edit `.test-status.json`; the trusted ledger workflow writes it.",
            "commit and push the test code",
            "wait for the workflow's bot commit to record it as `pending`",
            "push the implementation so the workflow can record `passing`",
            "If history is already wrong, rebase so the commits follow that sequence.",
        ],
    );
}

#[test]
fn already_working_behavior_guidance_suggests_temporarily_breaking_the_implementation() {
    let report = report_with_violations(vec![Violation::NewTestPassed {
        test: "suite::regression_test_for_existing_behavior".into(),
    }]);

    assert_contains_all(
        &report,
        &[
            "If the behavior already works",
            "temporarily break the implementation",
            "push the test while it is failing",
            "wait for the `pending` bot commit",
            "restore the implementation",
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
            "Restore the passing behavior and rerun `cargo ratchet`.",
            "use a rename or removal instruction in `.tdd-ratchet.json`",
        ],
    );
}

#[test]
fn disappeared_test_report_explains_the_rule_and_removals_workflow() {
    let report = report_with_violations(vec![Violation::TestDisappeared {
        test: "suite::removed_test".into(),
    }]);

    assert_story_14_fields(&report);
    assert_contains_all(
        &report,
        &[
            "suite::removed_test",
            "listed in `.test-status.json` but missing from the current test run",
            "`removals` list in `.tdd-ratchet.json`",
            "trusted workflow can update the ledger",
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
            "correct the `renames` entry in `.tdd-ratchet.json`",
            "trusted workflow can update the ledger",
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
            "Remove the `renames` entry from `.tdd-ratchet.json` in your next commit",
        ],
    );
}
