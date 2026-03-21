# tdd-ratchet

TDD ratchet for pure Rust projects — enforces failing-first test workflow via git history.

## What it does

A dev dependency binary that wraps `cargo test` / `cargo nextest`. It reads ratchet input from the committed `.test-status.json` in git history, writes the updated status file to the working tree, and enforces that new tests must fail before they can pass, verified by git history introspection.

See [VISION.md](VISION.md) for full requirements and [PLAN.md](PLAN.md) for stories and design decisions.

## Install

```
cargo install tdd-ratchet
```

This installs the `cargo-ratchet` binary, enabling `cargo ratchet` as a subcommand.

Alternative (bare binary release):

```
curl -Lo ~/.local/bin/cargo-ratchet https://tdd-ratchet.maxeonyx.com/releases/cargo-ratchet-x86_64-linux
chmod +x ~/.local/bin/cargo-ratchet
```

## Usage

```
cargo ratchet
```

On the first run in a project, `cargo ratchet` treats the status as empty if no committed `.test-status.json` exists yet. It writes the updated `.test-status.json` to the working tree; commit that file along with your code changes so the next run reads it from `HEAD`.

Do not run `cargo test` directly — the ratchet enforces this.

## Developing

```
cargo test
```

Prerequisites: Rust toolchain.
