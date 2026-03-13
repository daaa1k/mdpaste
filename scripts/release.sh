#!/usr/bin/env bash
# scripts/release.sh — bump version, tag, push, and update Nix binary hashes
#
# Usage:
#   scripts/release.sh <version>   e.g.  scripts/release.sh 0.5.0
#
# Prerequisites: cargo, nix, gh (GitHub CLI), jq

set -euo pipefail

# ── Argument ──────────────────────────────────────────────────────────────────

VERSION="${1:?Usage: scripts/release.sh <version>  (e.g. 0.5.0)}"
VERSION="${VERSION#v}"   # strip leading 'v' if present
TAG="v${VERSION}"

REPO="daaa1k/mdpaste"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"

# ── Pre-flight checks ─────────────────────────────────────────────────────────

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
  echo "error: must be on main (current: $BRANCH)" >&2
  exit 1
fi

if ! git diff --quiet HEAD; then
  echo "error: working tree has uncommitted changes" >&2
  exit 1
fi

if git rev-parse "$TAG" &>/dev/null; then
  echo "error: tag $TAG already exists" >&2
  exit 1
fi

echo "==> Releasing ${TAG}"

# ── 1. Bump version in Cargo.toml ─────────────────────────────────────────────

sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
echo "  Cargo.toml: version = \"${VERSION}\""

# ── 2. Update Cargo.lock ──────────────────────────────────────────────────────

cargo build --quiet 2>&1 | grep -v "^$" || true

# ── 3. Quality checks ─────────────────────────────────────────────────────────

echo "==> Running quality checks"
cargo fmt --check
cargo clippy -- -D warnings

# ── 4. Commit, tag, push ──────────────────────────────────────────────────────

echo "==> Committing and tagging"
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to ${VERSION}"
git tag "${TAG}"
git push origin main "${TAG}"

# ── 5. Wait for release workflow to complete ──────────────────────────────────

echo "==> Waiting for release workflow to be triggered..."
sleep 10

RUN_ID=$(gh run list --repo "$REPO" --workflow release.yml \
  --limit 1 --json databaseId --jq '.[0].databaseId')

echo "==> Watching run ${RUN_ID}..."
gh run watch "$RUN_ID" --repo "$REPO"

# Confirm the workflow succeeded before continuing
RUN_STATUS=$(gh run view "$RUN_ID" --repo "$REPO" --json conclusion --jq '.conclusion')
if [[ "$RUN_STATUS" != "success" ]]; then
  echo "error: release workflow finished with status '${RUN_STATUS}'" >&2
  echo "       Fix the issue, then run scripts/release-hashes.sh ${TAG}" >&2
  exit 1
fi

# ── 6. Fetch binary hashes ────────────────────────────────────────────────────

echo "==> Fetching binary hashes"
LINUX_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "${BASE_URL}/mdpaste-linux-x86_64" | jq -r '.hash')
MACOS_HASH=$(nix store prefetch-file --hash-type sha256 --json \
  "${BASE_URL}/mdpaste-macos-aarch64" | jq -r '.hash')

echo "  x86_64-linux:   ${LINUX_HASH}"
echo "  aarch64-darwin: ${MACOS_HASH}"

# ── 7. Update flake.nix ───────────────────────────────────────────────────────

echo "==> Updating flake.nix binaryHashes"
sed -i \
  "s|\"x86_64-linux\"   = \"sha256-.*\"|\"x86_64-linux\"   = \"${LINUX_HASH}\"|" \
  flake.nix
sed -i \
  "s|\"aarch64-darwin\" = \"sha256-.*\"|\"aarch64-darwin\" = \"${MACOS_HASH}\"|" \
  flake.nix

# ── 8. Verify flake evaluation ────────────────────────────────────────────────

echo "==> Verifying flake"
nix flake check --no-build

# ── 9. Commit and push ────────────────────────────────────────────────────────

echo "==> Committing updated flake.nix"
git add flake.nix
git commit -m "chore(nix): update binary hashes for ${TAG}"
git push origin main

echo ""
echo "Released ${TAG} successfully."
echo "  GitHub Release: https://github.com/${REPO}/releases/tag/${TAG}"
