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
- `Qwen3-4B-Instruct-2507-Q4_K_M.gguf` — 翻訳LLM（無くても文字起こしは動く。翻訳だけが止まる）

翻訳バックエンドはcdylibとして別にビルドする。llama.cppのggmlをwhisperのggmlと同じバイナリに入れられないため（[ADR-153805](docs/ADR/20260819-143000-isolate-llama-cpp-in-a-cdylib.md)）:

```sh
cargo build --release -p lpt-translate-ggml
```

起動:

- 開発: `npm run tauri dev`（上のcdylibを先に一度ビルドしておく。dev/releaseどちらのプロファイルでも探す）
- 実機確認: `./scripts/build-app.sh --open` — cdylibのビルドから`.app`の作成・起動まで。スピーカーレーンには`.app`起動が必須

## 録音

Recordを押すとセッションごとに `~/Music/live-poly-trans/<yyyyMMddHHmmss>/` が作られ、以下が書き出される（48kHz / モノラル / 16bit PCM）:

- `mic.wav` — マイクレーン
- `speaker.wav` — スピーカーレーン（取得できなかった場合は無音）
- `mix.wav` — 両レーンのミックス
- `transcript.jsonl` — 確定した発話と、その訳文（1行1件）

VADや無音ゲートより手前の音をそのまま書くので、文字起こしが拾わなかった部分もファイルには残る。3つのwavは同じ時間軸・同じ長さで、レーンが音を出していない区間は無音で埋められる。`transcript.jsonl` の `startMs`/`endMs` はこの録音上の位置なので、そのまま音声にシークできる:

```json
{"type":"utterance","id":1,"lane":"mic","startMs":1200,"endMs":3400,"lang":"ja","text":"こんにちは"}
{"type":"translation","id":1,"lang":"en","text":"Hello."}
```

訳文は発話の数秒後に、レーンをまたいで前後して届く。だから位置ではなく `id` で発話に結びつける。

スピーカーレーンはmacOSのシステム音声権限が要るため、`.app`として起動しないと無音になる（[docs/step0-tap-results.md](docs/step0-tap-results.md)）。

## 翻訳と言語設定

ヘッダの言語ピル（`日本語 ⇄ English` のような表示）から、**話される言語**（最大2）と**翻訳先**を別々に設定する。表示の記号がそのまま状態を表す:

| 表示 | 意味 |
|---|---|
| `日本語` | 単一言語・訳さない |
| `English → 日本語` | 英語の発言を日本語で読む |
| `日本語 ⇄ English` | 相互翻訳（画面を相手にも見せる） |
| `日本語 · English` | 2言語だが訳さない |

話される言語が1つなら言語判定そのものを行わないので、速く、取り違えも起きない。変更は録音中でも次の発話から反映される（モデルの再ロードは起きない）。

確定した文だけが翻訳に回り、認識をブロックしない。翻訳が追いつかないときは**古い文から捨てる**（画面に出ている最新の文を優先する）。捨てられた行は「訳しています…」のまま残らず、待つのをやめる。

## 環境変数

すべて任意。未設定時は括弧内の既定値で動く。

| 変数 | 既定 | 意味 |
|---|---|---|
| `LPT_WHISPER_MODEL` | `models/ggml-large-v3-turbo-q8_0.bin` | Whisperモデルのパス |
| `LPT_VAD_MODEL` | `models/ggml-silero-v5.1.2.bin` | Silero VADモデルのパス |
| `LPT_LLM_MODEL` | `models/Qwen3-4B-Instruct-2507-Q4_K_M.gguf` | 翻訳LLMのパス |
| `LPT_TRANSLATE_DYLIB` | `.app`内 → `target/{release,debug}` の順に探索 | 翻訳バックエンドcdylibのパス |
| `LPT_LLM_GPU_LAYERS` | 全層 | 翻訳LLMをGPUに載せる層数。`0`でCPU |
| `LPT_LANGS` | `ja,en` | 起動時の「話される言語」候補（最大2）。アプリ内の言語ピルで変更でき、そちらが優先 |
| `LPT_LANG` | 未設定 | 設定すると起動時の話される言語を1つに固定（言語判定を行わない） |
| `LPT_TARGET` | `ja` | 起動時の翻訳先。`none` で翻訳オフ |
| `LPT_VAD_THRESHOLD` | whisper.cpp既定(0.5) | 発話判定しきい値 0..1 |
| `LPT_VAD_MIN_SPEECH_MS` | whisper.cpp既定 | これより短い発話は無視 |
| `LPT_VAD_MIN_SILENCE_MS` | whisper.cpp既定 | 発話区切りとみなす最短無音 |
| `LPT_VAD_PAD_MS` | whisper.cpp既定 | 検出区間の前後パディング |
| `LPT_RECORD_DIR` | `~/Music/live-poly-trans` | 録音セッションフォルダを作る親ディレクトリ |
