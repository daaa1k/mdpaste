#!/usr/bin/env bash
# scripts/release.sh
#
# Release workflow:
# 1. bump Cargo version
# 2. commit + tag
# 3. push
# 4. wait for GitHub release workflow
# 5. fetch binary hashes
# 6. update flake.nix
#
# Usage:
#   scripts/release.sh 0.6.0

set -Eeuo pipefail

########################################
# error handling
########################################

trap 'echo "error: release failed at line $LINENO" >&2' ERR

########################################
# config
########################################

REPO="daaa1k/mdpaste"
WORKFLOW="release.yml"

VERSION="${1:?Usage: scripts/release.sh <version>}"
VERSION="${VERSION#v}"
TAG="v${VERSION}"

BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"

########################################
# helpers
########################################

require_cmd() {
  command -v "$1" >/dev/null || {
    echo "error: missing dependency: $1" >&2
    exit 1
  }
}

sedi() {
  if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "$@"
  else
    sed -i "$@"
  fi
}

prefetch_binary() {
  nix store prefetch-file --hash-type sha256 --json "$1" \
    | jq -r '.hash'
}

########################################
# dependency checks
########################################

for cmd in git cargo nix gh jq; do
  require_cmd "$cmd"
done

########################################
# git checks
########################################

BRANCH=$(git rev-parse --abbrev-ref HEAD)

if [[ "$BRANCH" != "main" ]]; then
  echo "error: must be on main (current: $BRANCH)" >&2
  exit 1
fi

if ! git diff --quiet HEAD; then
  echo "error: working tree has uncommitted changes" >&2
  exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "error: tag already exists: $TAG" >&2
  exit 1
fi

########################################
# release start
########################################

echo "==> Releasing ${TAG}"

########################################
# bump Cargo.toml
########################################

echo "==> Updating Cargo.toml version"

sedi "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

########################################
# update lockfile
########################################

echo "==> Updating Cargo.lock"

cargo build --quiet >/dev/null

########################################
# quality checks
########################################

echo "==> Running checks"

cargo fmt --check
cargo clippy -- -D warnings

########################################
# commit and tag
########################################

echo "==> Commit and tag"

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to ${VERSION}"

COMMIT_SHA=$(git rev-parse HEAD)

git tag "$TAG"

echo "==> Push"

git push origin main "$TAG"

########################################
# wait for workflow
########################################

echo "==> Waiting for GitHub Actions run"

RUN_ID=""

for i in {1..20}; do
  RUN_ID=$(gh run list \
    --repo "$REPO" \
    --workflow "$WORKFLOW" \
    --json databaseId,headSha \
    --jq ".[] | select(.headSha==\"$COMMIT_SHA\") | .databaseId" \
    | head -n1)

  if [[ -n "$RUN_ID" ]]; then
    break
  fi

  sleep 5
done

if [[ -z "$RUN_ID" ]]; then
  echo "error: release workflow not found" >&2
  exit 1
fi

echo "==> Watching run $RUN_ID"

gh run watch "$RUN_ID" --repo "$REPO"

CONCLUSION=$(gh run view "$RUN_ID" \
  --repo "$REPO" \
  --json conclusion \
  --jq '.conclusion')

if [[ "$CONCLUSION" != "success" ]]; then
  echo "error: workflow failed: $CONCLUSION" >&2
  exit 1
fi

########################################
# fetch hashes
########################################

echo "==> Fetching binary hashes"

LINUX_HASH=$(prefetch_binary "${BASE_URL}/mdpaste-linux-x86_64")
MACOS_HASH=$(prefetch_binary "${BASE_URL}/mdpaste-macos-aarch64")

echo "  x86_64-linux   : $LINUX_HASH"
echo "  aarch64-darwin : $MACOS_HASH"

########################################
# update flake
########################################

echo "==> Updating flake.nix"

sedi "s|\"x86_64-linux\" *= *\"sha256-.*\"|\"x86_64-linux\" = \"${LINUX_HASH}\"|" flake.nix
sedi "s|\"aarch64-darwin\" *= *\"sha256-.*\"|\"aarch64-darwin\" = \"${MACOS_HASH}\"|" flake.nix

########################################
# verify flake
########################################

echo "==> Verifying flake"

nix flake check --no-build

########################################
# commit hashes
########################################

echo "==> Commit flake update"

git add flake.nix
git commit -m "chore(nix): update binary hashes for ${TAG}"

git push origin main

########################################
# update homebrew formula
########################################

echo "==> Updating Homebrew formula"

# Convert a Nix SRI hash (sha256-BASE64) to a lowercase hex SHA-256 digest.
sri_to_sha256() {
  echo "${1#sha256-}" | base64 -d | od -An -tx1 | tr -d ' \n'
}

MACOS_SHA256=$(sri_to_sha256 "$MACOS_HASH")
LINUX_SHA256=$(sri_to_sha256 "$LINUX_HASH")

echo "  aarch64-darwin : $MACOS_SHA256"
echo "  x86_64-linux   : $LINUX_SHA256"

FORMULA="HomebrewFormula/mdpaste.rb"

sedi "s/version \"[^\"]*\"/version \"${VERSION}\"/" "$FORMULA"
sedi "s|sha256 \"[a-f0-9]*\" # macos-aarch64|sha256 \"${MACOS_SHA256}\" # macos-aarch64|" "$FORMULA"
sedi "s|sha256 \"[a-f0-9]*\" # linux-x86_64|sha256 \"${LINUX_SHA256}\" # linux-x86_64|" "$FORMULA"

git add "$FORMULA"
git commit -m "chore(brew): update formula to ${TAG}"

git push origin main

########################################
# push to homebrew tap repo
########################################

echo "==> Pushing to Homebrew tap"

TAP_REPO="daaa1k/homebrew-tap"

if gh repo view "$TAP_REPO" >/dev/null 2>&1; then
  TAP_DIR=$(mktemp -d)

  gh repo clone "$TAP_REPO" "$TAP_DIR"

  mkdir -p "$TAP_DIR/Formula"
  cp "$FORMULA" "$TAP_DIR/Formula/mdpaste.rb"

  (
    cd "$TAP_DIR"
    git add Formula/mdpaste.rb
    git commit -m "chore: update mdpaste to ${TAG}"
    git push origin "$(git rev-parse --abbrev-ref HEAD)"
  )

  rm -rf "$TAP_DIR"

  echo "  Tap updated: https://github.com/${TAP_REPO}"
else
  echo "  warning: tap repo ${TAP_REPO} not found — skipping"
  echo "  Create it at https://github.com/new with name 'homebrew-tap'"
  echo "  Then copy HomebrewFormula/mdpaste.rb to Formula/mdpaste.rb in the tap repo"
fi

########################################
# done
########################################

echo ""
echo "Release completed: ${TAG}"
echo "https://github.com/${REPO}/releases/tag/${TAG}"
