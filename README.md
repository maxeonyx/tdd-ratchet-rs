# tdd-ratchet

TDD ratchet for pure Rust projects — enforces failing-first test workflow via git history.

## What it does

A dev dependency binary that wraps `cargo test` / `cargo nextest`. It reads ratchet input from the committed `.test-status.json` ledger, writes a local preview, and enforces that new tests must fail before they can pass by inspecting git history. On pull requests, the trusted ledger workflow validates the change and records the preview with a dedicated bot commit.

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
cargo ratchet --help
cargo ratchet --version
```

Trusted automation that cannot observe a required external dependency may preserve an exact result already recorded as passing with `--preserve-passing <TEST>`. The flag is repeatable, keeps the test present as ignored for this run, and rejects pending, unknown, absent, or duplicate names. Keep this allowlist in reviewed workflow source; it is not a way to accept new or regressed behavior.

Use `cargo ratchet --init` once, before enabling the trusted ledger workflow, to create the repository's adoption snapshot. After adoption, treat `.test-status.json` as bot-written output: commit test and implementation changes separately, push each state to a pull request, and wait for the trusted ledger workflow to record `pending` before implementing and `passing` afterward.

Declare a deliberate rename or removal in `.tdd-ratchet.json`. The ratchet validates that instruction, records any rename bridge in the ledger, and keeps removals out of the ledger schema. Remove the instruction file after the bot records the transition.

The writer runs for same-repository pull requests. For a fork contribution, a maintainer must first move the commits onto a repository branch so the trusted workflow can validate and record the ledger without granting the fork a write path.

Do not run `cargo test` directly. The ratchet will ask you to add a "gatekeeper" test to enforce this.

## Developing

```
cargo ratchet
```

Prerequisites: Rust toolchain.
