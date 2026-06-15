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
HELPER_SOURCE="$ROOT_DIR/swift-helper/.build/release/live-poly-trans-helper"
RAW_HELPER="$ROOT_DIR/src-tauri/binaries/helper-$TARGET_SUFFIX"
HELPER_APP="$ROOT_DIR/src-tauri/binaries/LivePolyTransHelper.app"
HELPER_APP_BINARY="$HELPER_APP/Contents/MacOS/live-poly-trans-helper"

cp \
  "$HELPER_SOURCE" \
  "$RAW_HELPER"

rm -rf "$HELPER_APP"
mkdir -p "$HELPER_APP/Contents/MacOS"
cp "$HELPER_SOURCE" "$HELPER_APP_BINARY"
cp "$ROOT_DIR/swift-helper/Info.plist" "$HELPER_APP/Contents/Info.plist"

if [[ "$(uname -s)" == "Darwin" && -x /usr/bin/codesign ]]; then
  /usr/bin/codesign \
    --force \
    --sign - \
    --identifier com.staticwagomu.live-poly-trans.helper \
    "$RAW_HELPER"

  /usr/bin/codesign \
    --force \
    --sign - \
    --identifier com.staticwagomu.live-poly-trans.helper \
    "$HELPER_APP"

  LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
  if [[ -x "$LSREGISTER" ]]; then
    "$LSREGISTER" -f "$HELPER_APP"
  fi
fi
