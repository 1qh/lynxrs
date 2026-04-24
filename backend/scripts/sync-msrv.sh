#!/usr/bin/env bash
# Sync rust-version in Cargo.toml to whatever rust:alpine currently ships.
# Run from `just deps-bump` or manually before cargo upgrade.
set -euo pipefail

RUST_IMAGE="${RUST_IMAGE:-rust:alpine}"
docker pull "$RUST_IMAGE" >/dev/null
VER="$(docker run --rm "$RUST_IMAGE" rustc --version | awk '{print $2}' | cut -d. -f1,2)"
if [[ ! "$VER" =~ ^[0-9]+\.[0-9]+$ ]]; then
  echo "could not parse rustc version from $RUST_IMAGE (got: $VER)" >&2
  exit 1
fi

cd "$(dirname "$0")/.."
if grep -qE '^rust-version = ' Cargo.toml; then
  # Portable in-place edit (avoids BSD/GNU `sed -i` divergence).
  tmp="$(mktemp)"
  sed -E "s|^rust-version = \"[0-9.]+\"|rust-version = \"$VER\"|" Cargo.toml > "$tmp"
  mv "$tmp" Cargo.toml
  echo "rust-version → $VER"
else
  echo "no rust-version in Cargo.toml — nothing to update" >&2
fi
