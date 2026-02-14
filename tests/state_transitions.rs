// tests/state_transitions.rs
//
// Stories 5, 6, 7: The core ratchet rules — new tests must fail first,
// passing tests must stay passing, tracked tests must not disappear.
//
// These test the ratchet's comparison logic: given a status file and
// a set of test results, what does the ratchet accept or reject?
//
// This is the pure logic layer — no git repos, no cargo invocations,
// no filesystem. Input: (status map, test results). Output: accept/reject
// with reasons.
//
// Test cases:
// - New test that fails → accepted, added as pending
// - New test that passes → rejected (story 5)
// - Pending test that now passes → accepted, promoted to passing
// - Pending test still failing → accepted, stays pending
// - Passing test still passing → accepted
// - Passing test now fails → rejected (story 6)
// - Test in status file but not in results → rejected (story 7)
// - Test in results but not in status file (new) → handled per above
//
// Edge cases:
// - Multiple violations in one run → all reported, not just first
// - Empty status file + all tests pass → all rejected (none were pending)
// - Empty status file + all tests fail → all accepted as pending
// - Empty results + non-empty status file → all tracked tests rejected
//   as missing
