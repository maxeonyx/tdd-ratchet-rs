// Test runner output parsing: extracts per-test results from cargo test verbose output.

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

/// Parse `cargo test` verbose output into per-test results.
///
/// Looks for lines matching `test <name> ... ok/FAILED/ignored`.
pub fn parse_cargo_test_output(output: &str) -> Vec<TestResult> {
    let mut results = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if !line.starts_with("test ") {
            continue;
        }
        let outcome = if line.ends_with(" ... ok") {
            TestOutcome::Passed
        } else if line.ends_with(" ... FAILED") {
            TestOutcome::Failed
        } else if line.ends_with(" ... ignored") {
            TestOutcome::Ignored
        } else {
            continue;
        };
        // Extract name: between "test " and " ... "
        let after_test = &line["test ".len()..];
        let name = after_test
            .rsplit_once(" ... ")
            .map(|(name, _)| name)
            .unwrap_or(after_test);
        results.push(TestResult {
            name: name.to_string(),
            outcome,
        });
    }
    results
}
