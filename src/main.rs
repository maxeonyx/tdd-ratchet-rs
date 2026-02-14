use std::env;
use std::path::PathBuf;
use std::process::{self, Command};

use tdd_ratchet::errors::format_eval_violation;
use tdd_ratchet::history::collect_history_snapshots;
use tdd_ratchet::ratchet::evaluate;
use tdd_ratchet::runner::parse_cargo_test_output;
use tdd_ratchet::status::StatusFile;

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

fn init(status_path: &PathBuf, project_dir: &PathBuf) {
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
        .args(["test", "--no-fail-fast"])
        .current_dir(project_dir)
        .env("TDD_RATCHET", "1")
        .output()
        .ok();

    if let Some(output) = output {
        let combined = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        let results = parse_cargo_test_output(&combined);
        for result in &results {
            if result.outcome == tdd_ratchet::runner::TestOutcome::Ignored {
                continue;
            }
            let state = match result.outcome {
                tdd_ratchet::runner::TestOutcome::Passed => tdd_ratchet::status::TestState::Passing,
                tdd_ratchet::runner::TestOutcome::Failed => tdd_ratchet::status::TestState::Pending,
                tdd_ratchet::runner::TestOutcome::Ignored => unreachable!(),
            };
            status.tests.insert(result.name.clone(), state);
        }
    }

    status.save(status_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to create status file: {e}");
        process::exit(1);
    });

    let passing = status
        .tests
        .values()
        .filter(|s| matches!(s, tdd_ratchet::status::TestState::Passing))
        .count();
    let pending = status
        .tests
        .values()
        .filter(|s| matches!(s, tdd_ratchet::status::TestState::Pending))
        .count();
    println!("tdd-ratchet: initialized .test-status.json ({passing} passing, {pending} pending)");
}

fn get_head_commit(project_dir: &PathBuf) -> Option<String> {
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

fn run_ratchet(project_dir: &PathBuf, status_path: &PathBuf) {
    // ── Phase 1: Gather ─────────────────────────────────────────────
    let status = if status_path.exists() {
        StatusFile::load(status_path).unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: {e}");
            process::exit(1);
        })
    } else {
        eprintln!(
            "tdd-ratchet: no .test-status.json found.\n\
             Run `tdd-ratchet --init` to create one."
        );
        process::exit(1);
    };

    let output = Command::new("cargo")
        .args(["test", "--no-fail-fast"])
        .current_dir(project_dir)
        .env("TDD_RATCHET", "1")
        .output()
        .unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to run cargo test: {e}");
            process::exit(1);
        });

    let combined = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    let results = parse_cargo_test_output(&combined);

    let baseline = status.baseline.as_deref();
    let history_snapshots = collect_history_snapshots(project_dir, baseline).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to inspect git history: {e}");
        process::exit(1);
    });

    // ── Phase 2: Evaluate (pure) ────────────────────────────────────
    let result = evaluate(&status, &results, &history_snapshots);

    // ── Phase 3: Output ─────────────────────────────────────────────
    // Always save the updated status file — valid transitions (new
    // pending tests, promotions) should persist even when there are
    // violations. This prevents losing state on partial runs.
    result.updated.save(status_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to save status file: {e}");
        process::exit(1);
    });

    let pending_count = result
        .updated
        .tests
        .values()
        .filter(|s| matches!(s, tdd_ratchet::status::TestState::Pending))
        .count();
    let passing_count = result
        .updated
        .tests
        .values()
        .filter(|s| matches!(s, tdd_ratchet::status::TestState::Passing))
        .count();

    if result.violations.is_empty() {
        println!(
            "tdd-ratchet: ok ({passing} passing, {pending} pending)",
            passing = passing_count,
            pending = pending_count,
        );
    } else {
        eprintln!();
        for v in &result.violations {
            eprintln!("{}", format_eval_violation(v));
            eprintln!();
        }
        eprintln!(
            "tdd-ratchet: {n} violation(s) found ({passing} passing, {pending} pending)",
            n = result.violations.len(),
            passing = passing_count,
            pending = pending_count,
        );
        process::exit(1);
    }
}
