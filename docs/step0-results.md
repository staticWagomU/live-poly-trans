# Step 0 スパイク結果（2026-08-10, Apple M4 Pro）

plan.md Step 0の実測結果。再現方法: `cargo build --release -p spike` と `uv run scripts/mlx_bench.py`。
テスト音声は`say`生成の日本語17.2秒 / 英語13.3秒（`assets/`、16kHzモノラル）。

## 最重要の発見: ggmlシンボル衝突

**whisper-rs（0.16.0）とllama-cpp-2（0.1.154）を1バイナリに静的リンクすると、リンクは通るが実行時に壊れる。**
両クレートがそれぞれ別バージョンのggmlを同梱しており、C シンボルがリンク時に片方へ統合される。
whisper.cppの処理がllama.cpp側のggml（バージョン不一致）を呼び、バッファサイズ計算の不整合で
`ggml_tallocr_alloc: not enough space`（16バイト不足）→ SIGABRT。CPUモードでも
`GGML_ASSERT(device) failed`で落ちる。featureフラグで片方だけをリンクすれば完全に動作する。

単一プロセス構成（ADR-153800）を保つための選択肢:

1. **cdylib分離（推奨候補）**: 翻訳側（llama-cpp-2）をC ABIの薄いcdylibにまとめ、`libloading`でロード。
   macOSはtwo-level namespace、WindowsはDLL単位のシンボル解決により、ggmlの二重共存が安全になる。
   プロセスは1つのまま。v1がwhisperバックエンドをdylibシムにしていたのは同じ理由と思われる。
2. **テキストサイドカー**: llama.cppを別プロセスにし文単位でstdio通信。実装は最も簡単で、v1の痛みだった
   音声のbase64パイプと違いテキストなのでオーバーヘッドは無視できる。ただしプロセス監視が復活する。
3. **ggmlバージョンの一致するcrateバージョンの組をピン留め**: 動く組み合わせを探す。脆く、更新のたびに壊れうる。

→ Step 2（翻訳レーン実装）の着手時にcdylib方式を検証し、ADRとして確定する。

### 追記（2026-08-18）: cdylib分離の検証 — 成功

`crates/kkm-translate-ggml`（llama-cpp-2をC ABIのcdylibに隔離）を
whisper-rs静的リンク済みバイナリから`libloading`でロードし、
ウィンドウ再デコード（負荷あり）と文翻訳を**同一プロセスで同時実行**して確認:

- SIGABRTなし。ASR・翻訳とも正しい出力（両方Metal有効）
- 翻訳 0.9〜2.1s/文、ASR最悪ウィンドウデコード 868ms（同時負荷下）
- ピークRSS 3791MB（単一プロセス。2プロセス近似の3.9GBと同等）

再現: `cargo build --release -p kkm-translate-ggml` →
`cargo run --release -p spike --bin cdylib-check --no-default-features --features asr`

→ **cdylib方式で確定してよい**。Step 2着手時にADR化し、`Translator` trait実装を
このC ABI上に載せる（traitが同期的なのはABI境界と相性が良いことも確認済み）。

## A/B比較: ggml vs MLX

同一音声・同一文・同一重み級（whisper large-v3-turbo / Qwen3-4B-Instruct-2507 4bit級）。
ウィンドウ再デコードは window≤15s / step 1s、LocalAgreement相当のスケジューリング。

### ASR（whisper large-v3-turbo）

| 指標 | ggml q8_0 (whisper-rs) | MLX fp16 (mlx-whisper) | 予算 |
|---|---|---|---|
| ウィンドウ再デコード (ja) | 697〜815 ms | 598〜767 ms | — |
| 最初の部分結果 (ja) | 音声1s + 694 ms ≈ 1.7s | 音声1s + 598 ms ≈ 1.6s | ≤2s ✅ 両方 |
| フルデコードRTF (ja) | 0.07 | 0.04 | — |
| 言語自動判定モード (en) | 1355〜1499 ms/窓 | 未計測 | 判定コストが毎回乗る |
| 転写品質 | 完全一致（誤りなし） | 完全一致（誤りなし） | — |
| ピークRSS | 1.1 GB（単体プロセス） | —（下記合算） | — |

補足: ggml **q5_0はq8_0より遅い**（デコード805〜1211ms）。Metalでの逆量子化コストのため、
サイズ最小＝速いとは限らない。q8_0を既定候補とする。

### 翻訳（Qwen3-4B-Instruct-2507）

| 指標 | ggml Q4_K_M (llama-cpp-2) | MLX 4bit (mlx-lm) |
|---|---|---|
| 短文 (9〜17 tok) | 287〜541 ms | 322〜551 ms |
| 長文 (44 tok) | 871 ms | 819 ms |
| 生成速度 | 31〜51 tok/s | 31〜55 tok/s |
| 訳質 | 良好（byte蓄積修正後） | 良好 |

### メモリ

| 構成 | ピークRSS |
|---|---|
| ggml: whisper単体 + LLM単体（2プロセス合算） | 約3.9 GB |
| MLX: 両モデル同一Pythonプロセス | 4.3 GB |

### 同時稼働（ggmlのみ、2プロセスで近似）

| | 単体 | 同時 | 劣化 |
|---|---|---|---|
| ASRウィンドウデコード | 697〜815 ms | 689〜887 ms | +5〜8% |
| 翻訳（短文） | 287〜541 ms | 1311〜2232 ms | 3〜4倍 |
| 翻訳（長文44tok） | 871 ms | 3975〜4065 ms | 約4.6倍 |

whisperが1秒ステップごとに約0.7秒GPUを占有するため翻訳側が絞られる。それでも最悪4秒/文で、
文単位翻訳が会話ペースに追いつく要件は満たす。ASRのステップ間隔を適応制御（busy時にスキップ）
すれば翻訳側にGPUを譲る余地がある。

## 結論

1. **速度はMLXとggmlでほぼ互角**（ASRでMLXが1割強速い程度、翻訳は同等）。メモリも互角。
   クロスプラットフォーム一本化のメリットを覆すほどの差はなく、**ggml路線を維持**する。
   MLX採用条件（plan.md）は「ggmlが予算未達の場合」であり、実測は全予算クリア。
2. レイテンシー予算は**全項目クリア**: 最初の部分結果1.7秒（≤2秒）、翻訳は同時稼働でも会話ペース内。
3. アーキテクチャ上の残課題は**ggmlシンボル衝突の解消**のみ。cdylib分離をStep 2で検証する。
4. 言語自動判定は毎窓約2倍のコスト。Main/Sub 2言語運用では「確定時のみ判定」等の設計が必要。
