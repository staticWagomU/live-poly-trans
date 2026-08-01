# LivePolyTrans UI再編成 実装計画

仕様の正: `mockups/ui-redesign.html`(2026-07-31 ユーザー合意済み)。
旧「安定化+UX改善」計画は完了分を除き本計画に吸収した(旧本文は git 履歴参照)。

## 合意済みの設計判断

1. **動作モデル**: 起動と同時に文字起こし開始(常時字幕)。「録音」ボタンは
   保存セッションの開始/終了であり、文字起こしは録音と独立して流れ続ける
2. **Live画面**: チャットバブル廃止 → 字幕行(メイン言語の大きな本文+サブ言語の訳文)。
   話者は色ドット付きラベル(自分=青/相手=グレー、diarization後はN話者色分け)
3. **ツールバー**: Speaker/Both/Mic セグメント・言語ピル(Main→Sub統合メニュー)・
   ✦(AIパネル開閉)・…メニュー(文字サイズ⌘+/-、コピー、保存)・録音ボタン(録音中はタイマー表示)
4. **フッター**: 文字起こしステータス+一時停止 | コピー/保存/クリア(2度押し確認)。
   「音声も保存」トグルは廃止(設定の「録音に音声ファイルを含める」へ)
5. **録音範囲の可視化**: 字幕ストリームに ⏺開始/⏹終了マーカー+録音中字幕に赤レール
6. **エンジン2段構え**: ライブ=Apple内蔵/Whisper。録音後の再処理=+WhisperX
   (transcription + alignment + diarization)
7. **Recordings**: 外部音声アップロード(スマホ/ボイスレコーダー)、再処理メニュー、
   トリム(kanary式・波形ハンドル)、書き出し、削除
8. **対面モード(mimi)**: 耳の不自由な方・ご高齢の方向けの全画面特大字幕モード。
   ポケトークmimi型。最新発話を特大表示+高コントラスト反転。
   **プッシュトゥトーク方式**: ボタン(またはスペースキー)を押している間だけ聞き取る。
   理由: 相手が画面の文字を音読する→それが再度文字起こしされる無限ループを防ぐため。
   **マイク音声のみを扱い、話者ラベルは表示しない**(スピーカー捕捉はモード中停止)。
   キーボード返答は不要(ユーザー確認済み)。このモードでは訳文非表示

## Phase 0: Tidy First(構造整理のみ、挙動変更なし)

- [x] 0-1. `+page.svelte`(2046行)の分割: `LiveView.svelte` を新設し、
      ツールバー/字幕スレッド/AIパネルを子コンポーネント化。既存テストが緑のまま
- [x] 0-2. デザイントークンを `src/lib/theme.css` に集約(モックの CSS 変数を移植)。
      ライト/ダーク両セット定義(`prefers-color-scheme`、旧計画4-7を吸収)
- [x] 0-3. キャプチャ状態(activeStreams/sessionIds/restart)を `src/lib/captureState.ts`
      に純関数として抽出+単体テスト(Phase 1 の土台)

## Phase 1: 動作モデル転換(最重要・リスク最大)

- [x] 1-1. 【Swift】helper に stdin 制御チャネル追加: `{"cmd":"start-recording","dir":…}` /
      `{"cmd":"stop-recording"}` の JSON 行を受けて AudioRecorder を動的に開始/停止。
      ストリーム再起動なしで録音を出し入れする(現状は起動引数 recordingDir 固定)。
      TDD: CommandLineOptionsTests に倣い制御行パーサを TestSupport でテスト
- [x] 1-2. 【Rust】`start_recording_session` / `stop_recording_session` コマンド新設:
      create_recording → 稼働中 helper 全プロセスへ制御行送付 → finalize_recording。
      helper が制御未対応(旧バイナリ)の場合はエラーで明示
- [x] 1-3. 【Web】文字起こしと録音の状態分離: `isTranscribing`(常時ON基調)と
      `recordingSession`(id/開始時刻/経過秒)を別管理。captureState.ts に純関数+テスト
- [x] 1-4. 【Web】起動時自動開始: onMount で権限確認 → start。失敗時(マイク/画面収録
      未許可)は平易な文言で案内(旧計画2-2を吸収)。設定「起動時に自動で開始」
      (localStorage、デフォルトON)がOFFなら一時停止状態で起動
- [x] 1-5. 【Web】一時停止/再開: stop_all_sessions を「pause」として再解釈し、
      フッターのステータス+ボタンに接続。録音セッション中の一時停止は録音も止まる旨を確認ダイアログ
- [x] 1-6. 【Web】録音マーカー: ChatMessage 列に marker アイテム(recording-start/stop、
      タイムスタンプ)を挿入。transcripts.ts に型追加+表示テスト。録音中フラグを
      メッセージに付与し赤レール描画
- [x] 1-7. 録音停止 → トースト表示+ Recordings リストへ反映(list_recordings 再取得)

## Phase 2: Live画面の刷新(モック準拠)

- [x] 2-1. 字幕行コンポーネント: transcriptDisplay.ts はそのまま流用し、表示のみ
      バブル→字幕行へ(メイン19px/サブ14.5px、interim はカーソル点滅+減光)
- [x] 2-2. ツールバー再構成: タブセグメント/音源セグメント/言語ピル/✦/…/録音ボタン。
      言語ピルは Main・Sub を1メニューに統合(「翻訳しない」= sub なし選択肢を新設)
- [x] 2-3. …メニュー実装: 文字サイズ(⌘+/⌘−/⌘0)、コピー(⇧⌘C)、保存(⌘S)。
      キーボードショートカット登録(旧計画4-8を吸収)
- [x] 2-4. AIパネルの折りたたみ化: ✦トグルで開閉(既定は閉)。Apple Intelligence
      非対応機ではパネル内に案内を出し自動要約を止める(旧計画2-5を吸収)
- [x] 2-5. フッター実装: ステータスランプ/一時停止/コピー/保存/クリア(2度押し確認は現行移植)
- [x] 2-6. 字幕リストのウィンドウ化: 表示上限+「以前を表示」(旧計画3-3を吸収。
      常時文字起こしになりメッセージ増加が加速するため本フェーズで必須化)
- [x] 2-7. 空状態2種(待機中/一時停止中)とジャンプボタンの移植

## Phase 3: 対面モード(mimi)

Phase 1-2 のライブ字幕パイプラインをそのまま使う表示モード。依存が薄いため
必要なら Phase 2 完了直後に前倒し可能。

- [x] 3-1. モード切替: Live の「…」メニュー「対面モードへ切り替え」→ 全画面オーバーレイ。
      ESC/終了で復帰。入場時に speaker ストリームを停止し **mic のみ**へ切り替え、
      退出時に元の音源構成を復元する
- [x] 3-2. 特大字幕表示: 最新発話 48px 基準(専用スケール 0.7〜2.2x、A−/A＋ボタンは
      44pt 以上のタッチターゲット)。直前2発話は縮小・減光して上に残す。
      interim は本文と同様に逐次更新。**話者ラベルは表示しない**(純粋なテキストのみ)
- [x] 3-3. 高コントラスト反転(白地黒字⇔黒地白字)。両モードでコントラスト比 7:1 以上
- [x] 3-4. プッシュトゥトーク: 大型ボタン(全幅・64pt)とスペースキーの押下中だけ
      音声認識を有効化。実装は Phase 1-5 の pause/resume 基盤を流用し、
      押下=resume/解放=pause をヘルパーに送る(音読ループ防止が目的なので、
      離した瞬間に interim を確定 or 破棄する挙動を実機で検証して決める)
- [x] 3-5. このモードでは訳文非表示(既定)。表示スケール等の設定は localStorage 永続化
- [x] 3-6. アクセシビリティ検証: 文字サイズ最大時のレイアウト崩れ、
      Reduce Motion 時のアニメーション抑制

## Phase 4: Settings 刷新(小)

- [x] 4-1. サイドバーナビ化(一般/認識モデル/言語)。既存の言語パック・モデル選択UIを
      「言語」「認識モデル」ペインへ移設、言語再検出↻もここへ
- [x] 4-2. 「一般」ペイン新設: 起動時自動開始トグル/録音に音声を含めるトグル/保存先表示
- [x] 4-3. 言語パックDL進捗の表示改善(旧計画4-5を吸収、最低限スピナー+完了反映)

## Phase 5: Recordings 刷新

- [x] 5-1. サイドバー+詳細の2ペイン化(日付グループ: 今日/昨日/過去30日/それ以前)。
      recordings.ts にグルーピング純関数+テスト
- [x] 5-2. アクションメニュー化: 書き出し(mic/speaker/mixed)+削除を集約(機能は現行流用)
- [x] 5-3. 外部音声アップロード: 「＋音声ファイルを読み込む」+ドラッグ&ドロップ。
      【Rust】`import_audio_file` コマンド(m4a/wav/mp3 を recordings ディレクトリへ
      コピー+meta 生成、source=external)。取り込み後は再処理導線へ誘導
- [x] 5-4. トリム: 波形に開始/終了ハンドル(選択範囲を残す)。
      【Rust】`trim_recording`: ffmpeg or AudioFileTools 系で `trimmed.m4a` を別生成
      (元ファイル温存)。トランスクリプトも範囲外を除去+タイムスタンプをシフト
      (純関数 `trimTranscript(items, startMs, endMs)` を TDD で先行)
- [x] 5-5. トリム/削除の確認: 適用時に確認ダイアログ(破壊的操作の HIG 準拠)

## Phase 6: 事後処理パイプライン(WhisperX)

- [x] 6-1. 【調査spike】WhisperX 実行形態の決定: uvx/pipx 検出 → subprocess 実行 →
      JSON 出力パース。diarization は pyannote(HF トークン必要)のため、
      トークン未設定時は「話者分離なしで実行」へフォールバックする仕様を先に固める
- [x] 6-2. 【Rust】`reprocess_recording(id, engine)` コマンド: engine=builtin/whisper/whisperx。
      (実装済みは whisperx。builtin/whisper の再処理は明示エラー+メニューでは未対応表示)
      進捗イベント(transcribe→align→diarize)を emit、結果は `transcript.whisperx.jsonl`
      として元と並存(非破壊)
- [x] 6-3. 【Web】再処理メニュー+3段階進捗バー(モック準拠)。完了後は話者ラベル付き
      表示(話者N の色割当は recordings.ts に純関数+テスト)
- [x] 6-4. 設定「認識モデル」ペインに WhisperX 要件の案内(インストール状況/HFトークン入力)

## Phase 7: WhisperX 実行環境のアプリ内取得(uv 自動ダウンロード)

前提: `brew install uv` をユーザーに強いていた導線を、アプリ内の「準備する」ボタンに
置き換える。uv は Python 非依存の単体静的バイナリで、tarball を展開して chmod するだけで
動く(実測: `uv-aarch64-apple-darwin.tar.gz` 17.4MB / 展開後 uv 40MB + uvx 336KB、
アプリ自身が書いたファイルには `com.apple.quarantine` が付かず実行可能)。

**設計判断**: 同梱(externalBin)ではなく初回利用時ダウンロードを選択。理由は
(a) WhisperX を使わない人に DMG +40MB を払わせない、(b) どのみち `uvx whisperx` の
初回で数 GB 落ちるので進捗 UI は必要、(c) 署名フローに新しいバイナリを持ち込まない。
uv のバージョンは**ピン留め**し SHA256 をコードに埋め込む(整合性検証をネットワークに
依存させない。更新は意図的なコード変更として行う)。
取得系は macOS 標準の `/usr/bin/curl` `/usr/bin/shasum` `/usr/bin/tar` に委譲し、
Cargo への HTTP/解凍/ハッシュ依存の追加を避ける。

- [x] 7-1. 【Rust】ピン留め定義と配置先の純関数: `uv_download_for(arch)`(URL/SHA256/
      展開ディレクトリ名)、`managed_uv_dir(app_data)` = `$APPDATA/tools/uv`。
      `locate_program` を `program_candidates(name, home, managed_dir)` に分解し、
      アプリ管理 uv を探索候補の先頭へ。既存の whisperx ランナー選択テストは緑のまま
- [x] 7-2. 【Rust】`install_uv_with(download, work_dir, dest_dir, run, report)`:
      curl→shasum 照合→tar 展開→uv/uvx 配置→chmod。`run` を関数注入にして
      発行コマンド列と checksum 不一致時の中断を単体テスト。
      `sha256_from_shasum_output` も純関数として切り出す
- [x] 7-3. 【Rust】`ensure_uv` コマンド: 既存なら即返し、無ければ 7-2 を実行して
      進捗を `uv-install-progress`(download/verify/extract/done)で emit。
      `whisperx_status` / `reprocess_recording` はアプリ管理 uv を見るようになる
- [x] 7-4. 【Web】設定「認識モデル」の WhisperX 欄を「準備する」ボタン+進捗表示に変更。
      失敗時のみ `brew install uv` の手動導線をフォールバック表示。
      段階ラベルは `src/lib/uvInstall.ts` に純関数+テスト

## Phase 8: プライバシー権限のオンボーディング(1つずつ確認・付与)

問題: 起動と同時に mic/speaker のヘルパーが同時起動し、マイクと画面収録のダイアログが
一斉に出る(`+page.svelte` onMount → `startTranscription`)。

**設計判断**: macOS の TCC は「一度リクエストするまでシステム設定の一覧に載らない」ため、
事前登録する API は存在しない。したがって「一覧に載っていてチェックするだけ」の状態は
**ユーザーの明示操作で1つずつリクエストする**ことで作る。起動時は
`AVAudioApplication.shared.recordPermission` と `CGPreflightScreenCaptureAccess()`
(どちらもダイアログを出さない状態取得 API)だけを使い、未許可なら自動開始せず案内に留める。

- [x] 8-1. 【Swift】`--check-permissions` / `--request-permission <microphone|screen-recording>`
      をコマンドに追加。状態取得は非プロンプト API のみ、リクエストは1種類ずつ。
      `permissionState(for:)` の写像と JSON ペイロードを TestSupport でテスト
- [x] 8-2. 【Rust】`permission_status` / `request_permission(kind)` / `open_privacy_settings(kind)`
      コマンド。`privacy_settings_url(kind)` は純関数+テスト
      (`x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone` など)
- [x] 8-3. 【Web】`src/lib/permissions.ts`: `requiredPermissions(mode)` /
      `missingPermissions(status, required)` / `permissionActionFor(state)` を純関数+テスト
- [x] 8-4. 【Web】設定に「🔐 プライバシー」ペインを新設。マイク/画面収録を1行ずつ
      状態タグ付きで並べ、未確認は「許可する」(=単発リクエスト)、拒否済みは
      「システム設定を開く」(該当ペインへ直接ジャンプ)を出す
- [x] 8-5. 【Web】起動時の一斉ダイアログ廃止: onMount で `permission_status` を先に読み、
      不足があれば自動開始をスキップして「権限が必要です → 設定を開く」バナーを表示。
      すべて許可済みのときだけ従来どおり自動開始する

## Phase 9: ライブ Whisper の恒久ストリーミング化(Windows前提)

**設計判断**: Whisper推論は Swift helper から切り離し、OS非依存の
`lpt-whisper-engine` sidecar に移す。macOS helper と将来の Windows capture helper は
どちらも 16kHz mono PCM を同じ JSONL protocol で engine に渡す。これにより、
`whisper-cli` をチャンクごとに起動する現行方式と、Swift内にWhisperを直結する
macOS限定実装を廃止対象にする。

- [ ] 9-1. 【Rust】engine JSONL protocol を固定:
      `config` / `audio` / `flush` / `shutdown` input と
      `status` / `transcript(isFinal=false/true)` / `metric` / `error` output。
      protocol は純関数+単体テストで先行し、Tauri/Swift/Windows実装から独立させる
- [x] 9-2. 【Rust】`lpt-whisper-engine` sidecar skeleton:
      stdin JSONL → protocol parse → stdout JSONL。まず `config` で `status:ready`、
      `shutdown` で終了する最小実装を作り、後続でC++ Whisper backendを差し替えられる
      境界にする
- [ ] 9-3. 【C++】whisper.cpp backend:
      モデルをプロセス起動後に1回だけロードし、PCM ring buffer を rolling window で推論。
      `stepMs` / `windowMs` / `finalizeSilenceMs` は計測ログで調整する
- [ ] 9-4. 【Engine】partial安定化:
      local agreement、committed prefix、重複抑制、backlog時のpartial drop/final優先を実装。
      長時間発話で28秒待ちに戻らないことを fixture で検証する
- [ ] 9-5. 【macOS helper】Whisper選択時は `whisper-cli` ではなく
      `lpt-whisper-engine` にPCMを流す。Swift helperは音声取得/録音/権限管理へ責務を寄せる
- [ ] 9-6. 【Tauri】sidecar bundle/build:
      macOS/Windows targetごとの `lpt-whisper-engine` binary を外部バイナリとして扱う。
      `--whisper-cli` と `whisper-cli` 探索は安定後に削除する
- [ ] 9-7. 【Windows準備】capture helper interface を固定:
      `start(stream)` / `audioFrame(stream, pcm16, sampleRate, timestamp)` /
      `stop(stream)`。Windows実装は WASAPI capture/loopback でこの境界に合わせる
- [ ] 9-8. 【計測ゲート】first partial P95 <= 2秒、final after silence P95 <= 1.5秒、
      30秒以上の連続発話で待ち時間が線形増加しないことを合格条件にする

## 検証ゲート(全フェーズ共通)

- `bun run test`(svelte-check → vitest)+ `scripts/test-swift-helper.sh` を各項目の完了条件とする
- Phase 1 完了時に実機で: 起動→自動字幕→録音開始/停止→Recordings 反映→⌘Q 中断安全性
- Phase 3 完了時に: 対面モードの文字サイズ最大・反転・実距離(1m)での可読性確認
- Phase 5-4 完了時に: トリム後の波形シーク⇔トランスクリプト同期ズレがないこと
- Phase 7 完了時に実機で: uv 未導入の状態から「準備する」→ Recordings の再処理が通ること
- Phase 8 完了時に実機で: TCC リセット(`tccutil reset Microphone <id>` 等)後に起動して
  ダイアログが自動で出ないこと、設定ペインから1つずつ許可できること

## 対象外(今回見送り)

- メニューバー常駐・字幕オーバーレイ小窓(ハイブリッド案の将来拡張として保留)
- ライブ字幕への WhisperX 適用(リアルタイム diarization は非対応)
- iCloud 同期、AI コンテキスト上限管理(別計画)
