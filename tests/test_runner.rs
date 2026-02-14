// tests/test_runner.rs
//
// Stories 2, 3: Easy installation. Transparent — wraps the test harness
// without getting in the way.
//
// The ratchet invokes `cargo test` (or `cargo nextest` if available)
// and parses per-test results from the output. The user's test code
// is unchanged — the ratchet is purely a wrapper.
//
// Isolation: creates a temp Rust project with known tests and runs the
// ratchet's test-runner parsing against it. HOME and GIT_CONFIG_NOSYSTEM
// set.
//
// Test cases:
// - Project with three tests (2 pass, 1 fail) → correctly identifies
//   each test and its result
// - Test names with modules (e.g. `module::submodule::test_name`) →
//   parsed correctly
// - No tests in project → empty results, no error
//
// Edge cases:
// - Test that panics vs test that asserts → both detected as failures
// - Test with very long name → parsed correctly
// - Cargo test output with compiler warnings mixed in → not confused
// - #[ignore]d tests → handled (not counted as missing)
