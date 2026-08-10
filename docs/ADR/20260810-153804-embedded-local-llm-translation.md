# 翻訳はアプリ組み込みのローカルLLM推論を既定とし、モデルはユーザーが選択・ダウンロードする

| | |
|---|---|
| **Status** | proposed |
| **Date** | 2026-08-10（同日改訂: 当初のOllama既定案を置き換え） |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code |
| **Informed** | - |

## Context and Problem Statement

確定した文単位でのリアルタイム翻訳がMUST要件（例: 英語話者の発言を日本語ネイティブの利用者向けに翻訳）。当初はOllamaを既定バックエンドとする案だったが、外部ランタイムのインストールをユーザーに求める体験を避け、アプリ内でモデルを選んでダウンロードすれば翻訳が動く自己完結型にしたい。一方で、Ollama接続やCodex/Claude Code（CLIエージェント）接続も将来のバックエンドとして視野に入れている。

## Decision Drivers

* 【MUST】文単位のリアルタイム翻訳（レイテンシー要求はASRより緩い）
* 【MUST】Windows対応 — OS非依存の既定バックエンドが必要
* 会議内容を外部送信しないローカル既定
* セットアップの自己完結性: Ollama等の外部ツール導入をユーザーに求めない
* ASR側（Whisperモデル）でもモデルのダウンロード・選択の仕組みが必要であり、共通化できる
* 将来の拡張: Ollama / Codex / Claude Code / DeepL等をバックエンドとして追加したい

## Considered Options

1. アプリ組み込みのローカル推論（llama.cpp系）＋アプリ内モデル選択・ダウンロードを既定とし、`Translator` traitで外部バックエンドを追加可能にする
2. Ollama上のローカルLLMを既定にする（当初案）
3. Codex / Claude Code等のCLIエージェント接続を既定にする
4. DeepL APIを既定にする

## Decision Outcome

**Chosen option**: 「アプリ組み込みのローカル推論＋アプリ内モデル選択・ダウンロード」。外部ランタイム不要でセットアップが自己完結し、ローカル完結の方針（ADR-153801）とも整合する。モデル管理（一覧提示・ダウンロード・保存・切り替え）はWhisperモデルと共通の仕組み（ModelManager）として実装する。推論はwhisper.cppと同じggml系のllama.cpp（GGUFモデル）を採用候補とし、Metal/CPUバックエンドを共有する。

`Translator` traitの実装として、組み込み推論のほかにOllama（HTTP）、Codex/Claude Code（CLIサブプロセス）、DeepL（HTTP）を将来追加できる構造にする。既定はあくまで組み込み推論。

### Consequences

**Positive:**
* ユーザーはアプリ内でモデルを選んでダウンロードするだけで翻訳が使える（外部ツール導入なし）
* モデル管理がASR側と共通化でき、UI・保存領域・ダウンロード処理を一度だけ作ればよい
* whisper.cppとllama.cppがggml基盤を共有するため、ビルド・GPUバックエンドの知見が流用できる
* traitによりOllama / Codex / Claude Code / DeepLの追加が閉じた変更で済む

**Negative:**
* 推論エンジンをアプリに組み込む分、実装・ビルドの複雑さとバイナリサイズが増える
* ASR（Whisper）と翻訳LLMが同一プロセスでメモリ・GPUを奪い合う。モデルサイズの組み合わせ管理と、翻訳のキュー＋逐次処理（バックプレッシャー）が必須
* モデルダウンロードUX（進捗・ディスク容量・破損時の再取得）を自前で持つ

**Neutral:**
* 翻訳はASRの確定イベントから非同期に実行し、認識をブロックしない（v1の設計を踏襲）
* CLIエージェント接続（Codex/Claude Code）はレイテンシー・起動コストの面でリアルタイム翻訳の既定には不向きだが、要約や高品質な後処理翻訳の用途で価値がある

### Confirmation

* 新規環境で「モデルを選択→ダウンロード→翻訳が動く」までアプリ内で完結すること
* 文の確定から翻訳表示まで、通常の会話ペースで次の文が確定する前に追いつくこと
* ASR動作中に翻訳を並走させてもASRのレイテンシー計測が悪化しないこと
* `Translator` traitのテストダブルでコアのユニットテストが通ること

## Pros and Cons of the Options

### アプリ組み込み推論＋モデル選択・ダウンロード

* Good: セットアップ自己完結・ローカル完結・ASRとモデル管理を共通化できる
* Good: ggml基盤（whisper.cpp/llama.cpp）の共有でビルド知見が重複しない
* Bad: 推論エンジンとモデル管理を自前で抱える実装コスト

### Ollama既定（当初案）

* Good: 推論基盤を外部に任せられ、実装が薄い
* Bad: Ollamaのインストールとモデルpullをユーザーに求め、体験が自己完結しない
* Bad: 外部プロセスのライフサイクル（未起動・ポート競合）への対処が必要

### CLIエージェント接続（Codex / Claude Code）既定

* Good: 翻訳品質・文脈理解は最高クラスで、追加の推論基盤も不要
* Bad: プロセス起動・API課金・ネットワーク前提となり、リアルタイム翻訳の既定には不向き
* Bad: 会議内容の外部送信となりローカル方針に反する（オプトインの追加バックエンドとしては許容）

### DeepL API既定

* Good: 訳質が安定して高く、実装も単純
* Bad: 外部送信と従量課金が前提になり、ローカル方針と矛盾する

## More Information

前提: [ASR選定](20260810-153801-local-whisper-asr-in-process.md)（ModelManagerを共有）。v1のOllama/DeepL HTTPクライアント実装は`v1`ブランチの`src-tauri/src/translation_backend.rs`が将来のバックエンド追加時の流用候補。組み込み推論のRustバインディングはllama-cpp-2 / mistral.rs等をStep 2着手時に比較する。
