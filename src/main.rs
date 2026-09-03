use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::process;

use tdd_ratchet::errors::format_report;
use tdd_ratchet::history::{
    HistoryViolation, check_history, collect_history_snapshots, read_head_status,
};
use tdd_ratchet::ratchet::{evaluate, preserve_passing_results};
use tdd_ratchet::runner::{TestOutcome, TestResult, run_nextest as execute_nextest};
use tdd_ratchet::status::{StatusFile, TestState, TrackedStatus, WorkingTreeInstructions};

const HELP_TEXT: &str = "tdd-ratchet enforces strict TDD for Rust projects. New tests must fail in one committed run before they are allowed to pass in a later committed run, using the trusted `.test-status.json` ledger plus git history as the record.\n\nUsage: cargo ratchet [--init] [--help] [--version [--json]] [--preserve-passing <TEST>]...\n\nPrerequisite:\n  cargo-nextest must be installed and available as `cargo nextest`.\n\nWithout flags, cargo ratchet runs `cargo nextest`, compares the results with the committed ledger, enforces the pending→passing workflow, and writes a preview to `.test-status.json`. On pull requests, the trusted ledger workflow validates and commits that output; developers do not hand-edit it.\n\nOptions:\n  --init                    Create the one-time adoption snapshot before enabling the trusted workflow\n  --help, -h                Print help\n  --version, -V             Print version; combine with --json for machine-readable metadata\n  --json                    Format --version output as JSON\n  --preserve-passing TEST   Preserve an exact already-passing result that this runner cannot observe; repeatable\n\nExamples:\n  $ cargo ratchet --init            # Bootstrap the adoption snapshot once\n  $ cargo ratchet                   # Run tests with ratchet enforcement\n  $ cargo ratchet --version --json  # Print machine-readable version metadata\n";

enum CliAction {
    Run { preserved_tests: Vec<String> },
    Init,
    Help,
    Version { json: bool },
}

struct GatheredRun {
    status: TrackedStatus,
    instructions: WorkingTreeInstructions,
    results: Vec<tdd_ratchet::runner::TestResult>,
    history_violations: Vec<HistoryViolation>,
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let action = parse_cli(&args).unwrap_or_else(|message| {
        eprintln!("tdd-ratchet: {message}");
        eprintln!("Run `cargo ratchet --help` for usage.");
        process::exit(2);
    });

    match &action {
        CliAction::Help => {
            print!("{HELP_TEXT}");
            return;
        }
        CliAction::Version { json: true } => {
            println!(
                "{{\"package\":\"tdd-ratchet\",\"binary\":\"cargo-ratchet\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
            return;
        }
        CliAction::Version { json: false } => {
            println!("cargo-ratchet {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        CliAction::Run { .. } | CliAction::Init => {}
    }

    let project_dir = env::current_dir().unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: cannot determine current directory: {e}");
        process::exit(1);
    });

    let status_path = project_dir.join(".test-status.json");

    if matches!(&action, CliAction::Init) {
        init(&status_path, &project_dir);
        return;
    }

    let CliAction::Run { preserved_tests } = action else {
        unreachable!("non-run actions returned before project execution")
    };
    run_ratchet(&project_dir, &status_path, &preserved_tests);
}

fn parse_cli(args: &[String]) -> Result<CliAction, String> {
    let args = if args.first().is_some_and(|arg| arg == "ratchet") {
        &args[1..]
    } else {
        args
    };

    if args.iter().any(|arg| arg == "--preserve-passing") {
        let mut preserved_tests = Vec::new();
        let mut index = 0;
        while index < args.len() {
            if args[index] != "--preserve-passing" {
                return Err(format!("unrecognized option `{}`", args[index]));
            }
            let Some(test) = args.get(index + 1) else {
                return Err("`--preserve-passing` requires an exact test name".to_string());
            };
            preserved_tests.push(test.clone());
            index += 2;
        }
        return Ok(CliAction::Run { preserved_tests });
    }

    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => Ok(CliAction::Run {
            preserved_tests: Vec::new(),
        }),
        ["--init"] => Ok(CliAction::Init),
        ["--help"] | ["-h"] => Ok(CliAction::Help),
        ["--version"] | ["-V"] => Ok(CliAction::Version { json: false }),
        ["--version", "--json"] | ["-V", "--json"] | ["--json", "--version"] | ["--json", "-V"] => {
            Ok(CliAction::Version { json: true })
        }
        _ => {
            if let Some(arg) = args.iter().find(|arg| {
                !matches!(
                    arg.as_str(),
                    "--init" | "--help" | "-h" | "--version" | "-V" | "--json"
                )
            }) {
                Err(format!("unrecognized option `{arg}`"))
            } else {
                Err(format!("invalid option combination `{}`", args.join(" ")))
            }
        }
    }
}

fn init(status_path: &Path, project_dir: &Path) {
    if status_path.exists() {
        eprintln!(
            "tdd-ratchet: .test-status.json already exists; the ledger cannot be re-initialized."
        );
        process::exit(1);
    }

    let prior_snapshots = collect_history_snapshots(project_dir).unwrap_or_else(|e| {
        print_actionable_git_error("failed to inspect git history before initialization", &e);
        process::exit(1);
    });
    if !prior_snapshots.is_empty() {
        eprintln!(
            "tdd-ratchet: this repository already adopted .test-status.json; restore the deleted ledger from history instead of re-initializing it."
        );
        process::exit(1);
    }

    let mut status = StatusFile::empty();

    // Run tests and snapshot existing results into the status file
    status.tests =
        status_entries_from_results(&run_nextest(project_dir, false, "cargo ratchet --init"));

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

fn run_ratchet(project_dir: &Path, status_path: &Path, preserved_tests: &[String]) {
    let gathered = gather_run(project_dir, preserved_tests);

    // ── Phase 2: Evaluate (pure) ────────────────────────────────────
    let result = evaluate(
        &gathered.status,
        &gathered.instructions,
        &gathered.results,
        &gathered.history_violations,
    );

    // ── Phase 3: Output ─────────────────────────────────────────────
    // Always save the updated status file — valid transitions (new
    // pending tests, promotions) should persist even when there are
    // violations. This prevents losing state on partial runs.
    //
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

fn gather_run(project_dir: &Path, preserved_tests: &[String]) -> GatheredRun {
    let status = load_committed_status_input(project_dir).into_tracked_status();
    let instructions = load_working_tree_instructions(project_dir);
    let observed_results = run_nextest(project_dir, true, "cargo ratchet");
    let results = preserve_passing_results(&status, &observed_results, preserved_tests)
        .unwrap_or_else(|errors| {
            eprintln!("tdd-ratchet: invalid --preserve-passing request:");
            for error in errors {
                eprintln!("  - {error}");
            }
            eprintln!(
                "What to do: Name each result exactly as it appears in .test-status.json, and preserve only tests that are already tracked as passing but cannot observe a required external dependency in this runner."
            );
            process::exit(2);
        });
    for test in preserved_tests {
        eprintln!("tdd-ratchet: preserving unobservable passing result `{test}`");
    }
    let history_violations = check_history(project_dir).unwrap_or_else(|e| {
        print_actionable_git_error("failed to inspect git history", &e);
        process::exit(1);
    });

    GatheredRun {
        status,
        instructions,
        results,
        history_violations,
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
            "tdd-ratchet: no commits found. Commit the project first, then run `cargo ratchet --init` once to create the adoption snapshot before enabling the trusted ledger workflow."
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
    let instructions_path = project_dir.join(".tdd-ratchet.json");
    if !instructions_path.exists() {
        return WorkingTreeInstructions::default();
    }

    WorkingTreeInstructions::read_from_path(&instructions_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to read .tdd-ratchet.json instructions: {e}");
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

fn run_nextest(project_dir: &Path, inherit_stderr: bool, retry_command: &str) -> Vec<TestResult> {
    let run = execute_nextest(project_dir).unwrap_or_else(|error| {
        eprintln!("tdd-ratchet: {error}");
        eprintln!(
            "What to do: Fix the Cargo or Nextest error above, then rerun `{retry_command}`."
        );
        process::exit(1);
    });

    if inherit_stderr {
        eprint!("{}", run.stderr);
    }

    run.results
}
