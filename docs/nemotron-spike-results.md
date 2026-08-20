# Nemotron Streaming スパイク結果（2026-08-20, Apple M4 Pro）

plan.md Step 6の実測。再現:

```sh
cargo run --release -p spike --bin nemotron-check \
  --no-default-features --features parakeet -- ~/Music/kikimimic/20260819160920/mic.wav
```

テスト音声は**このアプリが実際に録った**セッション `20260819160920` のmicレーン（101.6秒、日英混在、
`transcript.jsonl`に現行Whisperの認識結果が残っているのでそのままA/Bになる）。

## モデルとランタイム

| 項目 | 値 |
|---|---|
| モデル | `nvidia/nemotron-3.5-asr-streaming-0.6b` の GGUF Q8_0（717MB） |
| アーキテクチャ | `parakeet` / head = **RNN-T**。`stt.capability.streaming = true` |
| ランタイム | [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp)（MIT、ggml）＋ crates.io の `transcribe-cpp` 0.2.1 |
| 入手経路 | Handyが落としたものを **HFキャッシュ共有でそのまま使用**（コピー不要） |
| 言語 | 32ロケール。日本語は **`ja-JP`**（後述） |

ストリーミングAPIが`begin` / `feed(pcm) -> StreamUpdate` / `finalize`で、
`StreamText`が`{ full, committed, tentative }`。**`committed` / `tentative` が
そのままこちらの `committedDelta` / `volatile` に対応する**。

## レイテンシーとスループット

| 指標 | Nemotron Streaming | 現行 Whisper large-v3-turbo q8_0 | 予算 |
|---|---|---|---|
| モデルロード | 217ms（初回のみMetalシェーダ生成で8.1s） | — | — |
| 1チャンク処理（320ms分） | 平均 **8ms** / 最悪 77ms | ウィンドウ再デコード 697〜815ms | チャンク長320ms未満 ✅ |
| RTF | **0.027** | 0.07 | — |
| 確定の遅れ（音声換算） | 平均 **150ms** / 最悪 230ms | 最初の部分結果 ≈1.7s | 確定≤1500ms ✅ |
| ピークRSS | **980MB** | 1.1GB | 低メモリ（MUST） |

チャンク処理が平均8msというのは、リアルタイムの**約36倍**の速度で流し込めるということ。
擬似ストリーミングのように「窓を毎回引き直す」必要がないので、音声が増えた分しか計算しない。

## 認識精度のA/B（同一音声・micレーン）

| 位置 | 現行 Whisper | Nemotron（言語自動判定） |
|---|---|---|
| 2.3–6.1s | Hello everyone. | Hello everybody |
| 9.0–10.8s | おはようございます | おはようございます |
| 15.0–18.8s | いい感じですね | **ね**（大半を落とす） |
| 50.3–53.3s | Thank you. | **（何も出さない）** |
| 74.0–78.3s | ちゃんと反応してるかな | 、ちゃんと反応してるかな |
| 82.2–85.0s | 動いてそうだ。 | 動いてそうだ |

一見Nemotronが2箇所負けているが、**切り出して単独で流すと結論が逆になる**。

- 15–18.8sを7秒だけ切って投入 → **「もういい感じですね。」**。
  Whisperが落とした「もう」まで拾い、句点まで付く。つまりモデルの実力ではなく、
  **無音を含む長い連続ストリームを流し込んだこと**が原因で落ちている。
- 50.3–53.3s を切って投入 → **やはり何も出さない**。この区間の音量を測ると **-55.0 dBFS**
  （こちらの無音ゲート閾値 -50 dBFS より下）。つまり**無音**であり、
  現行の `transcript.jsonl` に残っている "Thank you." は **Whisperの幻覚**。
  Nemotronは正しく何も出していない。

→ 精度で劣るという結論は**出ていない**。この比較はNemotron側にVAD・無音ゲートを
一切与えていない不公平な条件であり、公平にするとむしろ有利な材料が出た。

## 落とし穴: 言語コード

`KKM_LANG=ja` は `stream begin: unsupported language (status 10)` で**失敗する**。
プロンプト辞書（121ロケール）に `en` `es` `fr` `de` `ru` `ko` などの素の2文字コードはあるのに、
**`ja` だけ無く `ja-JP` / `ja-JA` しかない**。こちらの `LanguagePolicy` はISO 2文字で持っているので、
エンジン境界でロケールへ写す変換が要る（`auto` も予約語として使える）。

## このスパイクが答えていないこと

1. **VADをどこから持ってくるか。** 現在のSilero VADは whisper.cpp 由来なので、
   whisper-rsを外すとVADも消える。上の「長い連続ストリームで落ちる」現象からして、
   NemotronにもVADで区切った音声を渡すほうが良い可能性が高い。
2. **確定を誰が持つか。** `CommitPolicy::Auto`（ライブラリのstable-prefix実装）で測っており、
   `stable_prefix_agreement_n` は**こちらのLocalAgreementと同じ仕組み**。
   二重に持つ意味は薄く、どちらかに寄せる判断が要る。
3. **ggml隔離。** transcribe.cppは3つ目のggmlなので、ADR-153805と同じくcdylib＋dlopenが要る。
   ただしtranscribe.cppは**Whisperも動かせる**ので、ASRごと寄せればggmlは2つに減る。
   複数モデルを扱う前提なら、こちらのほうが構成として素直になりうる。
4. **発話境界。** `committed`は文の区切りを持たない連結テキスト。
   `Utterance{text,start_ms,end_ms}`に落とすには区切りの供給元が要る（VADか、`Segment`のタイムスタンプか）。
