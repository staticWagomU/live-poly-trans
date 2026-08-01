#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT_DIR/src-tauri/binaries"
SRC_DIR="$ROOT_DIR/src-tauri/native"
mkdir -p "$OUT_DIR"

case "$(uname -s)" in
  Darwin)
    OUTPUT="$OUT_DIR/liblpt_whisper_backend.dylib"
    INCLUDE_DIR="${WHISPER_CPP_INCLUDE_DIR:-/opt/homebrew/include}"
    LIB_DIR="${WHISPER_CPP_LIB_DIR:-/opt/homebrew/lib}"
    c++ -std=c++17 -dynamiclib \
      -I"$SRC_DIR" \
      -I"$INCLUDE_DIR" \
      "$SRC_DIR/lpt_whisper_backend.cpp" \
      -L"$LIB_DIR" \
      -lwhisper \
      -lggml \
      -lggml-base \
      -Wl,-rpath,"$LIB_DIR" \
      -o "$OUTPUT"
    ;;
  Linux)
    OUTPUT="$OUT_DIR/liblpt_whisper_backend.so"
    INCLUDE_DIR="${WHISPER_CPP_INCLUDE_DIR:-/usr/local/include}"
    LIB_DIR="${WHISPER_CPP_LIB_DIR:-/usr/local/lib}"
    c++ -std=c++17 -shared -fPIC \
      -I"$SRC_DIR" \
      -I"$INCLUDE_DIR" \
      "$SRC_DIR/lpt_whisper_backend.cpp" \
      -L"$LIB_DIR" \
      -lwhisper \
      -lggml \
      -lggml-base \
      -Wl,-rpath,"$LIB_DIR" \
      -o "$OUTPUT"
    ;;
  *)
    echo "Unsupported platform for scripts/build-whisper-backend.sh. Build the C++ shim with CMake/MSVC on Windows." >&2
    exit 1
    ;;
esac

echo "$OUTPUT"
