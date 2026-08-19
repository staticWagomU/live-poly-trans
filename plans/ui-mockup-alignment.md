# UIを mockups/desktop-prototype.html に寄せる

## なぜ

いまの画面はStep 1で置いた最小UI（ダーク・2カラム・`<details>`のセレクト）のままで、
モックアップで決めた「録音アプリとしての姿」から離れている。実装が進んで
録音ファイル・タイムスタンプ・翻訳レーンが揃った今、画面をその姿に合わせる。

## 決めたこと

- 文字起こしは**1本のストリーム＋話者ピル**にする。`startMs`で両レーンをマージする。
  レーン間同期は実測±15ms（plan.md Step 4）なので、時刻順に並べれば会話として読める。
  → plan.md Step 3の「UIはレーン別2カラム」を更新する。
- ホーム（録音一覧）とセッションの**2ビュー構成**を作る。一覧の中身は実在の録音を出す。
- モックアップにあってもバックエンドに無いものは**作らない**: Ask、波形／再生、
  書き出しメニュー、行削除、一時停止、レーン×言語マトリクス。
  出すなら動くこと。動かない飾りは置かない。

## やること

### 1. バックエンド: 録音の一覧（`src-tauri/src/library.rs`）

`list_recordings` コマンド。`record_base()` 配下のセッションディレクトリを列挙する。

- `name` — ディレクトリ名（`20260819090503` / 衝突時は `-2`）
- `startedAtMs` — 名前の壁時計をローカル時刻として解釈したepoch ms。名前が
  規則に合わなければ `null`（手で置いたフォルダを弾かず、並べ替えだけ諦める）
- `durationMs` — `mix.wav` のヘッダから。無ければ最初のレーンから
- `lanes` — 実在する `mic.wav` / `speaker.wav`
- `snippet` / `utterances` — `transcript.jsonl` の最初の発話と件数。一覧で
  セッションを見分ける手がかりになる
- 新しい順。wavが1つも無いディレクトリは録音ではないので除く

### 2. フロント: `src/routes/+page.svelte` の作り直し

- ライトテーマ。モックアップのCSS変数（`--accent` `--record` `--separator` …）をそのまま使う
- ツールバー: 戻る / ウィンドウタイトル / 「録音を開始」 / 録音中は経過時間＋停止
  - 経過時間は `status.recordingDir` の名前から引く（リロードしてもずれない）
  - 一時停止はバックエンドに無いので置かない
- ホーム: 検索、「録音中」セクション、日付グループ（今日 / 昨日 / 日付）
- セッション: `state-badge`、`capture-badge`＋レベルメーター、言語ピル＋ポップオーバー、
  1本の文字起こし（timestamp / speaker / utterance / translation）、コピー、トースト
- 言語ポップオーバーはモックアップの見た目（`.pop-section` / `.slot` / 検索付きピッカー）で、
  中身は `spoken` / `target` / `mutual`

## 確認

- `cargo test -p kikimimic`（library.rsのテスト）
- `bun run check`
- `./scripts/build-app.sh --open` で実発話
