# LivePolyTrans

LivePolyTrans is a macOS desktop app that listens to your microphone and speaker audio, displays live transcription in a chat-style log, and saves bilingual transcript files.

## First Run

1. Open `LivePolyTrans.app`.
2. Allow microphone access when macOS asks.
3. Allow audio capture or screen/audio recording access if macOS asks for speaker audio.
4. Press `Record`.

The app automatically detects installed Apple speech languages and selects a main two-language pair. You can manually change `Main` and `Sub` from the language controls.

## Main Controls

- `Record`: starts or stops both microphone and speaker streams.
- `Mic`: starts or stops only your microphone lane.
- `Speaker`: starts or stops the other-person/system-audio lane.
- `Refresh Languages`: reloads installed speech languages.
- `Copy`: copies the visible transcript.
- `Save`: writes JSON and TXT transcript exports.
- `Clear`: clears the visible chat log.

## Saved Files

Live final phrases are automatically written every 15 minutes into segment files:

- `segment-*.jsonl`
- `segment-*.txt`

Manual exports from `Save` are written as:

- `transcript-*.json`
- `transcript-*.txt`

Both are stored under the app data directory managed by macOS/Tauri.

## Requirements

- macOS 26 or later
- Apple Speech language packs installed for the languages you want to use
- Microphone permission
- Audio capture permission for speaker/system audio

Speaker capture depends on Apple Core Audio Tap support and macOS privacy approval.
