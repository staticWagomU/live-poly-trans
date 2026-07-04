# LivePolyTrans

LivePolyTrans is a macOS desktop app that listens to your microphone and speaker audio, displays live transcription with translation in a chat-style log, and can record both audio lanes to files you can review later.

Both selected languages are recognized in parallel and the helper picks the transcript that actually matches what was spoken, so a Japanese/English conversation lands in one clean lane per speaker.

## First Run

1. Open `LivePolyTrans.app`.
2. Allow microphone access when macOS asks.
3. Allow audio capture or screen/audio recording access if macOS asks for speaker audio.
4. Press `Record`.

The app automatically detects installed Apple speech languages and picks a language pair with your system language as `Main`. You can change `Main` and `Sub` from the language controls. If a selected language pack is missing, the helper downloads it automatically and shows the progress in the transcript pane.

## Main Controls

- `Live` / `Recordings`: switches between the live transcript and the recording review screen.
- `Record`: starts or stops the selected audio lanes.
- `Mic` / `Both` / `Speaker`: choose which lanes to capture.
- `Save audio`: when enabled before pressing Record, mic and speaker audio are also written to `.m4a` files.
- `↻` (in the language strip): reloads installed speech languages.
- `Copy`: copies the finalized transcript to the clipboard.
- `Save`: writes JSON and TXT transcript exports.
- `Clear`: clears the chat log, summary, and AI chat.

## Meeting AI

- `Summary` updates automatically a few seconds after speech settles, using a rolling summary so long meetings stay within the on-device model's context window. The summary language follows the `Main` language.
- `Ask` is a chat with history: follow-up questions see the previous turns and the recent transcript.
- Both run on Apple Intelligence via a persistent helper process, so responses do not pay model startup cost on every request.

## Recordings

Enable `Save audio` and record; then open the `Recordings` tab:

- The right sidebar lists recordings; each recording shows its audio files (`mic.m4a`, `speaker.m4a`).
- The center shows the selected file's waveform (click to seek) with an audio player.
- Below the waveform, the timestamped transcript follows playback; clicking a line jumps to that position.
- `Mic` / `Speaker` / `Merged` download buttons copy the chosen variant into your Downloads folder. `Merged` mixes both lanes into one file on demand.

## Saved Files

Live final phrases are automatically written every 15 minutes into segment files:

- `segment-*.jsonl`
- `segment-*.txt`

Manual exports from `Save` are written as:

- `transcript-<timestamp>.json`
- `transcript-<timestamp>.txt`

Recordings live under `recordings/<recording-id>/` with per-lane audio (`mic.m4a`, `speaker.m4a`), per-lane transcripts (`*.jsonl`), and `meta.json`. Everything is stored under the app data directory managed by macOS/Tauri.

## Requirements

- macOS 26 or later
- Apple Speech language packs for the languages you want (auto-downloaded when missing)
- Microphone permission
- Audio capture permission for speaker/system audio
- Apple Intelligence enabled for the summary/chat features

Speaker capture depends on ScreenCaptureKit system-audio capture and macOS privacy approval.
