# LivePolyTrans v2

リアルタイム多言語文字起こし＋翻訳アプリの作り直し（v2）ブランチ。

- 単語単位のリアルタイム文字起こし（ローカルWhisper）
- 確定文単位のリアルタイム翻訳（ローカルLLM）
- マイク／スピーカー別レーンの録音
- mac / Windows 対応予定

技術選定の経緯は [docs/ADR/](docs/ADR/)、進行計画は [plan.md](plan.md) を参照。

v1の実装は `v1` ブランチに保存されている。

## 実行準備

モデルファイルを `models/` に置く（gitignore済み）:

- `ggml-large-v3-turbo-q8_0.bin` — Whisper本体（q8_0が既定。q5_0はMetalで逆に遅い、docs/step0-results.md参照）
- `ggml-silero-v5.1.2.bin` — Silero VAD（whisper.cpp配布のGGML版）

起動: `npm run tauri dev`

## 環境変数

すべて任意。未設定時は括弧内の既定値で動く。

| 変数 | 既定 | 意味 |
|---|---|---|
| `LPT_WHISPER_MODEL` | `models/ggml-large-v3-turbo-q8_0.bin` | Whisperモデルのパス |
| `LPT_VAD_MODEL` | `models/ggml-silero-v5.1.2.bin` | Silero VADモデルのパス |
| `LPT_LANGS` | `ja,en` | 言語自動判定の候補ペア（Main/Sub）。この中から発話ごとに選ぶ |
| `LPT_LANG` | 未設定 | 設定すると単一言語に固定（自動判定を無効化） |
| `LPT_VAD_THRESHOLD` | whisper.cpp既定(0.5) | 発話判定しきい値 0..1 |
| `LPT_VAD_MIN_SPEECH_MS` | whisper.cpp既定 | これより短い発話は無視 |
| `LPT_VAD_MIN_SILENCE_MS` | whisper.cpp既定 | 発話区切りとみなす最短無音 |
| `LPT_VAD_PAD_MS` | whisper.cpp既定 | 検出区間の前後パディング |
