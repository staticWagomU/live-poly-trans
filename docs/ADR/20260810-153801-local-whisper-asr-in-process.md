# 文字起こしはローカルWhisper（whisper-rsインプロセス）一本にする

| | |
|---|---|
| **Status** | proposed |
| **Date** | 2026-08-10 |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code（エンジン比較調査） |
| **Informed** | - |

## Context and Problem Statement

「確定した単語がリアルタイムに表示される」文字起こしがv2のMUST要件だが、v1のデフォルトだったApple Speech（SpeechAnalyzer）はmacOS 26専用で、Windows対応の障害になる。またv1のWhisperモードは4ホップのプロセスチェーン経由で、モデル性能ではなくアーキテクチャがレイテンシーの主因だった。会議音声を扱うため機密性の観点もある。

## Decision Drivers

* 【MUST】単語単位のリアルタイム表示（発話終了待ちの文字起こしは不可）
* 【MUST】高精度な日本語・英語認識、および両言語混在への対応
* 【MUST】mac/Windowsで同一実装を使いたい
* 会議音声を外部送信しない（機密性・ランニングコストゼロ）
* Windows側マシンのスペックが未定のため、モデルサイズは可変にしたい

## Considered Options

1. whisper-rs（whisper.cppインプロセス）＋ LocalAgreement方式の擬似ストリーミング
2. クラウドストリーミングASR（Soniox / Speechmatics / Deepgram等）
3. OSネイティブ併用（macはApple Speech、WindowsのみローカルWhisper）
4. sherpa-onnx＋軽量モデル（SenseVoice-small等）

## Decision Outcome

**Chosen option**: 「whisper-rs＋LocalAgreement擬似ストリーミング」。機密性とコストの制約からローカル一本とし、日本語精度と言語混在対応で現状最良のWhisper系を採る。v1のWhisperモードの痛みは多段プロセスというアーキテクチャ税であり、インプロセス化で解消できると判断した。モデルは設定で差し替え可能にし（large-v3-turbo量子化〜small）、Windows側のスペック確定時に再選定する。sherpa-onnx＋SenseVoice-smallは低メモリ側の代替候補として検証対象に残す。

### Consequences

**Positive:**
* 会議音声が端末外に出ない。ランニングコストもゼロ
* mac（Metal）/Windows（CPU/CUDA/Vulkan）で同一コードパス
* エンジンをtrait化するため、将来クラウドや別モデルの追加が閉じた変更で済む

**Negative:**
* 真のストリーミングモデルではないため、部分結果の初速は1〜2秒程度が下限になる（クラウドの数百msには届かない）
* 再デコード方式はCPU/GPUを常時消費する。モデルメモリも量子化turboで1GB超
* Whisperの言語混在認識はチャンク単位の言語判定に依存し、文中コードスイッチには弱い

**Neutral:**
* モデルファイルの取得・配置フローをアプリ側で持つ必要がある

### Confirmation

* 計測: 発声から最初の部分結果まで2秒以内、発話終了から確定まで1.5秒以内（v1の計測予算を踏襲）
* 日英混在の会議音声サンプルで、言語誤判定によるレーン崩れが実用範囲であること
* ASRエンジンtraitのテストダブルでコアのユニットテストが通ること

## Pros and Cons of the Options

### whisper-rs＋LocalAgreement擬似ストリーミング

* Good: 日本語精度・多言語対応でローカル最良クラス
* Good: whisper.cppはmac/Win両対応で実績豊富、Rustバインディング（whisper-rs）が保守されている
* Bad: 擬似ストリーミングの実装（安定化・確定判定）を自前で持つ
* Bad: 常時再デコードのCPUコスト

### クラウドストリーミングASR

* Good: 単語単位・低レイテンシー・言語混在すべて要件に最も素直に合う
* Bad: 会議音声の外部送信が前提となり、機密性の制約に反する
* Bad: 従量課金（目安$1〜2/時間）

### OSネイティブ併用

* Good: macではApple Speechの品質（v1で実証済み）をそのまま使える
* Bad: エンジン2系統のメンテナンスとなり、挙動差の吸収が恒常的な負担になる
* Bad: v1で複雑さの主因だった並列認識＋アービターをmac側で持ち続けることになる

### sherpa-onnx＋SenseVoice-small

* Good: メモリ数百MBで最軽量、クロスプラットフォームのC API
* Bad: 精度はWhisper largeに一歩譲る。日本語の真のストリーミングモデルは選択肢が薄い

## More Information

前提: [単一プロセスアーキテクチャ](20260810-153800-rebuild-v2-as-single-process-rust-core.md)。関連: [翻訳バックエンド](20260810-153804-embedded-local-llm-translation.md)。v1のLocalAgreement類似実装（rolling re-transcription・安定化）は`v1`ブランチの`src-tauri/src/whisper_engine_*.rs`が参考になる。
