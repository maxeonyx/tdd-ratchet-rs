// tests/error_messages.rs
//
// Story 9: Ratchet-specific failures explain the context (this project
// uses strict TDD via tdd-ratchet), what the problem is, and what to
// do about it.
//
// This is a forward check — it should discover all ratchet-specific
// error paths in the codebase and verify each one includes context,
// problem, and suggestion. Regular test regressions (story 6) just
// show the test failure — the ratchet-specific errors are the ones
// about TDD workflow violations.
//
// Ratchet-specific errors include:
// - New test passed without being pending first
// - Tracked test disappeared from run
// - Bypass prevention not set up
// - Status file missing or malformed
// - Git history shows a test skipped pending state
//
// Each error message must include (in roughly one line each):
// 1. Context: "This project uses strict TDD (tdd-ratchet)."
// 2. Problem: what specifically went wrong
// 3. Suggestion: what to do about it
//
// The test should scan error construction sites in the codebase and
// verify the pattern, not maintain a list of known messages. Exceptions
// must be justified.
