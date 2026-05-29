use help_test::HelpTest;

#[test]
fn help_examples() {
    HelpTest::new("cargo-ratchet")
        .display_command(&["cargo", "ratchet"])
        .page(&[], |fixture| {
            fixture.file(
                "Cargo.toml",
                "[package]\nname = \"help-example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            );
            fixture.file(
                "tests/gatekeeper.rs",
                "#[test]\nfn tdd_ratchet_gatekeeper() {\n    if std::env::var(\"TDD_RATCHET\").is_err() {\n        panic!(\"Run tdd-ratchet instead of cargo test.\");\n    }\n}\n",
            );
            fixture.dir("src");
            fixture.file("src/lib.rs", "");
            fixture.env("PATH", std::env::var("PATH").expect("PATH should exist"));
            fixture.env("GIT_CONFIG_NOSYSTEM", "1");
            fixture.env("OPENSSL_NO_VENDOR", "1");
            fixture.env(
                "RUSTUP_HOME",
                std::env::var("RUSTUP_HOME").expect("RUSTUP_HOME should exist for help tests"),
            );
            fixture.env(
                "CARGO_HOME",
                std::env::var("CARGO_HOME").unwrap_or_else(|_| {
                    let home = std::env::var("HOME").expect("HOME should exist");
                    format!("{home}/.cargo")
                }),
            );
            fixture.command("git", &["init"]);
            fixture.command("git", &["config", "user.email", "help-test@example.com"]);
            fixture.command("git", &["config", "user.name", "Help Test"]);
            fixture.command("git", &["add", "Cargo.toml"]);
            fixture.command("git", &["add", "src/lib.rs"]);
            fixture.command("git", &["add", "tests/gatekeeper.rs"]);
            fixture.command("git", &["commit", "-m", "Initial project"]);
        })
        .run();
}
