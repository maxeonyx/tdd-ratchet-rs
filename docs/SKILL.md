---
name: tdd-ratchet
description: When working in a Rust project that uses strict TDD enforcement
---

# tdd-ratchet

Use `tdd-ratchet` to enforce the rule that new tests must fail first before they can later pass.

## Install

Download the latest release from https://tdd-ratchet.maxeonyx.com/releases and place `cargo-ratchet` on your `PATH`.

```bash
curl -Lo ~/.cargo/bin/cargo-ratchet https://tdd-ratchet.maxeonyx.com/releases/cargo-ratchet-x86_64-linux
chmod +x ~/.cargo/bin/cargo-ratchet
```

## Usage

```bash
cargo ratchet         # Verify history and preview the next trusted ledger state
cargo ratchet --init  # One-time adoption snapshot, before enabling the trusted workflow
```

After adoption, do not commit `.test-status.json` by hand. Push each red or green code commit to a pull request and let the trusted ledger workflow record the corresponding status. Put deliberate test `renames` or `removals` in `.tdd-ratchet.json`.
