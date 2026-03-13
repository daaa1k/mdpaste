# mdpaste Development Guide

## Build

```sh
nix develop --command cargo build
```

> **macOS 26+**: The system Rust toolchain fails to build `aws-lc-sys`.
> Always use `nix develop`. See `.claude/rules/build-env.md` for details.

## Code Quality Checks (Required)

After any code change, run the following and confirm there are no errors.

### Format

```sh
nix develop --command cargo fmt
```

### Clippy (treat warnings as errors)

```sh
nix develop --command cargo clippy -- -D warnings
```

- `dead_code` warning: functions only called from within a `#[cfg(...)]` block must have the same `#[cfg(...)]` attribute
- `manual_split_once` warning: rewrite `splitn(2, x).nth(1)` as `split_once(x).map(|t| t.1)`
- All other Clippy warnings must be fixed; suppressing with `#[allow(...)]` is a last resort

## Rules for Rust Files

### Refactoring

When editing any Rust-related file (`.rs`, `Cargo.toml`, `Cargo.lock`), use the `rust-engineer` skill to perform an overall refactoring review of the changed code.

### Test Coverage

When editing any Rust-related file (`.rs`), add or update tests so that the overall test coverage remains at **80% or above**.

## Nix Flake Maintenance

### Releasing a new version

Use the release script (see `.claude/rules/release.md` for full details):

```sh
scripts/release.sh <version>   # e.g. scripts/release.sh 0.5.0
```

### Verifying the flake

```sh
nix flake check --no-build   # evaluate all outputs
nix build .#mdpaste-bin      # verify pre-built binary package
```
