use std::env;
use std::path::Path;
use std::process::{self, Command, Stdio};

use tdd_ratchet::errors::format_report;
use tdd_ratchet::history::collect_history_snapshots;
use tdd_ratchet::ratchet::evaluate;
use tdd_ratchet::runner::parse_nextest_output;
use tdd_ratchet::status::StatusFile;

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

    let baseline = get_head_commit(project_dir);

    let mut status = StatusFile::empty();
    status.baseline = baseline;

    // Run tests and snapshot existing results into the status file
    let output = Command::new("cargo")
        .args([
            "nextest",
            "run",
            "--no-fail-fast",
            "--message-format",
            "libtest-json",
        ])
        .current_dir(project_dir)
        .env("TDD_RATCHET", "1")
        .env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")
        .output()
        .ok();

    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = parse_nextest_output(&stdout);
        for result in &results {
            if result.outcome == tdd_ratchet::runner::TestOutcome::Ignored {
                continue;
            }
            let state = match result.outcome {
                tdd_ratchet::runner::TestOutcome::Passed => tdd_ratchet::status::TestState::Passing,
                tdd_ratchet::runner::TestOutcome::Failed => tdd_ratchet::status::TestState::Pending,
                tdd_ratchet::runner::TestOutcome::Ignored => unreachable!(),
            };
            status.tests.insert(
                result.name.clone(),
                tdd_ratchet::status::TestEntry::Simple(state),
            );
        }
    }

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

fn get_head_commit(project_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn run_ratchet(project_dir: &Path, status_path: &Path) {
    let gathered = gather_run(project_dir, status_path);

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

fn gather_run(project_dir: &Path, status_path: &Path) -> GatheredRun {
    let status = load_status_input(status_path);
    let results = run_nextest(project_dir, true);
    let baseline = status.baseline.as_deref();
    let history_snapshots = collect_history_snapshots(project_dir, baseline).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to inspect git history: {e}");
        process::exit(1);
    });

    GatheredRun {
        status,
        results,
        history_snapshots,
    }
}

fn load_status_input(status_path: &Path) -> StatusFile {
    if !status_path.exists() {
        eprintln!(
            "tdd-ratchet: no .test-status.json found.\n\
             Run `tdd-ratchet --init` to create one."
        );
        process::exit(1);
    }

    StatusFile::read_from_path(status_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: {e}");
        process::exit(1);
    })
}

fn run_nextest(project_dir: &Path, inherit_stderr: bool) -> Vec<tdd_ratchet::runner::TestResult> {
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
