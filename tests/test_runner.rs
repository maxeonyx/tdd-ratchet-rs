// tests/test_runner.rs
//
// Stories 2, 3: The ratchet invokes cargo nextest and parses per-test results
// from libtest-json structured output.

use tdd_ratchet::runner::{parse_nextest_output, TestOutcome, TestResult};

#[test]
fn parses_mixed_pass_and_fail() {
    let output = r#"{"type":"suite","event":"started","test_count":3}
{"type":"test","event":"started","name":"my-crate::tests$test_one"}
{"type":"test","event":"ok","name":"my-crate::tests$test_one","exec_time":0.001}
{"type":"test","event":"started","name":"my-crate::tests$test_two"}
{"type":"test","event":"failed","name":"my-crate::tests$test_two","exec_time":0.002,"stdout":"assertion failed"}
{"type":"test","event":"started","name":"my-crate::tests$test_three"}
{"type":"test","event":"ok","name":"my-crate::tests$test_three","exec_time":0.001}
{"type":"suite","event":"failed","passed":2,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.004}
"#;
    let results = parse_nextest_output(output);
    assert_eq!(results.len(), 3);
    assert_eq!(
        results[0],
        TestResult {
            name: "test_one".into(),
            outcome: TestOutcome::Passed
        }
    );
    assert_eq!(
        results[1],
        TestResult {
            name: "test_two".into(),
            outcome: TestOutcome::Failed
        }
    );
    assert_eq!(
        results[2],
        TestResult {
            name: "test_three".into(),
            outcome: TestOutcome::Passed
        }
    );
}

#[test]
fn parses_all_passing() {
    let output = r#"{"type":"suite","event":"started","test_count":2}
{"type":"test","event":"started","name":"my-crate::lib$alpha"}
{"type":"test","event":"ok","name":"my-crate::lib$alpha","exec_time":0.001}
{"type":"test","event":"started","name":"my-crate::lib$beta"}
{"type":"test","event":"ok","name":"my-crate::lib$beta","exec_time":0.001}
{"type":"suite","event":"ok","passed":2,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.002}
"#;
    let results = parse_nextest_output(output);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.outcome == TestOutcome::Passed));
}

#[test]
fn parses_deeply_nested_module_names() {
    let output = r#"{"type":"suite","event":"started","test_count":1}
{"type":"test","event":"started","name":"my-crate::lib$a::b::c::d::deep_test"}
{"type":"test","event":"ok","name":"my-crate::lib$a::b::c::d::deep_test","exec_time":0.001}
{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.001}
"#;
    let results = parse_nextest_output(output);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "a::b::c::d::deep_test");
}

#[test]
fn no_tests_returns_empty() {
    let output = r#"{"type":"suite","event":"started","test_count":0}
{"type":"suite","event":"ok","passed":0,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.0}
"#;
    let results = parse_nextest_output(output);
    assert!(results.is_empty());
}

#[test]
fn ignored_tests_are_tracked_as_ignored() {
    let output = r#"{"type":"suite","event":"started","test_count":3}
{"type":"test","event":"started","name":"my-crate::lib$real_test"}
{"type":"test","event":"ok","name":"my-crate::lib$real_test","exec_time":0.001}
{"type":"test","event":"started","name":"my-crate::lib$slow_test"}
{"type":"test","event":"ignored","name":"my-crate::lib$slow_test"}
{"type":"test","event":"started","name":"my-crate::lib$another"}
{"type":"test","event":"ok","name":"my-crate::lib$another","exec_time":0.001}
{"type":"suite","event":"ok","passed":2,"failed":0,"ignored":1,"measured":0,"filtered_out":0,"exec_time":0.002}
"#;
    let results = parse_nextest_output(output);
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
fn non_json_lines_are_skipped() {
    // nextest may mix human-readable output with JSON on stdout
    let output = r#"some random non-json line
{"type":"suite","event":"started","test_count":1}
another non-json line
{"type":"test","event":"started","name":"my-crate::lib$basic"}
{"type":"test","event":"ok","name":"my-crate::lib$basic","exec_time":0.001}
{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.001}
"#;
    let results = parse_nextest_output(output);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "basic");
}

#[test]
fn multiple_suites_combined() {
    // nextest reports one suite per test binary
    let output = r#"{"type":"suite","event":"started","test_count":1}
{"type":"test","event":"started","name":"my-crate::unit_tests$unit_test"}
{"type":"test","event":"ok","name":"my-crate::unit_tests$unit_test","exec_time":0.001}
{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.001}
{"type":"suite","event":"started","test_count":2}
{"type":"test","event":"started","name":"my-crate::integration$test_a"}
{"type":"test","event":"ok","name":"my-crate::integration$test_a","exec_time":0.001}
{"type":"test","event":"started","name":"my-crate::integration$test_b"}
{"type":"test","event":"failed","name":"my-crate::integration$test_b","exec_time":0.002,"stdout":"boom"}
{"type":"suite","event":"failed","passed":1,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.003}
"#;
    let results = parse_nextest_output(output);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].name, "unit_test");
    assert_eq!(results[1].name, "test_a");
    assert_eq!(
        results[2],
        TestResult {
            name: "test_b".into(),
            outcome: TestOutcome::Failed
        }
    );
}
