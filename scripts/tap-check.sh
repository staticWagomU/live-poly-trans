#!/usr/bin/env bash
# Run the CoreAudio Process Tap spike (crates/spike/src/bin/tap_check.rs).
#
# The bundling and `open` are not ceremony: a bare CLI binary makes the
# terminal the TCC responsible process, and macOS then withholds the System
# Audio Recording permission *silently* — noErr everywhere, IOProc firing,
# all-zero samples. Launched as a signed .app the spike is its own
# responsible process and the permission applies to it.
set -euo pipefail

secs="${1:-6}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="/tmp/TapCheck.app"
report="/tmp/lpt-tap-check.txt"

cargo build -q -p spike --bin tap-check --no-default-features --manifest-path "$root/Cargo.toml"

mkdir -p "$app/Contents/MacOS"
cp "$root/target/debug/tap-check" "$app/Contents/MacOS/TapCheck"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>TapCheck</string>
  <key>CFBundleIdentifier</key><string>dev.wagomu.TapCheck</string>
  <key>CFBundleName</key><string>TapCheck</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSMinimumSystemVersion</key><string>14.2</string>
  <key>NSAudioCaptureUsageDescription</key><string>LivePolyTrans spike: capture system audio to verify the CoreAudio Process Tap path.</string>
</dict>
</plist>
PLIST
codesign -f -s - --identifier dev.wagomu.TapCheck "$app"

rm -f "$report"
# The tap needs audio to exist: an idle output device runs no IO cycle.
( for _ in $(seq 1 20); do afplay "$root/assets/test-ja.wav"; done ) &
player=$!
trap 'kill "$player" 2>/dev/null || true' EXIT
sleep 1

open "$app" --args "$secs"

# Launch latency varies (codesign verification on first run), so poll for
# the verdict rather than guessing a sleep.
for _ in $(seq 1 "$((secs + 25))"); do
  [[ -f "$report" ]] && break
  sleep 1
done

if [[ -f "$report" ]]; then
  cat "$report"
else
  echo "no report at $report — the app may not have launched"
  exit 1
fi
