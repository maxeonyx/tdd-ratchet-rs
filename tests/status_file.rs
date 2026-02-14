// tests/status_file.rs
//
// Story 4: Committed status file tracking each test's expected state.
//
// The `.test-status.json` file is the ratchet's persistent state. It
// maps test names to their expected state (`pending` or `passing`).
// The ratchet reads it, compares against actual results, and updates it.
//
// Isolation: temp dirs for all file operations.
//
// Test cases:
// - Empty status file (no tests tracked) → valid, parses to empty map
// - Status file with pending and passing tests → loads correctly
// - Ratchet updates status file: new failing test → added as pending
// - Ratchet updates status file: pending test now passes → promoted
// - Round-trip: write then read → identical
//
// Edge cases:
// - Status file doesn't exist → created on first run (with --init or
//   automatically — define policy)
// - Status file is malformed JSON → clear error with line info
// - Status file has unknown fields → ignored (forward compatibility)
// - Test name with special characters (colons, spaces) → handled
