#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="$(uname -m)"

if [[ "$(uname -s)" == "Darwin" && -x /usr/bin/xcrun ]]; then
  APPLE_DEVELOPER_ROOT="/Library/Developer/CommandLineTools"
  if [[ -d "$APPLE_DEVELOPER_ROOT/SDKs/MacOSX.sdk" ]]; then
    export DEVELOPER_DIR="$APPLE_DEVELOPER_ROOT"
    export SDKROOT="$APPLE_DEVELOPER_ROOT/SDKs/MacOSX.sdk"
    export CC="$APPLE_DEVELOPER_ROOT/usr/bin/clang"
    export CXX="$APPLE_DEVELOPER_ROOT/usr/bin/clang++"
  else
    export SDKROOT="$(env -u SDKROOT /usr/bin/xcrun --sdk macosx --show-sdk-path)"
    export CC="$(env -u SDKROOT /usr/bin/xcrun --find clang)"
    export CXX="$(env -u SDKROOT /usr/bin/xcrun --find clang++)"
  fi
fi

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
