// tests/bypass_prevention.rs
//
// Story 8: `cargo test` run directly should fail with instructions.
//
// The ratchet must prevent tests from being run outside the ratchet.
// The known approach is a gatekeeper test checking `TDD_RATCHET=1` env
// var, but there may be better approaches.
//
// Isolation: creates a temp Rust project and runs `cargo test` both
// with and without the ratchet.
//
// Test cases:
// - `cargo test` without ratchet → fails, output contains instructions
//   on how to run via the ratchet
// - `cargo test` via ratchet → the bypass prevention doesn't trigger
//
// Edge cases:
// - Bypass prevention not set up in project → ratchet detects this
//   and tells the user how to set it up
