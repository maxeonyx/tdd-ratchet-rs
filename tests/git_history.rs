// tests/git_history.rs
//
// Story 5 (enforcement mechanism): The ratchet inspects git history to
// verify that no test skipped the `pending` state — it must appear as
// failing in a prior commit before it can be passing.
//
// Isolation: creates temp git repos with scripted commit histories.
// HOME and GIT_CONFIG_NOSYSTEM set.
//
// Test cases:
// - Test appears as pending in commit N, passing in commit N+1 → ok
// - Test appears as passing in its first-ever commit → rejected
// - Test appears as pending in commit N, still pending in N+1,
//   passing in N+2 → ok
//
// Edge cases:
// - Baseline commit: tests before the baseline are grandfathered
// - New project with no baseline → all history checked
// - Rebased history (commits reordered) → checks the actual history
// - Merge commit introducing a test → both parents checked
