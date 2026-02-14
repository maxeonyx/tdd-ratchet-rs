use std::env;
use std::path::PathBuf;
use std::process::{self, Command};

use tdd_ratchet::errors::format_violation;
use tdd_ratchet::ratchet::check_ratchet;
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
        init(&status_path);
        return;
    }

    run_ratchet(&project_dir, &status_path);
}

fn init(status_path: &PathBuf) {
    if status_path.exists() {
        eprintln!(
            "tdd-ratchet: .test-status.json already exists. Remove it first to re-initialize."
        );
        process::exit(1);
    }
    let status = StatusFile::empty();
    status.save(status_path).unwrap_or_else(|e| {
        eprintln!("tdd-ratchet: failed to create status file: {e}");
        process::exit(1);
    });
    println!("tdd-ratchet: initialized .test-status.json");
}

fn run_ratchet(project_dir: &PathBuf, status_path: &PathBuf) {
    // Load status file
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

    // Run cargo test with TDD_RATCHET=1
    let output = Command::new("cargo")
        .args(["test"])
        .current_dir(project_dir)
        .env("TDD_RATCHET", "1")
        .output()
        .unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to run cargo test: {e}");
            process::exit(1);
        });

    let combined = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    // Parse test results
    let results = parse_cargo_test_output(&combined);

    // Apply ratchet rules
    let outcome = check_ratchet(&status, &results);

    if outcome.violations.is_empty() {
        // Save updated status file
        outcome.updated.save(status_path).unwrap_or_else(|e| {
            eprintln!("tdd-ratchet: failed to save status file: {e}");
            process::exit(1);
        });

        let pending_count = outcome
            .updated
            .tests
            .values()
            .filter(|s| matches!(s, tdd_ratchet::status::TestState::Pending))
            .count();
        let passing_count = outcome
            .updated
            .tests
            .values()
            .filter(|s| matches!(s, tdd_ratchet::status::TestState::Passing))
            .count();

        println!(
            "tdd-ratchet: ok ({passing} passing, {pending} pending)",
            passing = passing_count,
            pending = pending_count,
        );
    } else {
        eprintln!();
        for v in &outcome.violations {
            eprintln!("{}", format_violation(v));
            eprintln!();
        }
        eprintln!(
            "tdd-ratchet: {} violation(s) found. Status file not updated.",
            outcome.violations.len()
        );
        process::exit(1);
    }
}
