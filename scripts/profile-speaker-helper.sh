#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER_APP="$ROOT_DIR/src-tauri/binaries/LivePolyTransHelper.app"
PROFILE_DIR="${LIVE_POLY_TRANS_PROFILE_DIR:-/tmp/live-poly-trans-speaker-profile}"
SECONDS_TO_SAMPLE="${LIVE_POLY_TRANS_SAMPLE_SECONDS:-15}"
SAY_TEXT="${LIVE_POLY_TRANS_PROFILE_SAY_TEXT:-profiling speaker capture path for live poly trans}"
STDOUT_LOG="$PROFILE_DIR/helper.stdout.log"
STDERR_LOG="$PROFILE_DIR/helper.stderr.log"
SAMPLE_FILE="$PROFILE_DIR/speaker.sample.txt"
SEGMENT_DIR="$PROFILE_DIR/segments"

mkdir -p "$PROFILE_DIR" "$SEGMENT_DIR"
rm -f "$STDOUT_LOG" "$STDERR_LOG" "$SAMPLE_FILE"

if [[ ! -d "$HELPER_APP" ]]; then
  echo "helper app was not found: $HELPER_APP" >&2
  echo "run scripts/build-helper.sh first" >&2
  exit 1
fi

/usr/bin/open -n -W \
  --stdout "$STDOUT_LOG" \
  --stderr "$STDERR_LOG" \
  "$HELPER_APP" \
  --args \
  --stream speaker \
  --source-language en-US \
  --target-language ja-JP \
  --language en-US \
  --language ja-JP \
  --segment-directory "$SEGMENT_DIR" &

OPEN_PID=$!
HELPER_PID=""

cleanup() {
  if [[ -n "$HELPER_PID" ]] && kill -0 "$HELPER_PID" 2>/dev/null; then
    kill "$HELPER_PID" 2>/dev/null || true
    wait "$HELPER_PID" 2>/dev/null || true
  fi

  if kill -0 "$OPEN_PID" 2>/dev/null; then
    kill "$OPEN_PID" 2>/dev/null || true
    wait "$OPEN_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

for _ in {1..120}; do
  HELPER_PID="$(pgrep -n -f 'LivePolyTransHelper.app/Contents/MacOS/live-poly-trans-helper' || true)"

  if [[ -f "$STDERR_LOG" ]] && grep -q 'speaker-screencapturekit-started' "$STDERR_LOG"; then
    break
  fi

  if [[ -f "$STDERR_LOG" ]] && grep -q 'speaker-screen-capture-permission granted=false' "$STDERR_LOG"; then
    echo "speaker capture permission is not granted." >&2
    echo "stderr log: $STDERR_LOG" >&2
    sed -n '1,160p' "$STDERR_LOG" >&2
    exit 2
  fi

  if ! kill -0 "$OPEN_PID" 2>/dev/null && [[ -z "$HELPER_PID" ]]; then
    echo "helper exited before speaker capture started." >&2
    echo "stderr log: $STDERR_LOG" >&2
    sed -n '1,160p' "$STDERR_LOG" >&2 || true
    exit 1
  fi

  sleep 0.25
done

if [[ -z "$HELPER_PID" ]] || ! kill -0 "$HELPER_PID" 2>/dev/null; then
  echo "helper process was not found after launch." >&2
  echo "stderr log: $STDERR_LOG" >&2
  sed -n '1,160p' "$STDERR_LOG" >&2 || true
  exit 1
fi

if ! grep -q 'speaker-screencapturekit-started' "$STDERR_LOG"; then
  echo "speaker capture did not start before timeout." >&2
  echo "stderr log: $STDERR_LOG" >&2
  sed -n '1,200p' "$STDERR_LOG" >&2 || true
  exit 1
fi

say "$SAY_TEXT" >/dev/null 2>&1 &
sample "$HELPER_PID" "$SECONDS_TO_SAMPLE" -file "$SAMPLE_FILE"

echo "speaker helper pid: $HELPER_PID"
echo "sample: $SAMPLE_FILE"
echo "stdout log: $STDOUT_LOG"
echo "stderr log: $STDERR_LOG"
