#!/usr/bin/env bash
# Build the .app, translation backend included.
#
# Two things make this a script rather than one command:
#
#  1. The translation backend is a separate cdylib on purpose (llama's ggml
#     must not be linked next to whisper's — docs/step0-results.md), so cargo
#     does not build it as a dependency of the app. tauri.conf.json copies it
#     into Contents/Frameworks, and it has to exist by then.
#  2. The speaker lane needs a real bundle: launched any other way, macOS
#     withholds the system-audio permission by returning silence
#     (docs/step0-tap-results.md). Hence `open` at the end, not the binary.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> translation backend (cdylib)"
cargo build --release -p lpt-translate-ggml

echo "==> app bundle"
bun run tauri build

# The workspace shares one target dir, so the bundle lands at the root.
app="target/release/bundle/macos/LivePolyTrans.app"
echo "==> built $app"
if [[ "${1-}" == "--open" ]]; then
  open "$app"
fi
