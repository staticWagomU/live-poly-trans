# Kikimimic v2 再構築プラン

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
| ggml分離 | llama.cpp側をcdylibに隔離しdlopen（whisperのggmlとシンボル衝突するため）。プロセスは単一のまま | 153805 |

## ビルド順（small-first）

各ステップは動くものを維持したまま積み上げる。TDD（Red-Green-Refactor）で進め、コアはheadlessでテスト可能に保つ。

### Step 0: 技術検証スパイク

実測結果の詳細は [docs/step0-results.md](docs/step0-results.md)（2026-08-10, M4 Pro）。

- [x] RustからCoreAudio Process Tapでシステム音声PCMを取得できることを確認 — **成功**（objc2-core-audioで48kHz/2ch/f32を取得、Swiftヘルパー不要）。再現: `scripts/tap-check.sh`。ただし署名済み.appを`open`で起動しないとTCCが無音を返す（[docs/step0-tap-results.md](docs/step0-tap-results.md)）
- [x] whisper-rs＋Metalでlarge-v3-turbo量子化モデルの動作・メモリ実測 — 全予算クリア（最初の部分結果≈1.7s、RSS 1.1GB）。q5_0はq8_0より遅いためq8_0を既定候補に
- [x] llama.cpp系バインディング（llama-cpp-2等）で翻訳用小型GGUFモデルの動作・メモリ実測（whisperとの同時稼働込み）— 文あたり0.3〜0.9s、同時稼働時でも最悪4s/文で要件内
- [x] whisper-rsとllama-cpp-2の同一バイナリへの同時リンク検証 — **失敗を確認**: ggmlシンボル衝突により実行時SIGABRT。→ Step 2でcdylib分離を採用しADR化（[ADR-153805](docs/ADR/20260819-143000-isolate-llama-cpp-in-a-cdylib.md)）
- [x] **MLX比較ハーネス**: 同一音声・同一文でggmlとMLXをA/B比較できるようにする — 結果: ほぼ互角（ASRでMLXが1割強速い程度）。ggml路線を維持し、MLXバックエンドは追加しない
  - 擬似ストリーミングのスケジューラ（LocalAgreement）は共通実装にし、推論呼び出し（`transcribe(window)` / `translate(sentence)`）だけをバックエンド差し替えにする（エンジン差と確定ロジック差を混ぜない）
  - ggml側: whisper-rs / llama-cpp-2（インプロセス）
  - MLX側: mlx-whisper / mlx-lmを**PoC限定のPythonサイドカー**（stdio）で呼ぶ。製品構成は単一プロセス（ADR-153800）を維持し、MLX採用が決まった時点でmlx-rs / mlx-c FFIによる本実装を検討する
  - モデルは同一重みで揃える（例: Whisper large-v3-turboのGGUF量子化 vs MLX 4bit、翻訳はqwen3-4b級のGGUF q4 vs MLX 4bit）
  - 計測指標: 最初の部分結果までの時間 / 発話終了から確定までの時間 / RTF / ピークメモリ（RSS） / 翻訳の文あたりレイテンシーとtok/s / ASR・翻訳同時稼働時の劣化幅
- PoCアプリの設定でASR・翻訳バックエンド（ggml⇔MLX）を切り替え、体感でも比較できるようにする

### Step 1: マイク → ASR → 画面表示

- [x] ワークスペース初期化（Tauri 2＋Svelte 5、コアはcrateとして分離: kkm-core / kkm-whisper / src-tauri）
- [x] cpalでマイク取得 → リサンプリング（16kHz mono f32、線形補間・TDD済み）
- [x] `AsrEngine` trait＋whisper-rs実装、LocalAgreementによるpartial/final確定（文字単位LCP・TDD済み、テスト11本）
- [x] `transcript` イベント（committedDelta / volatile）をUIへ ※lane/start_msはStep 3のスピーカーレーン追加時に拡張
- [x] 最小UI: 1ウィンドウのトランスクリプト表示（確定=白 / volatile=グレーの逐次更新）
- [x] 無音ゲート（-50dBFS未満はデコードスキップ。無音時のWhisper幻覚対策、Step 0で実測確認）→ その後Silero VADに置換
- [x] 2026-08-18 レビュー指摘の全面対応（plans/review-fixes.md）: 常駐パイプラインワーカー化（レース・再ロード解消）、`utterance_final`イベント（Step 2翻訳レーンの入力単位）、スライド時の音声持ち越し＋発話頭ガード、rubatoリサンプラ、RT安全なキャプチャ、UIのstatus同期・自動スクロール
- [x] 2026-08-18 2回目レビュー対応（plans/review-fixes-2.md）: EmitGate（volatile取り消しイベントの配送）、Stop時の未デコード音声救済（ring/resampler/schedulerの畳み込み）、`get_status`同期＋楽観更新、キャプチャエラーのセッション伝播、レーン配管（`lane`フィールド・LaneRuntime）、空仮説の言語ピン抑止、翻訳cdylib強化（catch_unwind・context再利用・chat template・切り詰めの可視化）、追尾スクロールの一時停止、overrun表示のms化
- [x] 実発話での動作確認（Recordを押して日本語で話す→confirm。`KKM_LANG=en`で英語も確認）

### Step 2: 確定文の翻訳レーン

- [x] `Translator` trait＋組み込みllama.cpp実装 — cdylib（`kkm-translate-ggml`）をdlopenする`kkm-translate`＋専用スレッド。キュー上限8文、溢れたら**最古を捨てる**（会議で価値があるのは画面に出ている最新の文）
- [x] モデル管理（`kkm-core::models`）: ASR・VAD・翻訳LLMを共通の探索順（env override → アプリデータ → チェックアウトの`models/`）で解決。ダウンロードUIは後続
- [x] finalイベント→翻訳→UIの確定文に訳文を後付け表示 — 発話にID採番、`translation`イベントでID照合。`transcript.jsonl`は`{"type":"utterance"|"translation"}`の2種
- [x] 言語設定: **Main/Sub廃止**。「話される言語（最大2）」と「翻訳先（なし可）」を分離し、相互翻訳フラグを追加（`mockups/feature-language-picker.html`案A、設計は`plans/step2-translation.md`）。録音中の変更はモデル再ロードなしで次の発話から反映
- [x] 実発話での確認（2026-08-20, `Kikimimic.app`）— 確定文の下に訳文が後付けで並ぶことを確認。
      cdylibのC ABIを`kkm_translate_*`に改名した後の初回実行でもあり、dlopenが通ることも兼ねて確認した
- 将来: Ollama（HTTP）/ Codex・Claude Code（CLIサブプロセス）/ DeepL（HTTP）を`Translator`実装として追加

### Step 3: スピーカーレーン

- [x] CoreAudio Process Tap実装（Step 0の検証結果に基づく）— `capture/speaker.rs`
- [x] マイク／スピーカー2レーンの同時稼働と表示分離 — 当初はレーン別2カラム。Step 4でレーン間同期が実測±15msに収まったため、`start_ms`順の**1本のストリーム＋話者ピル**に変更（`mockups/desktop-prototype.html`、`plans/ui-mockup-alignment.md`）
- [x] 権限が「無音」として現れる問題の検知（`SilenceWatch`）と`NSAudioCaptureUsageDescription`
- [x] 実発話での2レーン動作確認（`.app`として起動しないとスピーカーは無音。`bun run tauri build` → `open`）
- [ ] デフォルト出力デバイス切り替え（ヘッドホン抜き差し等）への追従

### Step 4: 録音

- [x] ゲート等の加工前の生音声をレーン別ファイルに保存（v1の設計を踏襲）— `~/Music/kikimimic/<yyyyMMddHHmmss>/{mic,speaker,mix}.wav`（48kHz/mono/16bit）。`record.rs`＋`kkm-core::mix`
- [x] 実録音での確認（2026-08-19, `.app`起動で58秒）— mic peak 0.157・全区間連続、speaker peak 0.909・先頭10秒無音、mixは両者の和（10秒毎RMSで確認）
- [x] タイムスタンプとトランスクリプトの整合 — schedulerがストリーム絶対位置で`Utterance{text,start_ms,end_ms}`を返し、`LaneTimeline`が録音時間へ変換。`transcript.jsonl`に追記＋UIは各行を`mm:ss`付きで表示
- [x] レーン間の厳密な同期 — cpalの`capture`タイムスタンプとProcess Tapの`mHostTime`（どちらもmach host time）でバッファごとの録音位置を決定。`capture::Anchor`＋`SessionClock`。overrunで落ちた分も時間の穴として正しく残る（設計: plans/lane-sync.md）
- [x] 同期の実測（2026-08-19）— スピーカーから鳴らしたクリックが両レーンで **+15〜16ms**（空気の伝搬＋入力レイテンシ）に収まり、18.5秒離れた点で1ms以内。再測は `python3 scripts/check-recording.py --sync`（ヘッドホン再生だと測定不能）

### Step 4.5: UIをモックアップに合わせる

設計は `plans/ui-mockup-alignment.md`、意匠は `mockups/desktop-prototype.html`。

- [x] 録音一覧（`list_recordings`）と過去セッションの読み戻し（`read_recording`）— `src-tauri/src/library.rs`。
      `transcript.jsonl`は追記ログなので、読むときはidで訳文を結合し`start_ms`で並べ直す
- [x] ホーム（録音一覧）／セッションの2ビュー、ライトテーマ、ツールバー、状態バッジ、
      言語ピル＋ポップオーバー、1本の文字起こし
- [x] 実アプリでの確認（2026-08-20, `Kikimimic.app`）— ホームに過去8セッションが並ぶこと、
      スピーカーレーンが文字を出すことを確認。改名後の初回起動でもあるので、
      マイクとシステム音声の許可が新しいbundle identifierで訊き直されることも併せて確認した
- モックアップにあるが未実装（バックエンドが無いもの）: Ask、波形／再生、書き出し、行削除、
      一時停止、レーン×言語マトリクス

### Step 5: 話者分離（SHOULD）

- [ ] 録音ファイルへのポストプロセス話者分離（pyannote / WhisperX等を検証して選定）
      ※ `~/.cache/huggingface/hub` に `pyannote/speaker-diarization-community-1` が既にある

### Step 6: ASRエンジンの複線化（Nemotron Streaming）

Whisper一本から、**複数のASRモデルを選べる**構成にする。最初の追加候補が
NVIDIA Nemotron Streaming 3.5（`parakeet` RNN-T）で、ランタイムは
[transcribe.cpp](https://github.com/handy-computer/transcribe.cpp)（`transcribe-cpp` 0.2.1）。

実測は [docs/nemotron-spike-results.md](docs/nemotron-spike-results.md)（2026-08-20, M4 Pro）。

- [x] スパイク: 実録音でレイテンシー・メモリ・精度を計測 — `crates/spike/src/bin/nemotron_check.rs`。
      チャンク処理 平均8ms（リアルタイムの約36倍）、確定の遅れ 平均150ms（予算1500ms）、
      RSS 980MB（Whisper 1.1GBより低い）。精度はVAD無しの不利な条件でもほぼ互角で、
      **Whisperが無音区間(-55dBFS)で "Thank you." を幻覚していたことが判明**
- [ ] 決める: 確定を誰が持つか（`CommitPolicy::Auto` のstable-prefix vs 自前のLocalAgreement）。
      RNN-Tはネイティブにストリーミングするので、擬似ストリーミングの機構は要らない
- [ ] 決める: whisper-rsと併存させるか、ASRごとtranscribe.cppに寄せるか。
      transcribe.cppはWhisperも動かせるので、寄せればggmlは3つでなく2つに減る。
      併存ならcdylib＋dlopenでの隔離が必須（ADR-153805と同じ理由）
- [ ] 決める: VADの出所。現在のSilero VADはwhisper.cpp由来なので、whisper-rsを外すと消える
- [ ] 発話境界の供給: `committed`は区切りの無い連結テキスト。`Utterance{text,start_ms,end_ms}`に落とす方法
- [ ] 言語コードの写像: `LanguagePolicy`のISO 2文字 → モデルのロケール（`ja`は不可、`ja-JP`）
- [ ] モデル探索順にHFキャッシュ（`~/.cache/huggingface/hub`）を追加 — Handyと共有できる
- [ ] 上記が固まった時点でADR化

## v1から流用候補のファイル

- `src-tauri/src/whisper_engine_stabilization.rs` — partial安定化ロジック（インプロセス版に移植）
- `src-tauri/src/translation_backend.rs` — Ollama/DeepL HTTPクライアント
- `src/lib/theme.css` ほかUI部品 — 必要になった時点で選択的に
- `scripts/measure-whisper-engine.mjs` — レイテンシー計測予算の考え方
