// tests/status_file.rs
//
// Story 4: Committed status file tracking each test's expected state.

mod common;

use common::TestDir;
use std::collections::BTreeMap;
use std::fs;
use tdd_ratchet::status::{StatusFile, TestState};

fn make_status(tests: &[(&str, TestState)]) -> StatusFile {
    let mut map = BTreeMap::new();
    for (name, state) in tests {
        map.insert(name.to_string(), *state);
    }
    StatusFile {
        tests: map,
        baseline: None,
    }
}

#[test]
fn empty_status_file_parses_to_empty_map() {
    let json = r#"{"tests":{}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert!(status.tests.is_empty());
}

#[test]
fn status_file_with_pending_and_passing_loads_correctly() {
    let json = r#"{"tests":{"mod::test_a":"passing","mod::test_b":"pending"}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(status.tests["mod::test_a"], TestState::Passing);
    assert_eq!(status.tests["mod::test_b"], TestState::Pending);
}

#[test]
fn round_trip_write_then_read() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");

    let original = make_status(&[
        ("test_one", TestState::Passing),
        ("test_two", TestState::Pending),
    ]);

    original.save(&path).unwrap();
    let loaded = StatusFile::load(&path).unwrap();

    assert_eq!(original, loaded);
    dir.pass();
}

#[test]
fn status_file_does_not_exist_returns_error() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");
    let result = StatusFile::load(&path);
    assert!(result.is_err());
    dir.pass();
}

#[test]
fn malformed_json_returns_clear_error() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");
    fs::write(&path, "{ not valid json }").unwrap();

    let result = StatusFile::load(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("parse") || err.contains("JSON") || err.contains("json"),
        "Error should mention parsing: {err}"
    );
    dir.pass();
}

#[test]
fn unknown_fields_are_ignored() {
    let json = r#"{"tests":{"a":"passing"},"future_field":"whatever"}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(status.tests.len(), 1);
}

#[test]
fn test_name_with_special_characters() {
    let json = r#"{"tests":{"mod::sub::test with spaces & colons: yes":"pending"}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(
        status.tests["mod::sub::test with spaces & colons: yes"],
        TestState::Pending
    );
}

#[test]
fn saved_file_is_human_readable_json() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");

    let status = make_status(&[
        ("b_test", TestState::Pending),
        ("a_test", TestState::Passing),
    ]);
    status.save(&path).unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    // Should be pretty-printed (contains newlines) and sorted (a before b)
    assert!(contents.contains('\n'), "Should be pretty-printed");
    let a_pos = contents.find("a_test").unwrap();
    let b_pos = contents.find("b_test").unwrap();
    assert!(a_pos < b_pos, "Tests should be sorted alphabetically");
    dir.pass();
}
