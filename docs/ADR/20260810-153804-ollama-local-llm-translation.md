# 翻訳はローカルLLM（Ollama）を既定とし、trait差し替え可能にする

| | |
|---|---|
| **Status** | proposed |
| **Date** | 2026-08-10 |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code |
| **Informed** | - |

## Context and Problem Statement

確定した文単位でのリアルタイム翻訳がMUST要件（例: 英語話者の発言を日本語ネイティブの利用者向けに翻訳）。v1はApple Translationを既定とし、DeepL/Ollamaは「配線途中」のまま完成しなかった。Apple TranslationはmacOS専用のためv2の既定にはできない。

## Decision Drivers

* 【MUST】文単位のリアルタイム翻訳（単語単位は不要なので、レイテンシー要求はASRより緩い）
* 【MUST】Windows対応 — OS非依存の既定バックエンドが必要
* ASRと同様、会議内容を外部送信しないローカル既定が望ましい
* 文脈（直前の発言・用語）を踏まえた訳質を将来改善したい

## Considered Options

1. Ollama上のローカルLLM（qwen3 / gemma3等。日英特化ならPLaMo-2-translate）を既定とし、DeepL等をtraitで差し替え可能にする
2. DeepL APIを既定にする
3. 専用機械翻訳モデル（NLLB等）をアプリに組み込む

## Decision Outcome

**Chosen option**: 「Ollama既定＋trait差し替え」。ローカル完結の方針（ADR-153801）と整合し、Ollamaはmac/Win両対応で導入も容易。LLMなら将来、直前の会話文脈や用語集をプロンプトに入れる改善が同じ仕組みの上でできる。翻訳は`Translator` traitに切り、DeepL等のHTTPバックエンドは必要になった時点で追加する。

### Consequences

**Positive:**
* 会議内容が端末外に出ず、ランニングコストもゼロ
* 文脈利用・語調指定などプロンプトでの訳質改善の余地がある
* traitにより翻訳なし／別バックエンドへの切り替えが閉じた変更で済む

**Negative:**
* Ollamaのインストールとモデル取得をユーザーに求める（アプリ組み込みではない）
* ASRとLLMが計算資源を奪い合う。翻訳はキュー＋逐次処理にして、v1のスレッド乱発（発話ごとにOSスレッド生成）を繰り返さない
* 小型LLMの訳質はDeepLに劣る場面がある

**Neutral:**
* 翻訳はASRの確定イベントから非同期に実行し、認識をブロックしない（v1のこの設計は踏襲する）

### Confirmation

* 文の確定から翻訳表示まで、通常の会話ペースで次の文が確定する前に追いつくこと
* ASR動作中に翻訳を並走させてもASRのレイテンシー計測が悪化しないこと（バックプレッシャーの確認）

## Pros and Cons of the Options

### Ollama＋ローカルLLM既定

* Good: ローカル完結・クロスプラットフォーム・文脈利用の発展性
* Bad: 外部ランタイム（Ollama）依存と、ASRとの資源競合の管理が必要

### DeepL API既定

* Good: 訳質が安定して高く、実装も単純
* Bad: 外部送信と従量課金が前提になり、ローカル方針と矛盾する

### 専用MTモデル組み込み

* Good: 外部ランタイム不要で配布が自己完結する
* Bad: モデル管理・推論基盤を自前で持つ負担が大きく、訳質改善の柔軟性も低い

## More Information

前提: [ASR選定](20260810-153801-local-whisper-asr-in-process.md)。v1のOllama/DeepLクライアント実装は`v1`ブランチの`src-tauri/src/translation_backend.rs`が流用候補。
