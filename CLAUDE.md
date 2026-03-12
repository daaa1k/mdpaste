# mdpaste Development Guide

## Build

```sh
cargo build
```

## Code Quality Checks (Required)

After any code change, run the following and confirm there are no errors.

### Format

```sh
cargo fmt
```

### Clippy (treat warnings as errors)

```sh
cargo clippy -- -D warnings
```

- `dead_code` warning: functions only called from within a `#[cfg(...)]` block must have the same `#[cfg(...)]` attribute
- `manual_split_once` warning: rewrite `splitn(2, x).nth(1)` as `split_once(x).map(|t| t.1)`
- All other Clippy warnings must be fixed; suppressing with `#[allow(...)]` is a last resort
