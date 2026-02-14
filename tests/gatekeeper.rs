// tests/gatekeeper.rs
//
// Bypass prevention: ensures cargo test is not run directly.
// This test panics unless TDD_RATCHET=1 is set, which the ratchet
// binary sets before invoking cargo test.

#[test]
fn tdd_ratchet_gatekeeper() {
    if std::env::var("TDD_RATCHET").is_err() {
        panic!(
            "\n\n\
             This project uses strict TDD via tdd-ratchet.\n\
             Do not run `cargo test` directly.\n\
             Run `cargo run --` or the installed `tdd-ratchet` binary instead.\n\
             \n"
        );
    }
}
