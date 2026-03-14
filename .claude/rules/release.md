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
11. Updates `HomebrewFormula/mdpaste.rb` with the new version and SHA-256 hashes
12. Commits and pushes the updated formula to `main`
13. Clones `daaa1k/homebrew-tap` and copies the formula to `Formula/mdpaste.rb`, then pushes

## Prerequisites

- `cargo`, `nix`, `gh` (GitHub CLI), `jq` must be in PATH
- `gh` must be authenticated (`gh auth login`)
- The working tree must be on `main` with no uncommitted changes
- The `daaa1k/homebrew-tap` repository must exist (create it once at https://github.com/new with name `homebrew-tap`)

## Setting up the Homebrew tap (first time)

1. Create a public repository named `homebrew-tap` under the `daaa1k` GitHub account.
2. Copy `HomebrewFormula/mdpaste.rb` into it as `Formula/mdpaste.rb` and push.
3. After that, `scripts/release.sh` will keep it up to date automatically.

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

If the script exits after updating `flake.nix` but before updating the Homebrew formula,
run the formula update manually:

```sh
TAG=vX.Y.Z
VERSION="${TAG#v}"

# Convert Nix SRI hash to hex sha256
sri_to_sha256() { echo "${1#sha256-}" | base64 -d | od -An -tx1 | tr -d ' \n'; }

LINUX_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "https://github.com/daaa1k/mdpaste/releases/download/${TAG}/mdpaste-linux-x86_64" | jq -r '.hash')
MACOS_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "https://github.com/daaa1k/mdpaste/releases/download/${TAG}/mdpaste-macos-aarch64" | jq -r '.hash')

LINUX_SHA256=$(sri_to_sha256 "$LINUX_HASH")
MACOS_SHA256=$(sri_to_sha256 "$MACOS_HASH")

sed -i "s/version \"[^\"]*\"/version \"${VERSION}\"/" HomebrewFormula/mdpaste.rb
sed -i "s|sha256 \"[a-f0-9]*\" # macos-aarch64|sha256 \"${MACOS_SHA256}\" # macos-aarch64|" HomebrewFormula/mdpaste.rb
sed -i "s|sha256 \"[a-f0-9]*\" # linux-x86_64|sha256 \"${LINUX_SHA256}\" # linux-x86_64|" HomebrewFormula/mdpaste.rb

git add HomebrewFormula/mdpaste.rb
git commit -m "chore(brew): update formula to ${TAG}"
git push origin main

gh repo clone daaa1k/homebrew-tap /tmp/homebrew-tap
cp HomebrewFormula/mdpaste.rb /tmp/homebrew-tap/Formula/mdpaste.rb
cd /tmp/homebrew-tap && git add Formula/mdpaste.rb && git commit -m "chore: update mdpaste to ${TAG}" && git push
```
