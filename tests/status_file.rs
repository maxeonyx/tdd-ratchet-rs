// tests/status_file.rs
//
// Story 4: Committed status file tracking each test's expected state.

mod common;

use common::TestDir;
use std::collections::BTreeMap;
use std::fs;
use tdd_ratchet::status::{StatusFile, TestEntry, TestState};

fn make_status(tests: &[(&str, TestState)]) -> StatusFile {
    let mut map = BTreeMap::new();
    for (name, state) in tests {
        map.insert(name.to_string(), TestEntry::Simple(*state));
    }
    StatusFile::new(map, None)
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
    assert_eq!(status.tests["mod::test_a"].state(), TestState::Passing);
    assert_eq!(status.tests["mod::test_b"].state(), TestState::Pending);
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

    // save() injects $schema, so compare the fields we care about
    assert_eq!(original.tests, loaded.tests);
    assert_eq!(original.baseline, loaded.baseline);
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
fn unknown_fields_are_rejected() {
    let json = r#"{"tests":{"a":"passing"},"future_field":"whatever"}"#;
    let result: Result<StatusFile, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Unknown fields should be rejected");
}

#[test]
fn schema_field_is_accepted() {
    let json = r#"{"$schema":"https://tdd-ratchet.maxeonyx.com/schema/test-status.v1.json","tests":{"a":"passing"}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(status.tests.len(), 1);
}

#[test]
fn save_always_writes_schema_key() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");

    let status = make_status(&[("a", TestState::Passing)]);
    status.save(&path).unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains("$schema"),
        "Saved file should contain $schema key"
    );
    assert!(
        contents.contains("tdd-ratchet.maxeonyx.com"),
        "Saved file should contain schema URL"
    );
    dir.pass();
}

#[test]
fn test_name_with_special_characters() {
    let json = r#"{"tests":{"mod::sub::test with spaces & colons: yes":"pending"}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(
        status.tests["mod::sub::test with spaces & colons: yes"].state(),
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

#[test]
fn per_test_baseline_object_form_parses() {
    let json = r#"{"tests":{"my_test":{"state":"passing","baseline":"abc123"}}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(status.tests["my_test"].state(), TestState::Passing);
    assert_eq!(status.tests["my_test"].baseline(), Some("abc123"));
}

#[test]
fn per_test_baseline_mixed_with_simple_entries() {
    let json =
        r#"{"tests":{"simple":"pending","with_baseline":{"state":"passing","baseline":"def456"}}}"#;
    let status: StatusFile = serde_json::from_str(json).unwrap();
    assert_eq!(status.tests["simple"].state(), TestState::Pending);
    assert_eq!(status.tests["simple"].baseline(), None);
    assert_eq!(status.tests["with_baseline"].state(), TestState::Passing);
    assert_eq!(status.tests["with_baseline"].baseline(), Some("def456"));
}

#[test]
fn save_normalizes_simple_entries_as_strings() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");

    let status = make_status(&[("a", TestState::Passing)]);
    status.save(&path).unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    // Simple entries should be bare strings, not objects
    assert!(
        contents.contains(r#""a": "passing""#),
        "Simple entries should serialize as bare strings: {contents}"
    );
    dir.pass();
}

#[test]
fn save_preserves_per_test_baseline_as_object() {
    let dir = TestDir::new();
    let path = dir.path().join(".test-status.json");

    let mut tests = BTreeMap::new();
    tests.insert("simple".to_string(), TestEntry::Simple(TestState::Passing));
    tests.insert(
        "grandfathered".to_string(),
        TestEntry::WithBaseline {
            state: TestState::Passing,
            baseline: "abc123".to_string(),
        },
    );
    let status = StatusFile::new(tests, None);
    status.save(&path).unwrap();

    let loaded = StatusFile::load(&path).unwrap();
    assert_eq!(loaded.tests["simple"].state(), TestState::Passing);
    assert_eq!(loaded.tests["simple"].baseline(), None);
    assert_eq!(loaded.tests["grandfathered"].state(), TestState::Passing);
    assert_eq!(loaded.tests["grandfathered"].baseline(), Some("abc123"));
    dir.pass();
}

#[test]
fn schema_validates_status_file() {
    let schema_str = fs::read_to_string("docs/schema/test-status.v1.json")
        .expect("Schema file should exist at docs/schema/test-status.v1.json");
    let schema: serde_json::Value = serde_json::from_str(&schema_str).unwrap();

    let status_str = fs::read_to_string(".test-status.json")
        .expect("Status file should exist at .test-status.json");
    let instance: serde_json::Value = serde_json::from_str(&status_str).unwrap();

    let validator =
        jsonschema::validator_for(&schema).expect("Schema should be a valid JSON Schema");

    let errors: Vec<_> = validator.iter_errors(&instance).collect();
    assert!(
        errors.is_empty(),
        ".test-status.json does not validate against schema:\n{}",
        errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
