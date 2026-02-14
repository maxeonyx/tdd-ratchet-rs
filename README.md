# tdd-ratchet

TDD ratchet for pure Rust projects — enforces failing-first test workflow via git history.

## What it does

A dev dependency binary that wraps `cargo test` / `cargo nextest`. It tracks per-test states in a committed `.test-status.json` and enforces that new tests must fail before they can pass, verified by git history introspection.

See [VISION.md](VISION.md) for full requirements and [PLAN.md](PLAN.md) for stories and design decisions.

## Install

Not yet released. Will be installable as a dev dependency:

```toml
[dev-dependencies]
tdd-ratchet = { git = "https://github.com/maxeonyx/tdd-ratchet-rs" }
```

## Usage

```
cargo ratchet
```

Do not run `cargo test` directly — the ratchet enforces this.

## Developing

```
cargo test
```

Prerequisites: Rust toolchain.
