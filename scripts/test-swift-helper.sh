#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

swiftc \
  -parse-as-library \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/CommandLineOptions.swift" \
  "$ROOT_DIR/swift-helper/Sources/LivePolyTransHelper/LanguageDetection.swift" \
  "$ROOT_DIR/swift-helper/TestSupport/CommandLineOptionsTests.swift" \
  -o "$TMP_DIR/command-line-options-tests"

"$TMP_DIR/command-line-options-tests"
