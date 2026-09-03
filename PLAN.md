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

### New user stories

12. ~~As a user of tdd-ratchet, I want to rename tests without the ratchet treating the new name as a brand-new test. A `renames` section in `.tdd-ratchet.json` declares `old_name → new_name` mappings. The ratchet validates that the old name existed and the new name appears in test results, then transfers the state and records the bridge in the generated ledger.~~ ✅
13. ~~As a user of tdd-ratchet, I want the status file in my working tree to be _output only_ — the ratchet reads its input from the last committed `.test-status.json` in git history (or the earliest commit containing it), not from the working tree. This prevents bypassing the ratchet by manually editing the status file. The baseline concept may be simplified or eliminated — if the ratchet walks back to the first commit that contains `.test-status.json`, that _is_ the baseline.~~ ✅
14. ~~As a user of tdd-ratchet, I want the ratchet output to be self-documenting. When a violation occurs, it should explain: (a) why the ratchet exists (enforcing test-first discipline), (b) what the specific violation is, (c) what to do about it (e.g. rebase tests and implementation into separate commits). A first-time user encountering the ratchet should understand it without reading external docs.~~ ✅

15. ~~As a user of tdd-ratchet, I want to intentionally remove tests without the ratchet blocking me. A `removals` list in `.tdd-ratchet.json` declares test names to retire. The ratchet validates each removal, removes the entry from generated ledger output, and rejects undeclared disappearances as before.~~ ✅
16. ~~As a user of tdd-ratchet, I want failed Cargo/Nextest execution and invalid CLI options rejected before the trusted ledger is evaluated or written, with the original process evidence and relevant recovery preserved.~~ ✅
17. ~~As a user of tdd-ratchet, I want every observed failure to remain a failure, with no runner-specific result-preservation escape hatch; external evidence that a safe runner cannot observe belongs in a separate attestation system.~~ ✅

### Developer stories

10. ~~As a developer of tdd-ratchet, I want `git clone` + `{rust toolchain}` to give me a working dev environment.~~ ✅
11. ~~As a developer of tdd-ratchet, I want CI to run the ratchet's own tests.~~ ✅

## State Machine

```
(not in file) ──[new test fails]──▶ pending ──[passes]──▶ passing
                                       │                     │
                                       ▼                     ▼
                               [still fails: ok]    [still passes: ok]

pending ──[intentional removal]──▶ (not in file)
passing ──[intentional removal]──▶ (not in file)
```

Each transition requires a separate commit. Verified by git history. Intentional removal uses the `removals` instruction channel (story 15).

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

Developer instructions live separately in `.tdd-ratchet.json`:

```json
{
  "renames": {
    "test_module::new_name": "test_module::old_name"
  },
  "removals": [
    "test_module::retired_test"
  ]
}
```

The `renames` mapping bridges new name → old name and is copied into successful ledger output as historical evidence. The `removals` list is transient and never enters the ledger. Remove consumed instructions after the trusted workflow records the result.

## Ratchet Algorithm

1. Set `TDD_RATCHET=1` (or equivalent bypass mechanism)
2. Run `cargo test` / `cargo nextest`, collect per-test pass/fail
3. Compare results against the committed `.test-status.json` from `HEAD` (or empty status on first run):
   - New test that fails → add as `pending` (ok)
   - New test that passes → **reject** (must fail first)
   - `pending` test that now passes → promote to `passing` (ok)
   - `pending` test that still fails → ok
   - `passing` test that still passes → ok
   - `passing` test that now fails → **reject** (regression)
   - Test in status file but not in run → **reject** (silent removal), unless declared in `removals` (story 15)
4. Inspect git history to verify no test skipped the `pending` state
5. Write a local `.test-status.json` preview; on pull requests, the isolated trusted writer validates and commits it
6. Exit 0 if all rules pass, non-zero otherwise

## Design Decisions

### Test runner parsing

The ratchet needs per-test pass/fail results. `cargo test` verbose output prints `test name ... ok/FAILED` — parse with regex. `cargo nextest` has structured output which may be easier. Support both, detect which is available.

### Git history and trusted adoption

The first committed `.test-status.json` is the repository's one adoption snapshot. Tests already present there are trusted; every later test must appear as `pending` before `passing`. There is no movable baseline field and no second adoption. Deleting the ledger, recreating it, rewriting `passing` to `pending`, or removing an old violation does not repair history; only rewriting the offending commits does.

The ratchet reads tracked state from committed history and writes a local preview. On same-repository pull requests, an unprivileged validation job builds the enforcing binary from base-controlled source, runs it against a separate pull-request checkout, and uploads only the preview. A separate writer job, without a source checkout, revalidates transition semantics and commits exactly `.test-status.json` against the verified head. Ordinary pull-request commits that touch the ledger are rejected. Developer rename and removal intent comes from `.tdd-ratchet.json`. Fork commits must be moved onto a maintainer-controlled repository branch before this writer runs.

### Bypass prevention discussion

The ratchet must prevent `cargo test` from being run directly. Options considered:

1. **Gatekeeper test with env var** — a test in the consumer project that checks `TDD_RATCHET=1` and panics with instructions if not set. Simple, but requires the consumer to add a test manually.
2. **Other approaches** — the implementation agent should explore alternatives (e.g. cargo runner config, build script checks).

The gatekeeper approach is the known-good option. The ratchet should check that the bypass prevention is in place and tell the user how to set it up if missing.

## Future Work

- Host a formal JSON Schema for `.test-status.json` on GitHub Pages at `tdd-ratchet.maxeonyx.com`
- Switch from `cargo test` stdout regex parsing to `cargo nextest` structured output (JUnit XML or libtest JSON). Nextest can be required — no need to support both. This would replace `src/runner.rs` entirely.
- Continue refining the three-phase architecture (Gather → Logic → Output) introduced during story 13. The gather phase now reads committed status from git and the logic phase applies ratchet rules, but history checking is still partially separate. Fully unifying ratchet rules and history rules into a single pure logic phase would be the next structural improvement.
