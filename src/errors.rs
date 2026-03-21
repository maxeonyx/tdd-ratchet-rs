// Report formatting: produces the complete tdd-ratchet output after a run.

use crate::ratchet::{EvalResult, Violation, Warning, GATEKEEPER_TEST_NAME};
use crate::status::TestState;

const SEPARATOR: &str = "───────────────────────────────────────────────────────────────";

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

    // TDD violations section (NewTestPassed + SkippedPending)
    if !tdd_violations.is_empty() {
        let mut new_test_passed: Vec<&str> = Vec::new();
        let mut skipped_pending: Vec<(&str, &str)> = Vec::new();

        for v in &tdd_violations {
            match v {
                Violation::NewTestPassed { test } => {
                    new_test_passed.push(test);
                }
                Violation::SkippedPending { test, commit } => {
                    skipped_pending.push((test, commit));
                }
                _ => unreachable!(),
            }
        }

        out.push_str(SEPARATOR);
        out.push('\n');
        out.push_str(
            "tdd-ratchet: this project uses tdd-ratchet to enforce strict TDD.\n\
             \n\
             \x20\x20New tests must be committed in a failing state first. The implementation\n\
             \x20\x20that makes them pass must be in a separate commit. Tests that fail on\n\
             \x20\x20creation are expected — tdd-ratchet considers that a successful run.\n",
        );

        if !new_test_passed.is_empty() {
            out.push('\n');
            out.push_str("  New test passed without failing first:\n");
            for test in &new_test_passed {
                out.push_str(&format!("    ✗ {test}\n"));
            }
        }

        if !skipped_pending.is_empty() {
            out.push('\n');
            out.push_str("  Test skipped the pending state in git history:\n");
            for (test, commit) in &skipped_pending {
                let short = &commit[..8.min(commit.len())];
                out.push_str(&format!("    ✗ {test} (commit {short})\n"));
            }
        }

        out.push_str(SEPARATOR);
        out.push('\n');
    }

    // Disappeared tests section
    if !disappeared.is_empty() {
        let count = disappeared.len();
        let plural = if count == 1 { "was" } else { "were" };
        out.push_str(SEPARATOR);
        out.push('\n');
        out.push_str(&format!(
            "tdd-ratchet: {count} test in .test-status.json {plural} not found in the test run.\n\
             \x20\x20If you removed it intentionally, also remove it from .test-status.json.\n"
        ));
        for v in &disappeared {
            if let Violation::TestDisappeared { test } = v {
                out.push_str(&format!("    ✗ {test}\n"));
            }
        }
        out.push_str(SEPARATOR);
        out.push('\n');
    }

    if !rename_violations.is_empty() {
        out.push_str(&format_rename_violations(&rename_violations));
    }

    // Missing gatekeeper section
    if missing_gatekeeper {
        out.push_str(SEPARATOR);
        out.push('\n');
        out.push_str(&format!(
            "tdd-ratchet: no gatekeeper test found.\n\
             \n\
             \x20\x20tdd-ratchet requires a test named `{GATEKEEPER_TEST_NAME}` that fails\n\
             \x20\x20when TDD_RATCHET is not set. This prevents running tests outside the\n\
             \x20\x20ratchet. Add this to your tests:\n\
             \n\
             \x20\x20\x20\x20#[test]\n\
             \x20\x20\x20\x20fn {GATEKEEPER_TEST_NAME}() {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20if std::env::var(\"TDD_RATCHET\").is_err() {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20panic!(\"Run tdd-ratchet instead of cargo test.\");\n\
             \x20\x20\x20\x20\x20\x20\x20\x20}}\n\
             \x20\x20\x20\x20}}\n"
        ));
        out.push_str(SEPARATOR);
        out.push('\n');
    }

    // Regressions — one-line mention, nextest already showed details
    if !regressions.is_empty() {
        let count = regressions.len();
        let plural = if count == 1 { "" } else { "s" };
        out.push_str(&format!(
            "tdd-ratchet: {count} test{plural} failing unexpectedly\n"
        ));
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

fn format_rename_violations(rename_violations: &[&Violation]) -> String {
    let mut out = String::new();
    out.push_str(SEPARATOR);
    out.push('\n');
    out.push_str("tdd-ratchet: invalid test rename declaration in .test-status.json.\n");
    out.push_str("  Rename mappings must bridge one old committed test name to one new observed test name.\n");
    for violation in rename_violations {
        out.push_str(&format_rename_violation(violation));
    }
    out.push_str(SEPARATOR);
    out.push('\n');
    out
}

fn format_rename_violation(violation: &Violation) -> String {
    match violation {
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
