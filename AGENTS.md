# tdd-ratchet — Agent Guide

Read [VISION.md](VISION.md) for what tdd-ratchet does and why.
Read [PLAN.md](PLAN.md) for stories, state machine, algorithm, and design decisions.

## Implementation workflow

1. **One story at a time.** Pick the next story from PLAN.md.
2. **Failing test first, separate commit.** Write the test, run it, confirm it fails for the right reason, commit.
3. **Implement to make it pass.** Commit when green.
4. **Update PLAN.md** after completing each story.
5. **Commit and push frequently.**

Note: tdd-ratchet cannot use itself until it's built. During development, run `TDD_RATCHET=1 cargo test` so the gatekeeper test passes. Once functional, dogfood it.

## Test isolation

Tests create temporary git repos to simulate consumer projects. Each
test must be hermetic:
- Create a temp directory for all state (fake project, status file)
- Set `GIT_CONFIG_NOSYSTEM=1` and `HOME` to the temp dir
- No ambient git config or real project state leaks in

## Key conventions

- Parse `cargo test` verbose output (`test name ... ok/FAILED`) with regex
- Git history inspection via `git2`
- Side effects (subprocess calls, filesystem) at the edges behind abstractions
- Status file input comes from committed git history (`HEAD`), not the working tree — the working tree status file is output only
- The exceptions: `renames` and `removals` sections are read from the working tree as instruction channels for the current run. `renames` persist in the committed output (identity bridge for history); `removals` are transient (dropped from output after application)
- Every violation message must be self-documenting: why the ratchet exists, what went wrong, what to do about it
