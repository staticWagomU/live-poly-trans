# **LivePolyTrans - 多言語リアルタイム同時翻訳アプリ（Tauri版）**  
**完全企画書 v1.0**（2026年6月対応・非IT知人向け配布最適）

### 1. アプリコンセプト（知人向け説明文）
「インストールしてある言語（例: 日本語＋英語、または日本語＋中国語など）だけを選択してして、マイクとスピーカーの音声を**同時に**文字起こし＋即時翻訳してくれるアプリ。  

### 2. コア機能（更新版）

- **動的言語対応**  
  - 起動時に `SpeechTranscriber.supportedLocales` / `installedLocales` を調べる。
  - 例: 日本語＋英語 → ja/en同時文字起こし＋相互翻訳  
  - 例: 日本語＋中国語 → ja/zh同時  
  - 3言語以上インストール済みでも「メイン2言語」を優先（設定で手動選択可）

- **2独立ストリーム**  
  - マイク（自分の声）  
  - スピーカー（他者の声 / Zoomなどシステム音声）

- **各ストリームで**  
  - インストール済み言語すべてでリアルタイム文字起こし（SpeechAnalyzer + 複数 SpeechTranscriber）  
  - 即時対訳（TranslationSession）

- **長時間耐久**（8時間以上安定）  
  - Rust + C++ Ring Buffer  
  - 15分ごと自動セグメント保存（JSON + TXT）

- **UI**（シンプル・非IT向け）  
  - 大きなスタートボタン  
  - 必要に応じて録音を開始
  - LINEのような自分の音声と他者の音声をチャット風UIで表示
  - 言語自動表示＋手動切替ボタン  
  - 「コピー」「保存」「クリア」ボタン

### 3. 技術構成（Tauri 2.x + Swift sidecar）

| 層                | 技術                          | 理由 |
|-------------------|-------------------------------|------|
| **UI / フロント** | SvelteKit + Tauri | 軽量・美しいUI |
| **バックエンド**  | Rust（Tauri Commands）        | 安全性・高速 |
| **Apple API**     | Swift sidecar（別バイナリ）   | SpeechAnalyzer / Translation をフル活用 |
| **音声取得**      | Swift sidecar（Core Audio Tap + AVAudioEngine） | システム音声完璧 |
| **言語検知**      | Swift側で `SpeechTranscriber.installedLocales` | 動的・正確 |
| **C++高速化**     | sidecar内に埋め込み           | 長時間Ring Buffer |
| **配布**          | Tauri `tauri build` → dmg/pkg（5〜12MB） | 超軽量 |

**sidecar方式のメリット**  
- Xcodeで普通にSwift開発可能  
- Rustから `tauri::process::Command` で起動・IPC（JSON）  
- Tauri公式推奨（`externalBin`）

### 4. プロジェクト構造（Tauri標準）

```
live-poly-trans/
├── src/                  # Svelte/React UI
├── src-tauri/
│   ├── src/              # Rustコマンド
│   ├── binaries/         # Swift sidecar（macOS用）
│   │   ├── helper-aarch64-apple-darwin
│   │   └── helper-x86_64-apple-darwin
│   ├── tauri.conf.json
│   └── Cargo.toml
├── swift-helper/         # Xcodeプロジェクト（別フォルダ）
└── README.md（知人向け）
```

### 5. Swift sidecar（helper）の役割（必須機能）

- 起動引数 `--stream mic` / `--stream speaker` / `--detect-languages`
- 言語検知コマンド → インストール済み言語リストをJSON返却
- リアルタイム処理 → 標準出力に常時JSON流し（`{"stream":"mic","lang":"ja","text":"...","trans":"..."}`）
- C++ Ring Buffer内蔵（長時間耐久）
- 自動終了・再起動対応

### 6. 実装フェーズ（あなたがすぐ始められる）

**Phase 0**  
`cargo tauri init` → Svelteテンプレート → dmgビルド確認

**Phase 1**  
Swift sidecar作成（言語検知＋マイクのみ）＋ Tauriから起動確認

**Phase 2**  
スピーカー音声 + 複数言語同時 + Translation

**Phase 3**  
UI完成＋設定画面＋長時間テスト

**Phase 4**  
dmg配布用インストーラー（ドラッグ＆ドロップアニメーション付き）

---

### 7. 非IT知人向け配布イメージ

- ファイル1つ：`LivePolyTrans-1.0.dmg`（約8MB）
- 中身：アプリ本体＋「はじめてガイド.app」  
- 初回起動時：  
  「こんにちは！このアプリはあなたがインストール済みの言語を自動で使います。  
  マイクと画面録画の許可を2回お願いします👍」
