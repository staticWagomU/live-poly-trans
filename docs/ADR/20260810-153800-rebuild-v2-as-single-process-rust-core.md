# v2は単一Rustプロセス＋trait抽象のコアとして作り直す

| | |
|---|---|
| **Status** | proposed |
| **Date** | 2026-08-10 |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code（v1コードベース調査に基づく分析） |
| **Informed** | - |

## Context and Problem Statement

v1はTauri → Swiftヘルパー → Rustサイドカー → dlopenしたC++シム → whisper.cpp という多段プロセス構成になり、音声をbase64エンコードしたNDJSONでパイプ間受け渡ししていた。レイテンシー・メモリ・ビルドの脆さ（Homebrewパス依存、ヘルパーの二重ビルド、ad-hoc署名）の根源がこの構造にあり、`src-tauri/src/lib.rs`は4775行まで肥大化した。v2ではWindows対応も見据えて構造から作り直す。

## Decision Drivers

* 【MUST】低レイテンシー: プロセスホップごとのシリアライズ（base64 PCM）を排除したい
* 【MUST】低メモリ: プロセスごとのランタイム・バッファの重複を排除したい
* 【MUST】将来のWindows対応: macOS専用ヘルパー前提の構造を捨てたい
* 保守性: v1は機能追加のたびに`lib.rs`とヘルパー間プロトコルの双方が肥大化した

## Considered Options

1. 単一Rustプロセスに集約し、ASR・翻訳・音声取得をtraitで抽象化する
2. v1の多段プロセス構成を維持し、個々のコンポーネントだけ改善する
3. コアをすべてSwiftに寄せる（macOSネイティブ最適化）

## Decision Outcome

**Chosen option**: 「単一Rustプロセスに集約し、ASR・翻訳・音声取得をtraitで抽象化する」。プロセス間の音声シリアライズが消えることでレイテンシーとメモリのMUST要件に直結し、Rustコアはmac/Winで共通化できるため。OS依存部分（スピーカー音声取得）だけを薄いアダプタ層に閉じ込める。

### Consequences

**Positive:**
* 音声フレームがプロセス内のチャネル渡しになり、base64/NDJSONのオーバーヘッドが消える
* ビルドが単純化する（Swiftヘルパーの二重ビルド・署名・パス解決10候補が不要になる）
* trait差し替えでASR/翻訳エンジンをテストダブルに置き換えられ、TDDしやすい

**Negative:**
* ASRエンジンがクラッシュするとアプリ全体が落ちる（v1はヘルパーの再起動で隔離できていた）
* whisper.cppのFFI境界のメモリ安全性をコア内で管理する必要がある

**Neutral:**
* macOS固有API（CoreAudio等）はRustからFFI経由で呼ぶことになる

### Confirmation

* コアクレートがUIなし（headless）で単体テスト可能であること
* 音声フレームの取得からASR入力までにシリアライズが存在しないこと（コードレビューで確認）
* アイドル時の常駐メモリがv1同等以下であること

## Pros and Cons of the Options

### 単一Rustプロセス＋trait抽象

* Good: プロセスホップとシリアライズの排除がレイテンシー・メモリに直接効く
* Good: mac/Winでコアを共有できる
* Bad: 障害隔離がなくなる（パニック境界の設計が必要）

### v1の多段プロセス構成を維持

* Good: 障害隔離と権限分離（TCC）が既に動いている
* Bad: v1の痛みの根源（シリアライズ・ビルドの脆さ・プロトコル保守）がそのまま残る
* Bad: SwiftヘルパーはWindowsに持っていけない

### コアをSwiftに寄せる

* Good: macOSのメディアAPIとの親和性は最高
* Bad: Windows対応で全面書き直しになり、MUST要件と矛盾する

## More Information

関連: [ASR選定](20260810-153801-local-whisper-asr-in-process.md)、[音声取得](20260810-153803-audio-capture-cpal-and-os-taps.md)。v1実装は`v1`ブランチに保存されている。
