# LivePolyTrans 改善 + 録音機能追加 実装計画

対象: 問題点 1-1〜1-6 / 2-1〜2-8(2-2は現行デバウンス維持)/ 3の可能な範囲 / 4、
および録音機能の新規追加。

## 方針

言語採用判定をSwiftヘルパー内へ移動し「1発話=1イベント」で出力する。
これにより interim のチラつき(1-1)・二重バブル(1-2)・AIコンテキスト汚染(2-7)を
根本から解消する。AIは常駐サーバー化してモデルロードを1回にする(2-1)。

## Phase 1: Swift — 言語アービトレーション(1-1, 1-2)

- [x] `TranscriptArbitration.swift`: 純粋関数コア
  - 時間範囲オーバーラップによる言語間セグメント対応付け(250ms開始時刻比較を廃止)
  - スコアリング: confidence(重み付き) + NL言語判定一致 + 文字種フィットネス + 密度
  - 勝者テキストの結合(同言語の連続finalを時間順に連結)
- [x] `TranscriptArbiter` actor: volatile/finalの集約と単一イベント出力
  - volatile: 両言語の最新候補から勝者1件のみ emit(レーンはstream単位)
  - final: 反対言語の同区間finalを最大1.2s待って判定、勝者のみ emit
- [x] `emitResults` をアービター経由に変更。stdout出力は EventEmitter で直列化
- [x] テスト(TestSupportハーネスに追加)

## Phase 2: Swift — 安定化(1-3, 1-4, 1-6, 1-7)

- [x] 翻訳の非同期化: finalは trans=null で即emit、翻訳完了時に
      `{type:"translation", stream, segmentId, trans}` を追送(1-3)
- [x] AudioSilenceGate: 破棄開始を2.5sに延長しfinalize用の無音を確保(1-4)
- [x] `translation skipped` 等の非致命ログを debug プレフィックスへ(1-6)
- [x] 言語パック未インストール時に AssetInventory でダウンロード試行、
      `{type:"status"}` イベントで進捗通知(1-7)

## Phase 3: Swift — AI常駐サーバー(2-1, 2-4, 2-5, 2-6)

- [x] `--ai-server`: stdin/stdout JSONL のリクエストループ
      `{id, command, question?, language?, previousSummary?, history?, transcript}`
- [x] prewarm + プロセス常駐でモデルロードを初回のみに
- [x] サマリープロンプト: previousSummary を受けた差分更新(ローリングサマリー)
- [x] ask/suggest の応答言語を language パラメータ化(日本語ハードコード廃止)
- [x] ask にチャット履歴(直近ターン)を含める
- [x] 旧ワンショットAIフラグと死にコードの削除(2-8)

## Phase 4: Rust — プロセス管理(1-5, 2-7, 2-8, 4)

- [x] 子プロセス死活監視: exit時に `helper-exited {stream, sessionId, code}` をemit
- [x] AIサーバーの起動・再起動・リクエスト相関(id)・タイムアウト管理
- [x] `meeting_transcript` 蓄積と fallback コンテキストの全削除(UIのmessagesに一本化)
- [x] stop を graceful 化(stdinクローズ→待機→kill)。録音ファイル破損防止
- [x] エクスポートのファイル名を可読タイムスタンプに

## Phase 5: UI — 表示と AI パネル(2-3, 2-4, 2-6, 4)

- [x] 単一レーンイベント前提に interim/final 適用ロジックを簡素化、
      translation イベントの追記対応。transcriptSelection の競合判定を削除
- [x] helper-exited 受信時の自動再起動(バックオフ付き、最大3回)(1-5)
- [x] 自動サマリー失敗をSummaryセクション内に表示(2-3)。デバウンスは現行維持(2-2)
- [x] ローリングサマリー状態管理: previousSummary + 新規分のみ送信、文字数上限(2-4)
- [x] AIチャットを履歴付きスレッド表示に(2-6)
- [x] 不足ボタンの実装: Refresh Languages / Copy / Save / Clear(4)
- [x] デフォルト言語ペア: システム言語をMainに(2-5)

## Phase 6: 録音機能(新規)

要件:
- マイクとスピーカーを別々に録音(ライブ文字起こしと並行)
- 確認画面: 右=録音ファイル一覧、中央=選択した音声の波形、下=タイムスタンプ付き
  文字起こし(クリックで該当位置へシーク)
- mic のみ / speaker のみ / 統合版(ミックス)をダウンロード可能

実装:
- [x] Swift: `--record-file` で m4a(AAC 48k mono) 録音、stdin EOF/SIGTERMで確定
- [x] Swift: `--transcript-file` で確定イベントをJSONL保存(startMs付き)
- [x] Swift: `--waveform <file>` 波形ピークJSON出力 / `--mix a b --output out` ミックス
- [x] Rust: recordings ディレクトリ管理、list/read/waveform/export コマンド
- [x] UI: Live/Recordings タブ、波形canvas + audio再生、transcriptクリックでシーク
- [x] tauri.conf: asset protocol 有効化(録音再生用)

## Phase 7: 仕上げ

- [x] README更新(実在するUIと一致させる)
- [x] 全テスト実行(vitest / swift / cargo)

## 決定事項

- 録音フォーマット: m4a(AAC)。1時間で約15MB。graceful stopで確定処理する
- 波形生成はヘルパー側でピーク抽出(巨大ファイルをWebViewでデコードしない)
- サマリーの自動更新はデバウンス6sのまま(ユーザー判断)
- Rust側の会議コンテキスト二重管理は廃止し、UIの messages を唯一のソースにする
