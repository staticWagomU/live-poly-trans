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

## 録音

Recordを押すとセッションごとに `~/Music/live-poly-trans/<yyyyMMddHHmmss>/` が作られ、以下が書き出される（48kHz / モノラル / 16bit PCM）:

- `mic.wav` — マイクレーン
- `speaker.wav` — スピーカーレーン（取得できなかった場合は無音）
- `mix.wav` — 両レーンのミックス
- `transcript.jsonl` — 確定した発話（1行1発話）

VADや無音ゲートより手前の音をそのまま書くので、文字起こしが拾わなかった部分もファイルには残る。3つのwavは同じ時間軸・同じ長さで、レーンが音を出していない区間は無音で埋められる。`transcript.jsonl` の `startMs`/`endMs` はこの録音上の位置なので、そのまま音声にシークできる:

```json
{"endMs":3400,"lane":"mic","startMs":1200,"text":"こんにちは"}
```

スピーカーレーンはmacOSのシステム音声権限が要るため、`.app`として起動しないと無音になる（[docs/step0-tap-results.md](docs/step0-tap-results.md)）。

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
| `LPT_RECORD_DIR` | `~/Music/live-poly-trans` | 録音セッションフォルダを作る親ディレクトリ |
