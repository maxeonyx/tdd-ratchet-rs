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
cargo ratchet         # Verify the project's recorded test history still satisfies the ratchet
cargo ratchet --init  # Create .test-status.json from the current passing test set
```
