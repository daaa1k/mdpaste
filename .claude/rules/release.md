# Release Checklist

## Releasing a new version

Run the release script with the new version number:

```sh
scripts/release.sh <version>   # e.g. scripts/release.sh 0.5.0
```

The script handles the following automatically:

1. Validates that the working tree is clean and on `main`
2. Bumps the version in `Cargo.toml`
3. Rebuilds to update `Cargo.lock`
4. Runs `cargo fmt --check` and `cargo clippy`
5. Commits, creates the `vX.Y.Z` tag, and pushes both
6. Waits for the GitHub Actions release workflow to complete
7. Fetches the SHA-256 hashes of the uploaded binaries via `nix store prefetch-file`
8. Updates `binaryHashes` in `flake.nix`
9. Verifies with `nix flake check --no-build`
10. Commits and pushes the updated `flake.nix`

## Prerequisites

- `cargo`, `nix`, `gh` (GitHub CLI), `jq` must be in PATH
- `gh` must be authenticated (`gh auth login`)
- The working tree must be on `main` with no uncommitted changes

## If the workflow fails mid-way

If the script exits after the tag push but before updating `flake.nix`
(e.g. the release workflow failed), fix the root cause and re-run the
hash update manually:

```sh
TAG=vX.Y.Z
BASE_URL="https://github.com/daaa1k/mdpaste/releases/download/${TAG}"

LINUX_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "${BASE_URL}/mdpaste-linux-x86_64" | jq -r '.hash')
MACOS_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "${BASE_URL}/mdpaste-macos-aarch64" | jq -r '.hash')

sed -i "s|\"x86_64-linux\"   = \"sha256-.*\"|\"x86_64-linux\"   = \"${LINUX_HASH}\"|" flake.nix
sed -i "s|\"aarch64-darwin\" = \"sha256-.*\"|\"aarch64-darwin\" = \"${MACOS_HASH}\"|" flake.nix
```
