# レビュー指摘の全項目対応

2026-08-18のコードレビュー指摘（設計7件・実装13件・計画2件）を潰す。
TDD（Red-Green-Refactor）で進め、構造変更と振る舞い変更のコミットを分離する。
場当たり的パッチではなく恒久対応を選ぶ（下記「設計判断」）。

## 事前調査の結果

- whisper-rs 0.16: `WhisperState` は `Arc<WhisperInnerContext>` を内部保持し
  ライフタイムなし・`Send + Sync`（whisper_state/mod.rs:16-23）→ エンジン構造体に
  保持して再利用できる。`create_state` 毎回呼びは不要。
- whisper-rs 0.16: `WhisperVadParams` は `Copy + Clone`（whisper_vad.rs:15）→
  load時に一度envをパースして値を保持できる。
- cpal: コールバックはRTスレッド。現状はVec確保×2＋Mutexで非RT安全。
  `SupportedStreamConfig` からrate/channelsが得られるのはストリーム構築時なので、
  リサンプラ構築は消費側スレッドに設定を渡してから行う必要がある。

## 設計判断（恒久対応の根拠）

1. **リサンプラは rubato を採用**（手書き線形補間の廃止）。
   チャンク境界の位相リセット（正しさのバグ）と、ローパスなし線形補間の
   エイリアシング（品質のバグ）を一挙に解消する。rubatoは純Rustで
   プラットフォーム非依存 → kkm-coreの「headlessでテスト可能」原則を保てる。
   kkm-core側は可変長チャンクを固定ブロックに整えて渡す薄いラッパー
   `StreamResampler` を持ち、チャンク分割==一括のプロパティテストで検証する。
2. **セッション管理は常駐パイプラインワーカーに再設計**（フラグ共有の廃止）。
   単一の長寿命ワーカースレッドがエンジン（Whisper/VAD）とスケジューラを
   永続所有し、Start/Stopをコマンドチャネルで受けて直列処理する。
   - 再起動レース: コマンドが直列化されるので構造的に消える
   - モデル再ロード: ワーカーがエンジンを持ち続けるので初回のみ
   - 音声経路: cpalコールバック→rtrbリングバッファ→ワーカーが
     モノラル化・リサンプル・scheduler投入（コールバックはpushのみ）
   - キャプチャスレッドはセッション毎に生成し、デバイス設定
     (rate/channels)とリング消費側をチャネルでワーカーへ渡す

## Phase 1: kkm-core（TDD）

- [x] LocalAgreement: `flush()` — 保留中のvolatileを確定として取り出す（境界フラッシュ/停止時フラッシュの共通化）
- [x] LocalAgreement: 確定済み領域と食い違う仮説が来たらvolatileを空にする防御
- [x] Scheduler: 死んだ状態変数 `window_start` を削除（tidy）
- [x] StepOutput: `utterance_final: Option<String>` — 発話確定シグナル（Step 2翻訳レーンの前提）
- [x] Scheduler: 15秒スライド時、仮説 `end_ms` 以降の未転写音声を次窓へ持ち越す
- [x] Scheduler: 無音窓破棄・境界スライド時に末尾300msを残す（発話頭欠け防止）
- [x] Scheduler: `finish()` — 停止時に残volatileを確定して返す
- [x] Resampler: rubatoベースの `StreamResampler`（可変長チャンク対応、手書き線形補間を置換）
- [x] Resampler: チャンク分割処理==一括処理のプロパティテスト（44.1kHz等）

## Phase 2: kkm-whisper

- [x] tests modをファイル末尾へ（clippy: items after a test module）
- [x] `SpeechDetector` に `Send` 境界（kkm-core側）
- [x] VADパラメータをload時に一度だけenvからパースして保持
- [x] `WhisperState` をエンジンに保持して再利用（毎decodeのcreate_state廃止）

## Phase 3: src-tauri + UI

- [x] 常駐パイプラインワーカー（エンジン永続所有＋Start/Stopコマンドチャネル直列化）
      → 二重起動レースとRecord毎のモデル再ロードを構造的に解消
- [x] 音声コールバックをロックフリーリングバッファ（rtrb）化、モノラル化＋リサンプルはワーカー側へ
- [x] cpal入力のi16/u16/f32対応
- [x] デコードケイデンス: sleepからdecode所要時間を差し引き、100ms刻みで停止フラグ確認
- [x] status イベントを構造化（state + message）し、UIのrunningを同期
- [x] 空のStepOutputはemitしない
- [x] 停止時に scheduler.finish() で残テキストをフラッシュ
- [x] UI: 自動スクロール／utterance_finalで改行／トランスクリプト上限
- [x] model_path のCARGO_MANIFEST_DIR決め打ちにTODO明記

## Phase 4: ドキュメント

- [x] README: KKM_* 環境変数一覧とmodelsディレクトリの準備手順
- [x] plan.md: Step 1残項目の更新

## Phase 5: ggml cdylib分離スパイク（Step 2最大リスクの前倒し検証）

- [x] llama-cpp-2をC ABIのcdylibに隔離し、whisper-rsリンク済みバイナリからlibloadingでロード
- [x] whisperデコード＋翻訳の同時実行でSIGABRTが出ないことを確認（Metal両有効、RSS 3791MB）
- [x] 結果をdocs/step0-results.mdに追記（ADR化はStep 2着手時）
