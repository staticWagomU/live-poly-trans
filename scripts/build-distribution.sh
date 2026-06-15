#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DMG_PATH="$ROOT_DIR/src-tauri/target/release/bundle/dmg/LivePolyTrans_0.1.0_aarch64.dmg"

bun run tauri build

WORK_DIR="$(mktemp -d)"
MOUNT_DIR="$WORK_DIR/mount"
VOLUME_DIR="$WORK_DIR/volume"
GUIDE_APP="$VOLUME_DIR/はじめてガイド.app"
TMP_DMG="$WORK_DIR/LivePolyTrans-with-guide.dmg"

cleanup() {
  if mount | grep -q "$MOUNT_DIR"; then
    hdiutil detach "$MOUNT_DIR" -quiet || true
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p "$MOUNT_DIR" "$VOLUME_DIR"
hdiutil attach "$DMG_PATH" -mountpoint "$MOUNT_DIR" -nobrowse -quiet
ditto "$MOUNT_DIR" "$VOLUME_DIR"
hdiutil detach "$MOUNT_DIR" -quiet

mkdir -p "$GUIDE_APP/Contents/MacOS" "$GUIDE_APP/Contents/Resources"
cat > "$GUIDE_APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>guide</string>
  <key>CFBundleIdentifier</key>
  <string>com.staticwagomu.live-poly-trans.guide</string>
  <key>CFBundleName</key>
  <string>はじめてガイド</string>
  <key>CFBundleDisplayName</key>
  <string>はじめてガイド</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
</dict>
</plist>
PLIST

cat > "$GUIDE_APP/Contents/MacOS/guide" <<'GUIDE'
#!/usr/bin/env bash
osascript <<'APPLESCRIPT'
display dialog "こんにちは！LivePolyTrans はインストール済みの言語を自動で使います。初回はマイクと音声取得の許可をお願いします。起動後は Record を押すだけで、マイクとスピーカーの翻訳ログが表示されます。" buttons {"OK"} default button "OK" with title "LivePolyTrans はじめてガイド"
APPLESCRIPT
GUIDE

chmod +x "$GUIDE_APP/Contents/MacOS/guide"
ditto "$ROOT_DIR/README.md" "$GUIDE_APP/Contents/Resources/README.md"

hdiutil create -volname "LivePolyTrans" -srcfolder "$VOLUME_DIR" -ov -format UDZO "$TMP_DMG" -quiet
mv "$TMP_DMG" "$DMG_PATH"
echo "Finished guided dmg at: $DMG_PATH"
