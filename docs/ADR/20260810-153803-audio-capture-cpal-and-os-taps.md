# 音声取得はcpal＋OS別ループバック（CoreAudio Process Tap / WASAPI）にする

| | |
|---|---|
| **Status** | proposed |
| **Date** | 2026-08-10 |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code（v1のScreenCaptureKit実装の調査） |
| **Informed** | - |

## Context and Problem Statement

マイクとスピーカー（システム音声）を別レーンで取得・録音するのがMUST要件。v1はSwiftヘルパー内でマイクをAVAudioEngine、スピーカーをScreenCaptureKitで取得していたが、ScreenCaptureKitは「画面収録」権限を要求するため、権限取得のためだけにヘルパーを.appバンドル化する複雑さを抱えていた。Windows対応も必要になる。

## Decision Drivers

* 【MUST】マイク／スピーカーの独立レーン取得と録音
* 【MUST】Windows対応: 取得層以外はOS非依存にしたい
* 権限要求は最小限にしたい（「画面収録」権限は過剰で、ユーザーへの説明も難しい）
* 単一プロセス構成（ADR-153800）に収まること

## Considered Options

1. マイク=cpal（クロスプラットフォーム）、スピーカー=macOSはCoreAudio Process Tap、WindowsはWASAPIループバック
2. v1方式の踏襲（AVAudioEngine＋ScreenCaptureKitのSwiftヘルパー）
3. 仮想オーディオデバイス（BlackHole / VB-Cable等）の導入をユーザーに求める

## Decision Outcome

**Chosen option**: 「cpal＋OS別ループバックAPI」。マイク取得はcpalでOS非依存にし、OS固有なのはスピーカー取得のみに限定する。macOSはCoreAudio Process Tap（macOS 14.2+）を使うことで「画面収録」ではなく音声キャプチャ権限だけで済み、v1の.appバンドル化ヘルパーが不要になる。WindowsのWASAPIループバックは標準機能で追加権限も不要。

### Consequences

**Positive:**
* 「画面収録」権限とヘルパーの.appバンドル化・二重ビルドが不要になる
* スピーカー取得層だけがOS別実装で、それ以外のパイプラインは共通
* WASAPIループバックはWindows標準機能で、仮想デバイスの導入案内が不要

**Negative:**
* CoreAudio Process TapのRustからの利用は事例が少なく、FFI（objc2/coreaudioクレート群）の検証が必要
* 最低対応OSがmacOS 14.2+になる（v1のmacOS 26+よりは大幅に緩和）

**Neutral:**
* 録音はv1同様、無音ゲート等の加工前の生音声をレーン別に保存する（この設計はv1で正しく機能していた）

### Confirmation

* Spike: RustからProcess Tapでシステム音声のPCMが取得できること（v2最初の技術検証項目）
* マイク／スピーカー各レーンの録音ファイルの時刻が文字起こしのタイムスタンプと一致すること

## Pros and Cons of the Options

### cpal＋OS別ループバックAPI

* Good: 権限が最小で、取得層以外を完全共通化できる
* Good: 依存が標準APIのみ（仮想デバイス等の外部インストール不要）
* Bad: Process TapのRust FFIは自前検証が必要（失敗時はSwift薄ヘルパーにフォールバック）

### v1方式の踏襲

* Good: 動作実績がある
* Bad: 画面収録権限・.appバンドル・二重ビルドの複雑さが残り、Windowsに持っていけない

### 仮想オーディオデバイス導入

* Good: 実装は単純（通常の入力デバイスとして読むだけ）
* Bad: ユーザーにドライバ導入と音声ルーティング設定を強いる。品質・遅延も構成依存

## More Information

前提: [単一プロセスアーキテクチャ](20260810-153800-rebuild-v2-as-single-process-rust-core.md)。v1の録音設計（gate前保存・タイムライン保持）は`v1`ブランチの`swift-helper/Sources/LivePolyTransHelper/AudioRecorder.swift`と`AudioSilenceGate.swift`を参照。
