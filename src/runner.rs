// Test runner output parsing: extracts per-test results from nextest
// libtest-json structured output.

use serde::Deserialize;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    pub name: String,
    pub outcome: TestOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandExecutor {
    fn output(&mut self, command: &mut Command) -> io::Result<CommandOutput>;
}

pub struct ProcessExecutor;

impl CommandExecutor for ProcessExecutor {
    fn output(&mut self, command: &mut Command) -> io::Result<CommandOutput> {
        let output = command.output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            status: output.status.to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug)]
pub enum NextestError {
    Start(io::Error),
    Failed(CommandOutput),
}

impl fmt::Display for NextestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(error) => write!(formatter, "failed to start `cargo nextest`: {error}"),
            Self::Failed(output) => write!(
                formatter,
                "`cargo nextest` failed ({})\n{}{}",
                output.status, output.stdout, output.stderr
            ),
        }
    }
}

impl std::error::Error for NextestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    pub results: Vec<TestResult>,
    pub stderr: String,
}

pub fn nextest_command(project_dir: &Path) -> Command {
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
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
}

pub fn run_nextest(project_dir: &Path) -> Result<TestRun, NextestError> {
    run_nextest_with_executor(project_dir, &mut ProcessExecutor)
}

pub fn run_nextest_with_executor(
    project_dir: &Path,
    executor: &mut impl CommandExecutor,
) -> Result<TestRun, NextestError> {
    let mut command = nextest_command(project_dir);
    let output = executor.output(&mut command).map_err(NextestError::Start)?;

    // The status check is intentionally introduced by the green commit. This
    // refactor first exposes the process boundary without changing behavior.
    Ok(TestRun {
        results: parse_nextest_output(&output.stdout),
        stderr: output.stderr,
    })
}

#[derive(Deserialize)]
struct TestEvent {
    #[serde(rename = "type")]
    kind: String,
    event: String,
    name: Option<String>,
}

/// Parse nextest libtest-json output into per-test results.
///
/// Each JSON line with `"type":"test"` and `"event":"ok"|"failed"|"ignored"`
/// produces a TestResult. The full nextest name is preserved as-is
/// (e.g. `my-crate::tests$test_name`).
pub fn parse_nextest_output(output: &str) -> Vec<TestResult> {
    let mut results = Vec::new();
    for line in output.lines() {
        let Ok(event) = serde_json::from_str::<TestEvent>(line) else {
            continue;
        };
        if event.kind != "test" {
            continue;
        }
        let outcome = match event.event.as_str() {
            "ok" => TestOutcome::Passed,
            "failed" => TestOutcome::Failed,
            "ignored" => TestOutcome::Ignored,
            _ => continue, // "started" etc.
        };
        let Some(full_name) = event.name else {
            continue;
        };
        // Keep the full nextest name as-is (e.g. "my-crate::tests$test_one")
        results.push(TestResult {
            name: full_name,
            outcome,
        });
    }
    results
}
