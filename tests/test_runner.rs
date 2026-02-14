// tests/test_runner.rs
//
// Stories 2, 3: The ratchet invokes cargo test and parses per-test results.

use tdd_ratchet::runner::{parse_cargo_test_output, TestOutcome, TestResult};

#[test]
fn parses_mixed_pass_and_fail() {
    let output = "\
running 3 tests
test tests::test_one ... ok
test tests::test_two ... FAILED
test tests::test_three ... ok

failures:

---- tests::test_two stdout ----
thread 'tests::test_two' panicked at 'assertion failed'

failures:
    tests::test_two

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 3);
    assert_eq!(
        results[0],
        TestResult {
            name: "tests::test_one".into(),
            outcome: TestOutcome::Passed
        }
    );
    assert_eq!(
        results[1],
        TestResult {
            name: "tests::test_two".into(),
            outcome: TestOutcome::Failed
        }
    );
    assert_eq!(
        results[2],
        TestResult {
            name: "tests::test_three".into(),
            outcome: TestOutcome::Passed
        }
    );
}

#[test]
fn parses_all_passing() {
    let output = "\
running 2 tests
test alpha ... ok
test beta ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.outcome == TestOutcome::Passed));
}

#[test]
fn parses_deeply_nested_module_names() {
    let output = "\
running 1 test
test a::b::c::d::deep_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "a::b::c::d::deep_test");
}

#[test]
fn no_tests_returns_empty() {
    let output = "\
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let results = parse_cargo_test_output(output);
    assert!(results.is_empty());
}

#[test]
fn ignored_tests_are_tracked_as_ignored() {
    let output = "\
running 3 tests
test real_test ... ok
test slow_test ... ignored
test another ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 3);
    assert_eq!(
        results[1],
        TestResult {
            name: "slow_test".into(),
            outcome: TestOutcome::Ignored
        }
    );
}

#[test]
fn compiler_warnings_mixed_in_do_not_confuse_parser() {
    let output = "\
warning: unused variable: `x`
 --> src/lib.rs:10:9
  |
10 |     let x = 5;
  |         ^ help: if this is intentional, prefix it with an underscore: `_x`

running 1 test
test basic ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "basic");
}

#[test]
fn multiple_test_binaries_combined() {
    // cargo test runs integration tests as separate binaries
    let output = "\
running 1 test
test unit_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 2 tests
test integration::test_a ... ok
test integration::test_b ... FAILED

failures:

failures:
    integration::test_b

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
    let results = parse_cargo_test_output(output);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].name, "unit_test");
    assert_eq!(results[1].name, "integration::test_a");
    assert_eq!(
        results[2],
        TestResult {
            name: "integration::test_b".into(),
            outcome: TestOutcome::Failed
        }
    );
}
