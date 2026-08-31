// Report formatting: produces the complete tdd-ratchet output after a run.

use crate::ratchet::{EvalResult, Violation, Warning, GATEKEEPER_TEST_NAME};
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
                details.push(format!(
                    "    ✗ New test passed without failing first: {test}\n"
                ));
            }
            Violation::SkippedPending { test, commit } => {
                let short = &commit[..8.min(commit.len())];
                details.push(format!(
                    "    ✗ Test skipped the pending state in git history: {test} (commit {short})\n"
                ));
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
        fix: "Rebase your branch so the failing test is committed before the implementation that makes it pass. If a new test already passes, move or rewrite it so an earlier commit shows the test failing for the right reason.".into(),
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
                format!("    ✗ Tracked test missing from the run: {test}\n")
            }
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "tracked test missing from run".into(),
        why: story_14_why(
            "It relies on `.test-status.json` as the committed record of which tests define the project's expected behavior, so missing tests could hide deleted coverage or an undeclared rename.",
        ),
        problem: format!("{count} tracked {test_word} tracked in `.test-status.json` but missing from the current test run."),
        fix: "If you removed it intentionally, also remove it from `.test-status.json`. If you renamed it, add a `renames` entry so tdd-ratchet can bridge the old name to the new one.".into(),
        details,
        extra: None,
    }
}

fn format_rename_violations(rename_violations: &[&Violation]) -> ReportSection {
    let details = rename_violations
        .iter()
        .map(|violation| match violation {
            Violation::RenameOldNameMissing { new_name, old_name } => {
                format!("    ✗ {new_name} -> {old_name}: old name is not present in committed status\n")
            }
            Violation::RenameNewNameMissing { new_name, old_name } => {
                format!(
                    "    ✗ {new_name} -> {old_name}: new name was not found in the current test run\n"
                )
            }
            Violation::RenameOldNameStillPresent { new_name, old_name } => {
                format!(
                    "    ✗ {new_name} -> {old_name}: old name still appears in the current test run\n"
                )
            }
            Violation::RenameNewNameAlreadyTracked { new_name, old_name } => {
                format!("    ✗ {new_name} -> {old_name}: new name is already tracked independently\n")
            }
            Violation::RenameOldNameMappedMultipleTimes { old_name } => {
                format!("    ✗ {old_name}: multiple rename entries point at the same old name\n")
            }
            _ => unreachable!(),
        })
        .collect();

    ReportSection {
        title: "invalid test rename declaration".into(),
        why: story_14_why(
            "When a test is renamed, it needs a valid identity bridge so the existing test history is preserved instead of looking like one test disappeared and a different one appeared.",
        ),
        problem: "A rename instruction is invalid, so tdd-ratchet cannot safely connect the committed test history to the currently observed test name.".into(),
        fix: "To fix it, correct the `renames` entry so it bridges one committed old name to one observed new name, and remove any stale or conflicting mappings.".into(),
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
                format!("    ✗ Previously passing test now fails: {test}\n")
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
        fix: "Fix the failing test, or if the change is intentional, update `.test-status.json` to match the new reality.".into(),
        details,
        extra: None,
    }
}

fn format_warnings(warnings: &[Warning]) -> String {
    let mut out = String::new();
    out.push_str(SEPARATOR);
    out.push('\n');
    out.push_str("tdd-ratchet: rename warnings:\n");
    for warning in warnings {
        out.push_str(&format_warning(warning));
    }
    out.push_str(SEPARATOR);
    out.push('\n');
    out
}

fn format_warning(warning: &Warning) -> String {
    match warning {
        Warning::RenameApplied { new_name, old_name } => {
            format!(
                "    ! {new_name} renamed from {old_name}; the renames entry can now be removed\n"
            )
        }
        Warning::StaleRename { new_name, old_name } => {
            format!("    ! {new_name} -> {old_name} is stale; the renames entry can be removed\n")
        }
    }
}
