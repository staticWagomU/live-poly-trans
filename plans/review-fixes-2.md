# レビュー指摘の修正プラン（2回目 / 2026-08-18）

セッション内レビューで挙がった指摘 #1〜#9 ＋ 小言4件の修正。
Tidy First: 構造変更（refactor:）と挙動変更（fix:/feat:）はコミットを分ける。
挙動変更はコアに閉じるものからTDDで進める。

## 対象外（調査の結果、対応不要）

- cclens.db → 既に .gitignore 済みだった（指摘が誤り）

## Tidy First（構造のみ、挙動不変）

- [ ] T1. `pipeline::run` の `if !matches!(cmd, Cmd::Start)` を `match` に
      （Cmdが3種目を得たときに網羅性チェックが効くように）
- [ ] T2. `run_capture_loop` のレーン固有状態（session / resampler /
      scheduler / mono / reported_drops）を `LaneRuntime` に抽出。
      Step 3（スピーカーレーン）が「ループ複製」でなく「Vecに1要素足す」
      になる形へ。Engines（whisper/VAD）はレーン間共有のまま

## 挙動修正（コア: TDD）

- [ ] B1. 【#5】空テキスト仮説から `window_lang` をピンしない
      （咳払い1秒の誤検出言語が窓全体を汚染するのを防ぐ）
- [ ] B2. 【#2の部品】`StreamResampler::flush()` 追加。pending（CHUNK未満の
      端数）をゼロ詰めで押し出し、フィルタ遅延分も1チャンク分ゼロを
      流して回収する
- [ ] B3. 【#2】`StreamScheduler::finish(engine, vad, lang)` に変更。
      バッファに未デコード音声が残っていれば（VADが発話を認めるとき）
      MIN_WINDOWまでゼロ詰めして最後の1デコードをしてからflushする。
      既存テストはシグネチャ追随＋再デコード分のスクリプト追加
- [ ] B4. 【#1】emit判定を `EmitGate` に分離: 「何も新しくない」と
      「volatileが空に*変わった*（矛盾検出で隠した）」を区別し、
      後者はUIへ届ける。セッション終了時に残留volatileも掃除する

## 挙動修正（シェル側）

- [ ] B5. 【#4】cpalエラーコールバック → `CaptureSession.error`
      （Mutex<Option<String>>、try_lockで初回のみ記録）。
      ポーリングループで検知したらセッションをエラー終了させる
- [ ] B6. 【#6】`get_status` コマンド追加（Arc<Mutex<StatusPayload>> を
      tauri stateとworkerで共有）。UIはonMountで照会して初期化。
      Record押下成功時はUI側でも `loading` に楽観更新（toggleレース緩和）
- [ ] B7. 【#7】`transcript` イベントに `lane: "mic"` を追加。UI型も追随
      （表示分離はStep 3で）
- [ ] B8. 【小言】overrunメッセージ: サンプル数 → ミリ秒表記
      （dropped / (channels × src_rate)）、最後のdropから5秒で自動クリア
- [ ] B9. 【#8】lpt-translate-ggml の強化:
      - 全 `extern "C"` を catch_unwind で包む（panic→abort をnull返却に）
      - `tokens.len()-1` の空トークン列panicガード
      - contextを init 時に1回だけ作り `clear_kv_cache()` で再利用
        （LlamaContext<'a>の自己参照は Box::leak ＋ 手動Drop で回避。
        Drop順: ctx → model → backend）
      - プロンプトはモデル内蔵chat template（`chat_template()` +
        `apply_chat_template`）を優先、無ければ従来のChatMLへフォールバック
      - 生成上限 256→512、上限到達時は stderr に警告（無言切り詰め廃止）
      - プロンプト長 + 生成上限 > n_ctx は明示エラー

## UI（挙動）

- [ ] U1. 【小言】followTail: 最下部付近（48px以内）にいるときだけ追尾。
      スクロールアップ中は引き戻さない
- [ ] U2. 【小言】status表示に `role="status"`（aria-live: polite 相当）

## ドキュメント

- [ ] D1. 【#3】`Hypothesis::start_ms/end_ms` のdocを「ウィンドウ先頭からの
      ms」に修正（実装・schedulerの持ち越し計算と契約を一致させる）
- [ ] D2. 【#9】max-windowスライドの `end_ms` 依存に「過小→重複コミット/
      過大→未転写破棄、被害はMAX_CARRYで5秒に有界」の注記を追加

## 検証

- [ ] cargo test --workspace 全緑
- [ ] cargo clippy --workspace
- [ ] svelte autofixer / bun run check
- [ ] cargo build -p lpt-translate-ggml（cdylibはビルド確認。実機確認は
      spike cdylib-check で可能）
