# tdd-ratchet — Plan

## Stories

### Big story
1. ~~As a user of tdd-ratchet, I want my Rust project to enforce strict TDD — new tests must fail before they can pass, verified by git history.~~ ✅

### User stories
2. ~~As a user of tdd-ratchet, I want easy installation into my Rust project.~~ ✅
3. ~~As a user of tdd-ratchet, I want the ratchet to be transparent — I control my test harness naturally and the ratchet wraps it without getting in the way.~~ ✅
4. ~~As a user of tdd-ratchet, I want a committed status file tracking each test's expected state (`pending` or `passing`).~~ ✅
5. ~~As a user of tdd-ratchet, I want new tests rejected if they pass on their first appearance — they must be `pending` in a prior commit, verified by git history.~~ ✅
6. ~~As a user of tdd-ratchet, I want tests in `passing` state that now fail to fail the ratchet.~~ ✅
7. ~~As a user of tdd-ratchet, I want the ratchet to fail if a tracked test disappears from the run.~~ ✅
8. ~~As a user of tdd-ratchet, I want `cargo test` run directly (bypassing the ratchet) to fail with instructions. The gatekeeper-test-with-env-var is one approach; there may be better ones.~~ ✅
9. ~~As a user of tdd-ratchet, I want ratchet-specific failures to explain the context (this project uses strict TDD via tdd-ratchet), what the problem is, and what to do about it.~~ ✅

### Developer stories
10. ~~As a developer of tdd-ratchet, I want `git clone` + `{rust toolchain}` to give me a working dev environment.~~ ✅
11. ~~As a developer of tdd-ratchet, I want CI to run the ratchet's own tests.~~ ✅

## State Machine

```
(not in file) ──[new test fails]──▶ pending ──[passes]──▶ passing
                                       │                     │
                                       ▼                     ▼
                               [still fails: ok]    [still passes: ok]
```

Each transition requires a separate commit. Verified by git history.

## Status File

`.test-status.json`, committed to the repo:

```json
{
  "tests": {
    "test_module::test_name": "passing",
    "test_module::another_test": "pending"
  }
}
```

## Ratchet Algorithm

1. Set `TDD_RATCHET=1` (or equivalent bypass mechanism)
2. Run `cargo test` / `cargo nextest`, collect per-test pass/fail
3. Compare results against `.test-status.json`:
   - New test that fails → add as `pending` (ok)
   - New test that passes → **reject** (must fail first)
   - `pending` test that now passes → promote to `passing` (ok)
   - `pending` test that still fails → ok
   - `passing` test that still passes → ok
   - `passing` test that now fails → **reject** (regression)
   - Test in status file but not in run → **reject** (silent removal)
4. Inspect git history to verify no test skipped the `pending` state
5. Update `.test-status.json`
6. Exit 0 if all rules pass, non-zero otherwise

## Design Decisions

### Test runner parsing

The ratchet needs per-test pass/fail results. `cargo test` verbose
output prints `test name ... ok/FAILED` — parse with regex. `cargo
nextest` has structured output which may be easier. Support both,
detect which is available.

### Git history baseline

For new projects: baseline is the initial commit, check all history.
For existing projects: baseline is the commit where the ratchet was
added, earlier tests are grandfathered.

### Bypass prevention discussion

The ratchet must prevent `cargo test` from being run directly. Options
considered:

1. **Gatekeeper test with env var** — a test in the consumer project
   that checks `TDD_RATCHET=1` and panics with instructions if not set.
   Simple, but requires the consumer to add a test manually.
2. **Other approaches** — the implementation agent should explore
   alternatives (e.g. cargo runner config, build script checks).

The gatekeeper approach is the known-good option. The ratchet should
check that the bypass prevention is in place and tell the user how to
set it up if missing.

## Future Work

- Host a formal JSON Schema for `.test-status.json` on GitHub Pages at `tdd-ratchet.maxeonyx.com`
- Switch from `cargo test` stdout regex parsing to `cargo nextest` structured output (JUnit XML or libtest JSON). Nextest can be required — no need to support both. This would replace `src/runner.rs` entirely.
- Refactor main pipeline into clean three-phase architecture:
  1. **Gather** — load status file from disk, run tests, walk git history snapshots. All inputs collected upfront.
  2. **Logic** — pure function over all gathered data. Applies ratchet rules AND history rules together. Produces updated status file + violations list.
  3. **Output** — always save updated status file (valid transitions apply even when there are violations), then report violations and exit non-zero if any.
  Currently the phases are interleaved: ratchet logic runs, then history is checked separately, and the status file is only saved if everything passes. This means valid state transitions (e.g. new pending tests) are lost on any violation.
