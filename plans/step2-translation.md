# Step 2: 確定文の翻訳レーン

`utterance_final` を入力に、確定文をローカルLLMで訳して確定行に後付けする。

## 決定事項（2026-08-19）

- **言語モデル**: plan.md の Main/Sub を廃し、`mockups/feature-language-picker.html` 案Aの
  **「話される言語（最大2）」×「翻訳先（なし可）」＋相互翻訳フラグ** を採用する。
  Main/Sub の1語では「単一言語・訳なし」が表現できない。
- **UI範囲**: 訳文の後付け表示＋ヘッダの最小言語切替まで。ピル＋ポップオーバーは後続のUI刷新で。
- **ModelManager**: ローカルパス解決の共通化まで（env override → 探索ディレクトリ）。ダウンロードは後続。

## 構造

```
utterance_final ──> TranslationQueue（バックプレッシャー）──> 翻訳ワーカースレッド
   (pipeline)          kkm-core（純ロジック）                 kkm-translate（dlopen）
                                                                    │
   pipeline のポーリングループが結果を回収 ──> transcript イベント / transcript.jsonl
```

翻訳結果をワーカーから直接 emit せず **pipeline に返す** のは、`SessionRecorder` の
単一所有を崩さないため。ポーリング間隔100msの遅延はレイテンシー要件（文単位）に対して無視できる。

### バックプレッシャー

キューは有限長。溢れたら **最古を捨てる**。会議中に価値があるのは画面に出ている最新の文で、
1分前の訳文を待って現在の文を詰まらせるのは本末転倒。捨てた `id` はUIに通知し、
「訳しています…」の表示を残さない。

### 翻訳方向

`LanguagePolicy::target_for(source)`:

| 状況 | 結果 |
|---|---|
| 翻訳先なし | 訳さない |
| source == 翻訳先、相互off | 訳さない（既に読める） |
| source == 翻訳先、相互on、話される言語が2つ | もう一方の言語へ |
| source != 翻訳先 | 翻訳先へ |
| source 不明（検出が不確信） | 翻訳先へ（元言語は指定せずプロンプトする） |

話される言語が1つなら `transcribe(_, Some(lang))` に固定され `lang_detect` を丸ごと省ける。

## 作業（TDD）

- [x] `kkm-core::language`: `LanguagePolicy`（`pinned_lang` / `target_for`）と言語表示名
- [x] `kkm-core::translate`: `TranslationQueue`（有限長・最古退避）
- [x] `kkm-core::models`: モデルパス解決（env override → 探索ディレクトリ）
- [x] `scheduler::Utterance` に `lang` を追加（翻訳元の判定に要る）
- [x] `crates/kkm-translate`: cdylib の dlopen ローダ（`Translator` 実装）＋探索順のテスト
- [x] cdylib: 元言語を指定しないプロンプト経路
- [x] `src-tauri/src/translate.rs`: ワーカースレッド（遅延ロード・失敗は通知のみ）
- [x] pipeline 配線: 発話ID採番・投入・結果回収・Stop時のドレイン
- [x] `set_languages` / `get_languages` コマンドと録音中の反映
- [x] UI: 確定行への訳文後付け、ヘッダの言語切替
- [x] `.app` バンドルへの dylib 同梱（`tauri.conf.json` の `macOS.frameworks` ＋ `scripts/build-app.sh`）
- [x] ADR-153805（cdylib分離の決定を記録。Step 0の宿題）
- [ ] 実発話での確認 — `./scripts/build-app.sh --open`

## 実装後に気づいて直したこと

- **アイドル中の言語変更が反映されない**: エンジンはセッションをまたいで生き残るので、
  「ポリシーが変わったか」ではなく「エンジンが何を知っているか」と比べる必要があった（`Engines::spoken`）。
- **Stopドレイン中の通知でステータスが `listening` に戻る**: 停止処理中に翻訳が失敗すると
  UIの録音ボタンが「Stop」に戻ってしまう。`apply_translation` に状態を渡すようにした。
- **前セッションの訳文が新しい `transcript.jsonl` に混ざる**: ドレインがタイムアウトした後に
  届いた訳文は、画面の行には出すべきだが、その発話を含まないファイルに書いてはいけない（`first_id`）。
