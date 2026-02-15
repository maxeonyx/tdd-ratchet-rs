// tests/bypass_prevention.rs
//
// Story 8: `cargo test` run directly should fail with instructions.

mod common;

use common::TestDir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve RUSTUP_HOME for subprocess isolation.
fn rustup_home() -> PathBuf {
    if let Ok(val) = std::env::var("RUSTUP_HOME") {
        PathBuf::from(val)
    } else {
        let real_home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(real_home).join(".rustup")
    }
}

fn cargo_home() -> PathBuf {
    if let Ok(val) = std::env::var("CARGO_HOME") {
        PathBuf::from(val)
    } else {
        let real_home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(real_home).join(".cargo")
    }
}

/// Create a minimal Rust project in a temp dir with a gatekeeper test.
fn create_project_with_gatekeeper(dir: &Path) {
    // Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "").unwrap();

    fs::create_dir_all(dir.join("tests")).unwrap();

    // Gatekeeper test
    fs::write(
        dir.join("tests/gatekeeper.rs"),
        r#"
#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!(
            "\n\nThis project uses strict TDD via tdd-ratchet.\n\
             Do not run `cargo test` directly.\n\
             Run `cargo ratchet` instead.\n\n"
        );
    }
}
"#,
    )
    .unwrap();

    // A real test
    fs::write(
        dir.join("tests/real_test.rs"),
        r#"
#[test]
fn something_useful() {
    assert_eq!(1 + 1, 2);
}
"#,
    )
    .unwrap();
}

#[test]
fn cargo_test_without_ratchet_env_fails_with_instructions() {
    let dir = TestDir::new();
    create_project_with_gatekeeper(dir.path());

    let output = Command::new("cargo")
        .arg("test")
        .current_dir(dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.path())
        .env("RUSTUP_HOME", rustup_home())
        .env("CARGO_HOME", cargo_home())
        .env_remove("TDD_RATCHET")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "cargo test should fail without TDD_RATCHET"
    );
    let stderr = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tdd-ratchet") || stderr.contains("cargo ratchet"),
        "Output should mention tdd-ratchet: {stderr}"
    );
    dir.pass();
}

#[test]
fn cargo_test_with_ratchet_env_passes_gatekeeper() {
    let dir = TestDir::new();
    create_project_with_gatekeeper(dir.path());

    let output = Command::new("cargo")
        .arg("test")
        .current_dir(dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.path())
        .env("RUSTUP_HOME", rustup_home())
        .env("CARGO_HOME", cargo_home())
        .env("TDD_RATCHET", "1")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "cargo test should pass with TDD_RATCHET=1: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    dir.pass();
}
