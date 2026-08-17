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

12. ~~As a user of tdd-ratchet, I want to rename tests without the ratchet treating the new name as a brand-new test. A `renames` section in `.test-status.json` declares `old_name → new_name` mappings. The ratchet validates that the old name existed and the new name appears in test results, then transfers the state. After the rename commit, the ratchet warns that the renames section can be removed. If stale renames are left for more than one commit, the ratchet should warn (not error).~~ ✅
13. ~~As a user of tdd-ratchet, I want the status file in my working tree to be _output only_ — the ratchet reads its input from the last committed `.test-status.json` in git history (or the earliest commit containing it), not from the working tree. This prevents bypassing the ratchet by manually editing the status file. The baseline concept may be simplified or eliminated — if the ratchet walks back to the first commit that contains `.test-status.json`, that _is_ the baseline.~~ ✅
14. ~~As a user of tdd-ratchet, I want the ratchet output to be self-documenting. When a violation occurs, it should explain: (a) why the ratchet exists (enforcing test-first discipline), (b) what the specific violation is, (c) what to do about it (e.g. rebase tests and implementation into separate commits). A first-time user encountering the ratchet should understand it without reading external docs.~~ ✅

15. ~~As a user of tdd-ratchet, I want to intentionally remove tests without the ratchet blocking me. A `removals` list in the working-tree `.test-status.json` declares test names to retire. The ratchet validates each removal (name exists in committed status, test is absent from current results, no conflict with renames), removes the entry from the output status file, and rejects undeclared disappearances as before. Unlike `renames`, `removals` is transient — it's read from the working tree as an instruction for the current run and not persisted in the ratchet-generated output. Both `pending` and `passing` tests can be removed.~~ ✅

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

With renames (story 12) — temporary section, valid for one commit:

```json
{
  "tests": {
    "test_module::new_name": "passing",
    "test_module::another_test": "pending"
  },
  "renames": {
    "test_module::new_name": "test_module::old_name"
  }
}
```

The `renames` section maps new name → old name. After the rename commit, the ratchet warns that the section can be removed.

With removals (story 15) — temporary section, valid for one run:

```json
{
  "tests": {
    "test_module::other_test": "passing"
  },
  "removals": [
    "test_module::retired_test"
  ]
}
```

The `removals` section lists tests to retire. Unlike `renames`, it is transient — read from the working tree and not persisted in the output.

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
5. Update `.test-status.json`
6. Exit 0 if all rules pass, non-zero otherwise

## Design Decisions

### Test runner parsing

The ratchet needs per-test pass/fail results. `cargo test` verbose output prints `test name ... ok/FAILED` — parse with regex. `cargo nextest` has structured output which may be easier. Support both, detect which is available.

### Git history and the adoption baseline

There is no grandfathering. Every test earns its place by failing before it passes: the history walk requires each active test that appears as `passing` to have had a prior `pending` appearance.

The one sanctioned exception is the **adoption baseline** — a single immutable commit recorded in the top-level `baseline` field of `.test-status.json`. It names the last commit before the project began enforcing the ratchet. The first status snapshot at or after that commit is the _adoption snapshot_: everything in it (and before it) is trusted as "the suite as it stood at adoption"; every test first appearing as `passing` after the adoption snapshot must earn red→green. A project with no `baseline` is in bootstrap, where the first status snapshot is the adoption snapshot — reproducing the original first-snapshot trust.

The baseline is meant to stay fixed once set. A lightweight two-commit check guards it: read HEAD's baseline `B`, read the baseline of the commit `B` points at; if both exist and differ, the baseline was moved. This is a tripwire on an already-established baseline link, not a tamper-proof guarantee — a baseline pointed at a baseline-less commit is bootstrap, so the check intentionally does not catch establishing a new forward link.

The ratchet reads tracked test states from the committed version in `HEAD` (and further back for history validation), and re-injects the committed baseline into the written output each run so it survives. The deliberate exception is story 12: working-tree `renames` are an instruction channel for the current run, then are saved into the rename commit so history can see the identity bridge. This still prevents bypassing the ratchet by manually editing tracked test states in the working tree.

### Bypass prevention discussion

The ratchet must prevent `cargo test` from being run directly. Options considered:

1. **Gatekeeper test with env var** — a test in the consumer project that checks `TDD_RATCHET=1` and panics with instructions if not set. Simple, but requires the consumer to add a test manually.
2. **Other approaches** — the implementation agent should explore alternatives (e.g. cargo runner config, build script checks).

The gatekeeper approach is the known-good option. The ratchet should check that the bypass prevention is in place and tell the user how to set it up if missing.

## Future Work

- **Make history rewrite the ONLY way to fix a ratchet violation.** Today the baseline tripwire (see "Git history and the adoption baseline") intentionally does not catch establishing a new forward baseline link: pointing the baseline at a baseline-less commit is treated as bootstrap, so an agent can "fix" a violation by re-adopting / moving the baseline forward onto a fresh anchor instead of rewriting the offending history. That is the wrong fix and the tool currently permits it. The ratchet should reject every escape hatch — moving the baseline, re-`--init`, fresh adoption anchors that grandfather a new passing test — so the only sanctioned response to a violation is rewriting history so the test genuinely goes `pending` → `passing`. The single adoption baseline is allowed ONCE per repo (first adoption); after that, any attempt to re-baseline or forward-move must fail loudly with a message pointing at the history-rewrite procedure. This is the enforcement that would have prevented the trunc/agent-harness re-adoptions during the v1.0.0 rollout.
- Host a formal JSON Schema for `.test-status.json` on GitHub Pages at `tdd-ratchet.maxeonyx.com`
- Switch from `cargo test` stdout regex parsing to `cargo nextest` structured output (JUnit XML or libtest JSON). Nextest can be required — no need to support both. This would replace `src/runner.rs` entirely.
- Continue refining the three-phase architecture (Gather → Logic → Output) introduced during story 13. The gather phase now reads committed status from git and the logic phase applies ratchet rules, but history checking is still partially separate. Fully unifying ratchet rules and history rules into a single pure logic phase would be the next structural improvement.
