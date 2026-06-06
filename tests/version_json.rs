use std::process::Command;

#[test]
fn version_json_flag_prints_machine_readable_version() {
    let output = Command::new(assert_cmd::cargo::cargo_bin("cargo-ratchet"))
        .args(["--version", "--json"])
        .output()
        .expect("run cargo-ratchet --version --json");

    assert!(
        output.status.success(),
        "cargo-ratchet --version --json should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(value["package"], "tdd-ratchet");
    assert_eq!(value["binary"], "cargo-ratchet");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
}
