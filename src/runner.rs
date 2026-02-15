// Test runner output parsing: extracts per-test results from nextest
// libtest-json structured output.

use serde::Deserialize;

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
/// produces a TestResult. The test name is the part after `$` in the
/// nextest name format `crate::binary$test_name`.
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
        // Name format: "crate::binary$test_name" — extract after $
        let name = match full_name.split_once('$') {
            Some((_, test_name)) => test_name.to_string(),
            None => full_name,
        };
        results.push(TestResult { name, outcome });
    }
    results
}
