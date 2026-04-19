// Report formatting: produces the complete tdd-ratchet output after a run.

use crate::ratchet::{EvalResult, GATEKEEPER_TEST_NAME, Violation, Warning};
use crate::status::TestState;

const SEPARATOR: &str = "───────────────────────────────────────────────────────────────";

struct ReportSection {
    title: String,
    why: String,
    problem: String,
    fix: String,
    details: Vec<String>,
    extra: Option<String>,
}

/// Format the complete report for a ratchet evaluation.
///
/// Takes the full eval result and produces all output. This is the single
/// function that owns all output formatting.
pub fn format_report(result: &EvalResult) -> String {
    let mut tdd_violations: Vec<&Violation> = Vec::new();
    let mut regressions: Vec<&Violation> = Vec::new();
    let mut disappeared: Vec<&Violation> = Vec::new();
    let mut rename_violations: Vec<&Violation> = Vec::new();
    let mut removal_violations: Vec<&Violation> = Vec::new();
    let mut missing_gatekeeper = false;

    for v in &result.violations {
        match v {
            Violation::NewTestPassed { .. } | Violation::SkippedPending { .. } => {
                tdd_violations.push(v);
            }
            Violation::Regression { .. } => {
                regressions.push(v);
            }
            Violation::TestDisappeared { .. } => {
                disappeared.push(v);
            }
            Violation::RenameOldNameMissing { .. }
            | Violation::RenameNewNameMissing { .. }
            | Violation::RenameOldNameStillPresent { .. }
            | Violation::RenameNewNameAlreadyTracked { .. }
            | Violation::RenameOldNameMappedMultipleTimes { .. } => {
                rename_violations.push(v);
            }
            Violation::RemovalMissingTrackedTest { .. }
            | Violation::RemovalTestStillPresent { .. }
            | Violation::RemovalConflictsWithRename { .. } => {
                removal_violations.push(v);
            }
            Violation::MissingGatekeeper => {
                missing_gatekeeper = true;
            }
        }
    }

    let passing_count = result
        .updated
        .tests
        .values()
        .filter(|s| s.state() == TestState::Passing)
        .count();

    let pending: Vec<&String> = result
        .updated
        .tests
        .iter()
        .filter(|(_, s)| s.state() == TestState::Pending)
        .map(|(name, _)| name)
        .collect();

    let has_any_violation = !result.violations.is_empty();

    let mut out = String::new();

    if !tdd_violations.is_empty() {
        out.push_str(&render_section(format_tdd_violations(&tdd_violations)));
    }

    if !disappeared.is_empty() {
        out.push_str(&render_section(format_disappeared_tests(&disappeared)));
    }

    if !rename_violations.is_empty() {
        out.push_str(&render_section(format_rename_violations(
            &rename_violations,
        )));
    }

    if !removal_violations.is_empty() {
        out.push_str(&render_section(format_removal_violations(
            &removal_violations,
        )));
    }

    if missing_gatekeeper {
        out.push_str(&render_section(format_missing_gatekeeper()));
    }

    if !regressions.is_empty() {
        out.push_str(&render_section(format_regressions(&regressions)));
    }

    if !result.warnings.is_empty() {
        out.push_str(&format_warnings(&result.warnings));
    }

    // Success line — only when no violations at all
    if !has_any_violation {
        if pending.is_empty() {
            out.push_str(&format!("tdd-ratchet: ok ({passing_count} passing)\n"));
        } else {
            out.push_str(&format!(
                "tdd-ratchet: ok ({passing_count} passing, {} pending)\n",
                pending.len()
            ));
            for name in &pending {
                out.push_str(&format!("  ○ {name}\n"));
            }
        }
    }

    out
}

fn detail_line(message: impl Into<String>) -> String {
    format!("    ✗ {}\n", message.into())
}

fn warning_line(message: impl Into<String>) -> String {
    format!("    ! {}\n", message.into())
}

fn render_section(section: ReportSection) -> String {
    let mut out = String::new();
    out.push_str(SEPARATOR);
    out.push('\n');
    out.push_str(&format!("tdd-ratchet: {}\n", section.title));
    out.push('\n');
    out.push_str(&format!("  Why: {}\n", section.why));
    out.push_str(&format!("  Problem: {}\n", section.problem));
    out.push_str(&format!("  What to do: {}\n", section.fix));

    if !section.details.is_empty() {
        out.push('\n');
        for detail in section.details {
            out.push_str(&detail);
        }
    }

    if let Some(extra) = section.extra {
        out.push('\n');
        out.push_str(&extra);
        if !extra.ends_with('\n') {
            out.push('\n');
        }
    }

    out.push_str(SEPARATOR);
    out.push('\n');
    out
}

fn story_14_why(specific_context: &str) -> String {
    format!("This project uses tdd-ratchet to enforce test-first discipline. {specific_context}")
}

fn format_tdd_violations(violations: &[&Violation]) -> ReportSection {
    let mut details = Vec::new();

    for violation in violations {
        match violation {
            Violation::NewTestPassed { test } => {
                details.push(detail_line(format!(
                    "New test passed without failing first: {test}"
                )));
            }
            Violation::SkippedPending { test, commit } => {
                let short = &commit[..8.min(commit.len())];
                details.push(detail_line(format!(
                    "Test skipped the pending state in git history: {test} (commit {short})"
                )));
            }
            _ => unreachable!(),
        }
    }

    ReportSection {
        title: "strict TDD violation".into(),
        why: story_14_why(
            "It checks git history because a test must fail before it is allowed to pass, so the test describes the desired behavior before the implementation exists.",
        ),
        problem: "One or more tests violated the failing-first rule: tdd-ratchet could not find a commit where the test was failing before a later commit made it pass.".into(),
        fix: "Always commit `.test-status.json` whenever tdd-ratchet changes it. Write the failing test, run `cargo ratchet`, and commit the test code together with `.test-status.json` showing that test as `pending`. Then write the implementation, run `cargo ratchet` again, and commit the implementation together with `.test-status.json` showing that test as `passing`. If history is already wrong, rebase so the commits follow that sequence.".into(),
        details,
        extra: None,
    }
}

fn format_disappeared_tests(violations: &[&Violation]) -> ReportSection {
    let count = violations.len();
    let test_word = if count == 1 { "test is" } else { "tests are" };
    let details = violations
        .iter()
        .map(|violation| match violation {
            Violation::TestDisappeared { test } => {
                detail_line(format!("Tracked test missing from the run: {test}"))
            }
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "tracked test missing from run".into(),
        why: story_14_why(
            "It relies on `.test-status.json` as the committed record of which tests define the project's expected behavior, so missing tests could hide deleted coverage or an undeclared rename.",
        ),
        problem: format!("{count} tracked {test_word} listed in `.test-status.json` but missing from the current test run."),
        fix: "Check whether the test was accidentally deleted, skipped, or renamed. If you removed it intentionally, add its tracked name to the working-tree `removals` list in `.test-status.json`, run `cargo ratchet`, and commit the removal together with the updated `.test-status.json`. If it was renamed, add a valid `renames` entry so tdd-ratchet can bridge the committed old name to the observed new name, then commit the rename together with the `.test-status.json` update. Otherwise restore the missing test so the committed behavior is still exercised.".into(),
        details,
        extra: None,
    }
}

fn format_rename_violations(rename_violations: &[&Violation]) -> ReportSection {
    let details = rename_violations
        .iter()
        .map(|violation| match violation {
            Violation::RenameOldNameMissing { new_name, old_name } => detail_line(format!(
                "{new_name} -> {old_name}: old name is not present in committed status"
            )),
            Violation::RenameNewNameMissing { new_name, old_name } => detail_line(format!(
                "{new_name} -> {old_name}: new name was not found in the current test run"
            )),
            Violation::RenameOldNameStillPresent { new_name, old_name } => detail_line(format!(
                "{new_name} -> {old_name}: old name still appears in the current test run"
            )),
            Violation::RenameNewNameAlreadyTracked { new_name, old_name } => detail_line(format!(
                "{new_name} -> {old_name}: new name is already tracked independently"
            )),
            Violation::RenameOldNameMappedMultipleTimes { old_name } => detail_line(format!(
                "{old_name}: multiple rename entries point at the same old name"
            )),
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "invalid test rename declaration".into(),
        why: story_14_why(
            "When a test is renamed, it needs a valid identity bridge so the existing test history is preserved instead of looking like one test disappeared and a different one appeared.",
        ),
        problem: "A rename instruction is invalid, so tdd-ratchet cannot safely connect the committed test history to the currently observed test name.".into(),
        fix: "To fix it, correct the `renames` entry so it bridges one committed old name to one observed new name, remove any stale or conflicting mappings, and commit the rename together with the `.test-status.json` update.".into(),
        details,
        extra: None,
    }
}

fn format_removal_violations(removal_violations: &[&Violation]) -> ReportSection {
    let details = removal_violations
        .iter()
        .map(|violation| match violation {
            Violation::RemovalMissingTrackedTest { test } => detail_line(format!(
                "{test}: removal target is not present in committed status"
            )),
            Violation::RemovalTestStillPresent { test } => detail_line(format!(
                "{test}: removal target still appears in the current test run"
            )),
            Violation::RemovalConflictsWithRename { test } => detail_line(format!(
                "{test}: removal target also participates in a `renames` entry"
            )),
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "invalid test removal declaration".into(),
        why: story_14_why(
            "Intentional test retirement must be explicit, because silently dropping a tracked test would weaken the suite without recording that decision.",
        ),
        problem: "A `removals` instruction is invalid, so tdd-ratchet cannot safely retire the tracked test from the committed behavior set.".into(),
        fix: "Use `removals` only for tests that are currently tracked in committed status, are absent from the current test run, and are not also involved in a rename. Then run `cargo ratchet` and commit the test removal together with the updated `.test-status.json`.".into(),
        details,
        extra: None,
    }
}

fn format_missing_gatekeeper() -> ReportSection {
    ReportSection {
        title: "missing gatekeeper test".into(),
        why: story_14_why(
            "It only works when tests are run through the ratchet, and without it, someone can run `cargo test` directly and bypass the ratchet.",
        ),
        problem: format!("no test named `{GATEKEEPER_TEST_NAME}` was found in the current run."),
        fix: "To fix it, add the gatekeeper test below so direct `cargo test` runs fail with instructions and ratchet runs can set `TDD_RATCHET=1`.".into(),
        details: Vec::new(),
        extra: Some(format!(
            "    #[test]\n\
             \x20\x20\x20\x20fn {GATEKEEPER_TEST_NAME}() {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20if std::env::var(\"TDD_RATCHET\").is_err() {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20panic!(\"Run tdd-ratchet instead of cargo test.\");\n\
             \x20\x20\x20\x20\x20\x20\x20\x20}}\n\
             \x20\x20\x20\x20}}\n"
        )),
    }
}

fn format_regressions(violations: &[&Violation]) -> ReportSection {
    let count = violations.len();
    let test_word = if count == 1 { "test is" } else { "tests are" };
    let details = violations
        .iter()
        .map(|violation| match violation {
            Violation::Regression { test } => {
                detail_line(format!("Previously passing test now fails: {test}"))
            }
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "regression detected".into(),
        why: story_14_why(
            "Once a test is accepted as passing, later failures mean the protected behavior regressed and the suite is no longer keeping that promise.",
        ),
        problem: format!("{count} tracked passing {test_word} was previously tracked as passing but is now failing in the current run."),
        fix: "Fix the failing test, or if the change is intentional, run `cargo ratchet` and commit the code change together with the updated `.test-status.json`. Always commit `.test-status.json` whenever tdd-ratchet changes it.".into(),
        details,
        extra: None,
    }
}

fn format_warnings(warnings: &[Warning]) -> String {
    render_section(ReportSection {
        title: if warnings.len() == 1 {
            "rename warning".into()
        } else {
            "rename warnings".into()
        },
        why: story_14_why(
            "Temporary rename mappings are only meant to bridge one rename commit, so the report also teaches you when that temporary bookkeeping can be removed.",
        ),
        problem: if warnings.len() == 1 {
            "A temporary rename mapping no longer needs to stay in `.test-status.json`.".into()
        } else {
            "Temporary rename mappings no longer need to stay in `.test-status.json`.".into()
        },
        fix: "Remove the `renames` entry in your next commit once the rename bridge is no longer needed.".into(),
        details: warnings.iter().map(format_warning).collect(),
        extra: None,
    })
}

fn format_warning(warning: &Warning) -> String {
    match warning {
        Warning::RenameApplied { new_name, old_name } => warning_line(format!(
            "{new_name} renamed from {old_name}; the temporary `renames` entry has done its job and can now be removed"
        )),
        Warning::StaleRename { new_name, old_name } => warning_line(format!(
            "{new_name} -> {old_name} is stale; the temporary `renames` entry can be removed"
        )),
    }
}
