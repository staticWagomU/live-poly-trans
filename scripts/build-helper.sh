#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="$(uname -m)"

case "$ARCH" in
  arm64) TARGET_SUFFIX="aarch64-apple-darwin" ;;
  x86_64) TARGET_SUFFIX="x86_64-apple-darwin" ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

swift build -c release --package-path "$ROOT_DIR/swift-helper"
mkdir -p "$ROOT_DIR/src-tauri/binaries"
cp \
  "$ROOT_DIR/swift-helper/.build/release/live-poly-trans-helper" \
  "$ROOT_DIR/src-tauri/binaries/helper-$TARGET_SUFFIX"
