# Build Environment

## macOS Notice

`aws-lc-sys` has been removed from the dependency tree (replaced `aws-sdk-s3` with
`rusty-s3`). The macOS 26 build failure that previously required the Nix dev shell is
no longer triggered by this project's dependencies.

Standard `cargo build` should now work on macOS 26+ with the system Rust toolchain.
The Nix dev shell is still the recommended workflow for reproducible builds:

```sh
nix develop --command cargo build
nix develop --command cargo clippy -- -D warnings
nix develop --command cargo test
nix develop --command cargo fmt
```
