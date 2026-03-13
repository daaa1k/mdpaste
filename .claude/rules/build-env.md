# Build Environment

## macOS Notice

On macOS 26 (Darwin 26.x) and later, running `cargo build` with the system Rust toolchain
fails because the `aws-lc-sys` build script crashes with SIGABRT.

**Workaround: use the Nix dev shell**

```sh
nix develop --command cargo build
nix develop --command cargo clippy -- -D warnings
nix develop --command cargo test
nix develop --command cargo fmt
```

All development on macOS must be done through `nix develop`.
