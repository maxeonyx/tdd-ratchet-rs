use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::process::{self, Command, Stdio};

use tdd_ratchet::errors::format_report;
use tdd_ratchet::history::{
    BaselineViolation, adoption_snapshot_index, check_adoption_baseline, collect_history_snapshots,
    read_head_status,
};
use tdd_ratchet::ratchet::{Violation, evaluate};
use tdd_ratchet::runner::{TestOutcome, TestResult, parse_nextest_output};
use tdd_ratchet::status::{StatusFile, TestState, TrackedStatus, WorkingTreeInstructions};

const HELP_TEXT: &str = "tdd-ratchet enforces strict TDD for Rust projects. New tests must fail in one committed run before they are allowed to pass in a later committed run, using `.test-status.json` plus git history as the record.\n\nUsage: cargo ratchet [--init] [--help] [--version]\n\nWithout flags, cargo ratchet runs `cargo nextest`, compares the results with the committed `.test-status.json`, enforces the pending→passing workflow, and writes the updated status file back to the working tree.\n\nOptions:\n  --init          Initialize .test-status.json from the current test run\n  --help          Print help\n  --version       Print version\n\nExamples:\n  $ cargo ratchet --init            # Initialize from current test state\n  $ cargo ratchet                   # Run tests with ratchet enforcement\n";

struct GatheredRun {
    status: TrackedStatus,
    instructions: WorkingTreeInstructions,
    results: Vec<tdd_ratchet::runner::TestResult>,
    history_snapshots: Vec<tdd_ratchet::history::HistorySnapshot>,
    adoption_snapshot_idx: usize,
    /// The committed adoption baseline from HEAD, re-injected into the written
    /// output so it survives every run (it is an IO/git concern, not a
    /// transition-logic concern, so the pure `evaluate` never sees it).
    committed_baseline: Option<String>,
    /// Immutability tripwire: set when HEAD's baseline link was moved.
    baseline_violation: Option<BaselineViolation>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if is_version_json_request(&args[1..]) {
        println!(
            "{{\"package\":\"tdd-ratchet\",\"binary\":\"cargo-ratchet\",\"version\":\"{}\"}}",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP_TEXT}");
        return;
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("cargo-ratchet {}", env!("CARGO_PKG_VERSION"));
        return;
    }

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

fn is_version_json_request(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--version" | "-V"))
        && args.iter().any(|arg| arg == "--json")
        && args
            .iter()
            .all(|arg| matches!(arg.as_str(), "--version" | "-V" | "--json"))
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
        .filter(|s| **s == TestState::Passing)
        .count();
    let pending = status
        .tests
        .values()
        .filter(|s| **s == TestState::Pending)
        .count();
    println!("tdd-ratchet: initialized .test-status.json ({passing} passing, {pending} pending)");
}

fn run_ratchet(project_dir: &Path, status_path: &Path) {
    let gathered = gather_run(project_dir);

    // ── Phase 2: Evaluate (pure) ────────────────────────────────────
    let mut result = evaluate(
        &gathered.status,
        &gathered.instructions,
        &gathered.results,
        &gathered.history_snapshots,
        gathered.adoption_snapshot_idx,
    );

    // The adoption-baseline immutability check is a git-IO concern, computed in
    // the gather phase; fold it into the unified violation set so all output
    // flows through one formatter.
    if let Some(BaselineViolation::Moved {
        head_baseline,
        pointed_at_baseline,
    }) = gathered.baseline_violation
    {
        result.violations.push(Violation::AdoptionBaselineMoved {
            head_baseline,
            pointed_at_baseline,
        });
    }

    // ── Phase 3: Output ─────────────────────────────────────────────
    // Always save the updated status file — valid transitions (new
    // pending tests, promotions) should persist even when there are
    // violations. This prevents losing state on partial runs.
    //
    // Re-inject the committed adoption baseline so it survives the run; the
    // pure evaluate path deliberately does not carry it.
    let mut output = result.updated.clone();
    output.baseline = gathered.committed_baseline.clone();
    output.write_to_path(status_path).unwrap_or_else(|e| {
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
    let committed = load_committed_status_input(project_dir);
    let committed_baseline = committed.baseline.clone();
    let status = committed.into_tracked_status();
    let instructions = load_working_tree_instructions(project_dir);
    let results = run_nextest(project_dir, true);
    let history_snapshots = collect_history_snapshots(project_dir).unwrap_or_else(|e| {
        print_actionable_git_error("failed to inspect git history", &e);
        process::exit(1);
    });
    let adoption_snapshot_idx = adoption_snapshot_index(project_dir, &history_snapshots)
        .unwrap_or_else(|e| {
            print_actionable_git_error("failed to resolve adoption baseline", &e);
            process::exit(1);
        });
    let baseline_violation = check_adoption_baseline(project_dir).unwrap_or_else(|e| {
        print_actionable_git_error("failed to check adoption baseline", &e);
        process::exit(1);
    });

    GatheredRun {
        status,
        instructions,
        results,
        history_snapshots,
        adoption_snapshot_idx,
        committed_baseline,
        baseline_violation,
    }
}

fn load_committed_status_input(project_dir: &Path) -> StatusFile {
    read_head_status(project_dir)
        .unwrap_or_else(|e| {
            print_actionable_git_error("failed to read committed status file", &e);
            process::exit(1);
        })
        .unwrap_or_else(StatusFile::empty)
}

fn print_actionable_git_error(operation: &str, error: &git2::Error) {
    let message = match classify_git_error(error) {
        GitErrorKind::NotGitRepository => {
            "tdd-ratchet: not a git repository. tdd-ratchet must be run inside a git repository."
        }
        GitErrorKind::NoCommitsFound => {
            "tdd-ratchet: no commits found. Run `cargo ratchet --init`, then commit .test-status.json before running again."
        }
        GitErrorKind::Other => {
            eprintln!("tdd-ratchet: {operation}: {error}");
            return;
        }
    };

    eprintln!("{message}");
    eprintln!("Caused by: {error}");
}

fn classify_git_error(error: &git2::Error) -> GitErrorKind {
    if error.code() == git2::ErrorCode::UnbornBranch {
        return GitErrorKind::NoCommitsFound;
    }

    if error.class() == git2::ErrorClass::Repository
        && error.code() == git2::ErrorCode::NotFound
        && error.message().to_ascii_lowercase().contains("repository")
    {
        return GitErrorKind::NotGitRepository;
    }

    GitErrorKind::Other
}

enum GitErrorKind {
    NotGitRepository,
    NoCommitsFound,
    Other,
}

fn load_working_tree_instructions(project_dir: &Path) -> WorkingTreeInstructions {
    let status_path = project_dir.join(".test-status.json");
    if !status_path.exists() {
        return WorkingTreeInstructions::default();
    }

    StatusFile::load(&status_path)
        .map(|status| status.working_tree_instructions())
        .unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to read working-tree instructions: {e}");
            process::exit(1);
        })
}

fn status_entries_from_results(results: &[TestResult]) -> BTreeMap<String, TestState> {
    results
        .iter()
        .filter_map(|result| match result.outcome {
            TestOutcome::Passed => Some((result.name.clone(), TestState::Passing)),
            TestOutcome::Failed => Some((result.name.clone(), TestState::Pending)),
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
