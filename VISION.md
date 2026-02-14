# tdd-ratchet — Vision

## The Problem

Test-first discipline is easy to skip. Nothing stops a developer (or AI
agent) from writing the implementation first and tests after — or writing
tests that pass immediately. The result is tests that verify what was
built rather than specifying what should be built.

## What tdd-ratchet Does

A binary added as a dev dependency to pure Rust projects. It:

1. Wraps `cargo test` (or `cargo nextest`), collecting per-test results
2. Tracks expected test states in a committed `.test-status.json`
3. Enforces state transitions via git history — a test must appear as
   `pending` (failing) in a commit before it can be `passing`
4. Prevents running tests outside the ratchet — `cargo test` run
   directly should fail with instructions

## Design Goals

- **Transparent** — the developer controls their test harness naturally.
  The ratchet wraps it without getting in the way.
- **Easy installation** — adding the ratchet to a project should be
  minimal friction.
- **Pure Rust projects only** — mixed-language projects need a different
  approach.

## Bypass Prevention

Running `cargo test` directly (bypassing the ratchet) should fail with
instructions on how to run via the ratchet. One approach is a gatekeeper
test that checks for a `TDD_RATCHET=1` env var set by the ratchet. There
may be better approaches — the implementation agent should consider
alternatives.
