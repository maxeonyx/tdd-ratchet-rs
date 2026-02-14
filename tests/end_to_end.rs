// tests/end_to_end.rs
//
// Story 1: As a user of tdd-ratchet, I want my Rust project to enforce
// strict TDD — new tests must fail before they can pass, verified by
// git history.
//
// This is the big black-box test. We create a temporary Rust project
// with a git repo, simulate the full TDD workflow, and verify the
// ratchet enforces the rules at each step.
//
// Isolation: each test creates a temp dir containing a complete Rust
// project with Cargo.toml, src/, tests/, and a git repo. HOME and
// GIT_CONFIG_NOSYSTEM are set. Nothing outside the temp dir is touched.
//
// The workflow simulated:
// 1. Init project + ratchet, commit
// 2. Add a failing test, run ratchet → succeeds, test tracked as pending
// 3. Commit the status file
// 4. Make the test pass, run ratchet → succeeds, test promoted to passing
// 5. Commit
// 6. Verify: adding a test that passes immediately → ratchet rejects
// 7. Verify: removing a tracked test → ratchet rejects
// 8. Verify: regressing a passing test → ratchet rejects
//
// Test cases:
// - Full happy-path TDD workflow → ratchet succeeds at each step
// - Skip the pending step (test passes immediately) → ratchet rejects
// - Remove a tracked test without removing from status file → rejects
// - Passing test regresses → rejects
//
// Edge cases:
// - Project with zero tests → ratchet succeeds (nothing to enforce)
// - Two tests added in same commit, one fails one passes → the passing
//   one is rejected, the failing one is accepted
