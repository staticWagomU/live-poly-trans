#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ "$(uname -s)" == "Darwin" && -x /usr/bin/xcrun ]]; then
  APPLE_DEVELOPER_ROOT="/Library/Developer/CommandLineTools"
  if [[ -d "$APPLE_DEVELOPER_ROOT/SDKs/MacOSX.sdk" ]]; then
    export DEVELOPER_DIR="$APPLE_DEVELOPER_ROOT"
    export SDKROOT="$APPLE_DEVELOPER_ROOT/SDKs/MacOSX.sdk"
  else
    export SDKROOT="$(env -u SDKROOT /usr/bin/xcrun --sdk macosx --show-sdk-path)"
  fi
fi

SDK_ARGS=()
if [[ -n "${SDKROOT:-}" ]]; then
  SDK_ARGS=(-sdk "$SDKROOT")
fi

swiftc \
  -parse-as-library \
  "${SDK_ARGS[@]}" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/CommandLineOptions.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/ISO8601Timestamp.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/JsonLine.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/LanguageDetection.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/LiveTranslationPolicy.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/ScreenCaptureKitSpeakerConfiguration.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/SegmentWriter.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/TranscriptEvent.swift" \
  "$ROOT_DIR/swift-helper/TestSupport/CommandLineOptionsTests.swift" \
  -o "$TMP_DIR/command-line-options-tests"

"$TMP_DIR/command-line-options-tests"
