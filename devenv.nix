{ pkgs, ... }:

{
  packages = [
    # `cargo ratchet` must be this working tree's ratchet, not whatever
    # cargo-ratchet an agent or developer happens to have installed: a stale
    # binary would validate this repository against its own older rules.
    (pkgs.writeShellScriptBin "cargo-ratchet" ''
      exec cargo run --quiet --manifest-path "$DEVENV_ROOT/Cargo.toml" -- "$@"
    '')
    pkgs.actionlint
    pkgs.cargo
    pkgs.cargo-nextest
    pkgs.clippy
    pkgs.curl
    pkgs.gcc
    pkgs.gh
    pkgs.git
    pkgs.openssl
    pkgs.pkg-config
    pkgs.rustc
    pkgs.rustfmt
  ];

  enterTest = ''
    actionlint
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo ratchet
  '';
}
