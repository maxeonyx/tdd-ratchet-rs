use std::env;
use std::path::Path;
use std::process::{self, Command, Stdio};

use tdd_ratchet::errors::format_report;
use tdd_ratchet::history::{collect_history_snapshots, read_head_status};
use tdd_ratchet::ratchet::evaluate;
use tdd_ratchet::runner::{parse_nextest_output, TestOutcome, TestResult};
use tdd_ratchet::status::{StatusFile, TestEntry, TestState};

struct GatheredRun {
    status: StatusFile,
    results: Vec<tdd_ratchet::runner::TestResult>,
    history_snapshots: Vec<tdd_ratchet::history::HistorySnapshot>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let project_dir = env::current_dir().unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: cannot determine current directory: {e}");
        process::exit(1);
    });

    let status_path = project_dir.join(".test-status.json");

    if args.iter().any(|a| a == "--init") {
        init(&status_path, &project_dir);
        return;
    }

    run_ratchet(&project_dir, &status_path);
}

fn init(status_path: &Path, project_dir: &Path) {
    if status_path.exists() {
        eprintln!(
            "tdd-ratchet: .test-status.json already exists. Remove it first to re-initialize."
        );
        process::exit(1);
    }

    let mut status = StatusFile::empty();

    // Run tests and snapshot existing results into the status file
    status.tests = status_entries_from_results(&run_nextest(project_dir, false));

    status.write_to_path(status_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to create status file: {e}");
        process::exit(1);
    });

    let passing = status
        .tests
        .values()
        .filter(|s| s.state() == tdd_ratchet::status::TestState::Passing)
        .count();
    let pending = status
        .tests
        .values()
        .filter(|s| s.state() == tdd_ratchet::status::TestState::Pending)
        .count();
    println!("tdd-ratchet: initialized .test-status.json ({passing} passing, {pending} pending)");
}

fn run_ratchet(project_dir: &Path, status_path: &Path) {
    let gathered = gather_run(project_dir);

    // ── Phase 2: Evaluate (pure) ────────────────────────────────────
    let result = evaluate(
        &gathered.status,
        &gathered.results,
        &gathered.history_snapshots,
    );

    // ── Phase 3: Output ─────────────────────────────────────────────
    // Always save the updated status file — valid transitions (new
    // pending tests, promotions) should persist even when there are
    // violations. This prevents losing state on partial runs.
    result
        .updated
        .write_to_path(status_path)
        .unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to save status file: {e}");
            process::exit(1);
        });

    let has_violations = !result.violations.is_empty();
    let report = format_report(&result);
    eprint!("\n{report}");

    if has_violations {
        process::exit(1);
    }
}

fn gather_run(project_dir: &Path) -> GatheredRun {
    let status = load_status_input(project_dir);
    let results = run_nextest(project_dir, true);
    let history_snapshots = collect_history_snapshots(project_dir).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to inspect git history: {e}");
        process::exit(1);
    });

    GatheredRun {
        status,
        results,
        history_snapshots,
    }
}

fn load_status_input(project_dir: &Path) -> StatusFile {
    read_head_status(project_dir)
        .unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to read committed status file: {e}");
            process::exit(1);
        })
        .unwrap_or_else(StatusFile::empty)
}

fn status_entries_from_results(
    results: &[TestResult],
) -> std::collections::BTreeMap<String, TestEntry> {
    results
        .iter()
        .filter_map(|result| match result.outcome {
            TestOutcome::Passed => {
                Some((result.name.clone(), TestEntry::Simple(TestState::Passing)))
            }
            TestOutcome::Failed => {
                Some((result.name.clone(), TestEntry::Simple(TestState::Pending)))
            }
            TestOutcome::Ignored => None,
        })
        .collect()
}

fn run_nextest(project_dir: &Path, inherit_stderr: bool) -> Vec<TestResult> {
    let mut command = Command::new("cargo");
    command
        .args([
            "nextest",
            "run",
            "--no-fail-fast",
            "--message-format",
            "libtest-json",
        ])
        .current_dir(project_dir)
        .env("TDD_RATCHET", "1")
        .env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1");

    if inherit_stderr {
        command.stderr(Stdio::inherit());
    }

    let output = command.output().unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to run cargo nextest: {e}");
        process::exit(1);
    });

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_nextest_output(&stdout)
}
