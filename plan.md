# LivePolyTrans v2 再構築プラン

技術選定の背景と根拠は `docs/ADR/` を参照。v1の実装は `v1` ブランチに保存されており、必要なファイルは都度持ってくる。

## 要件（v2スコープ）

- 【MUST】単語単位のリアルタイム文字起こし（発話終了待ちにしない）
- 【MUST】確定文単位のリアルタイム翻訳（例: 英語発言→日本語）
- 【MUST】低レイテンシー・高精度（部分結果2秒以内 / 確定1.5秒以内を計測予算とする）
- 【MUST】マイク／スピーカー別レーンの録音
- 【MUST】低メモリ
- 【SHOULD】録音へのポストプロセス話者分離

v1にあった以下はv2スコープ外: オーバーレイ、トレイ、AI要約/チャット、用語集、Obsidianエクスポート、N言語並列認識＋アービター（言語はMain/Subの2言語ペアに固定）。

## 技術スタック（決定済み）

| レイヤー | 選定 | ADR |
|---|---|---|
| コア構造 | 単一Rustプロセス、trait抽象（`AsrEngine` / `Translator` / `CaptureSource`） | 153800 |
| ASR | whisper-rs（whisper.cppインプロセス）＋LocalAgreement擬似ストリーミング。モデルサイズ可変 | 153801 |
| UIシェル | Tauri 2＋Svelte 5 | 153802 |
| 音声取得 | マイク=cpal / スピーカー=CoreAudio Process Tap（mac, 14.2+）・WASAPIループバック（Win） | 153803 |
| 翻訳 | 組み込みローカルLLM（llama.cpp系GGUF）＋アプリ内モデル選択・ダウンロード。Ollama / Codex / Claude Code / DeepLはtraitで将来追加 | 153804 |

## ビルド順（small-first）

各ステップは動くものを維持したまま積み上げる。TDD（Red-Green-Refactor）で進め、コアはheadlessでテスト可能に保つ。

### Step 0: 技術検証スパイク

- [ ] RustからCoreAudio Process Tapでシステム音声PCMを取得できることを確認（最大の未知数。不可ならSwift薄ヘルパーへフォールバックし、ADR-153803を更新）
- [ ] whisper-rs＋Metalでlarge-v3-turbo量子化モデルの動作・メモリ実測
- [ ] llama.cpp系バインディング（llama-cpp-2等）で翻訳用小型GGUFモデルの動作・メモリ実測（whisperとの同時稼働込み）
- [ ] whisper-rsとllama-cpp-2の同一バイナリへの同時リンク検証（両者はそれぞれggmlを同梱するため、シンボル衝突・ggmlバージョン不一致・バイナリ肥大の有無を確認）
- [ ] **MLX比較ハーネス**: 同一音声・同一文でggmlとMLXをA/B比較できるようにする
  - 擬似ストリーミングのスケジューラ（LocalAgreement）は共通実装にし、推論呼び出し（`transcribe(window)` / `translate(sentence)`）だけをバックエンド差し替えにする（エンジン差と確定ロジック差を混ぜない）
  - ggml側: whisper-rs / llama-cpp-2（インプロセス）
  - MLX側: mlx-whisper / mlx-lmを**PoC限定のPythonサイドカー**（stdio）で呼ぶ。製品構成は単一プロセス（ADR-153800）を維持し、MLX採用が決まった時点でmlx-rs / mlx-c FFIによる本実装を検討する
  - モデルは同一重みで揃える（例: Whisper large-v3-turboのGGUF量子化 vs MLX 4bit、翻訳はqwen3-4b級のGGUF q4 vs MLX 4bit）
  - 計測指標: 最初の部分結果までの時間 / 発話終了から確定までの時間 / RTF / ピークメモリ（RSS） / 翻訳の文あたりレイテンシーとtok/s / ASR・翻訳同時稼働時の劣化幅
- PoCアプリの設定でASR・翻訳バックエンド（ggml⇔MLX）を切り替え、体感でも比較できるようにする

### Step 1: マイク → ASR → 画面表示

- [ ] ワークスペース初期化（Tauri 2＋Svelte 5、コアはcrateとして分離）
- [ ] cpalでマイク取得 → リサンプリング（16kHz mono f32）
- [ ] `AsrEngine` trait＋whisper-rs実装、LocalAgreementによるpartial/final確定
- [ ] `transcript` イベント（lane / kind: partial|final / text / start_ms）をUIへ
- [ ] 最小UI: 1ウィンドウのトランスクリプト表示（partialの逐次更新表示）

### Step 2: 確定文の翻訳レーン

- [ ] `Translator` trait＋組み込みllama.cpp実装（キュー＋逐次処理、バックプレッシャーあり）
- [ ] モデル管理（ModelManager）: ASRのWhisperモデルと翻訳LLMを共通の仕組みで選択・ダウンロード・切り替え（PoC段階はローカルパス指定でも可）
- [ ] finalイベント→翻訳→UIの確定文に訳文を後付け表示
- [ ] 言語設定: Main（利用者言語）/ Sub の2言語ペア
- 将来: Ollama（HTTP）/ Codex・Claude Code（CLIサブプロセス）/ DeepL（HTTP）を`Translator`実装として追加

### Step 3: スピーカーレーン

- [ ] CoreAudio Process Tap実装（Step 0の検証結果に基づく）
- [ ] マイク／スピーカー2レーンの同時稼働と表示分離

### Step 4: 録音

- [ ] ゲート等の加工前の生音声をレーン別ファイルに保存（v1の設計を踏襲）
- [ ] タイムスタンプとトランスクリプトの整合

### Step 5: 話者分離（SHOULD）

- [ ] 録音ファイルへのポストプロセス話者分離（pyannote / WhisperX等を検証して選定）

## v1から流用候補のファイル

- `src-tauri/src/whisper_engine_stabilization.rs` — partial安定化ロジック（インプロセス版に移植）
- `src-tauri/src/translation_backend.rs` — Ollama/DeepL HTTPクライアント
- `src/lib/theme.css` ほかUI部品 — 必要になった時点で選択的に
- `scripts/measure-whisper-engine.mjs` — レイテンシー計測予算の考え方
