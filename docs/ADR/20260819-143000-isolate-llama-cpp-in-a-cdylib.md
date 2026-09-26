# 翻訳用llama.cppはcdylibに隔離してdlopenで呼ぶ

| | |
|---|---|
| **Status** | accepted |
| **Date** | 2026-08-19 |
| **Decision-makers** | staticWagomU |
| **Consulted** | Claude Code |
| **Informed** | - |

## Context and Problem Statement

Step 0の検証で、whisper-rs（whisper.cpp）とllama-cpp-2（llama.cpp）を同一バイナリに静的リンクすると**実行時にSIGABRT**することが確認された（[docs/step0-results.md](../step0-results.md)）。両者はそれぞれ自前のggmlをvendorしており、同名のCシンボルがリンク時に1つへ統合される。結果としてwhisperがllama側の（バージョンの異なる）ggmlを呼び、構造体レイアウトの食い違いでクラッシュする。

ASRと翻訳の両方をローカル推論で行う以上（ADR-153801 / ADR-153804）、この衝突は避けて通れない。Step 0では選択肢を洗い出すに留め、決定はStep 2着手時とした。

## Decision Drivers

* 【MUST】単一プロセス構成を維持する（ADR-153800）。IPCの遅延・障害モード・配布物の複雑さを増やしたくない
* 【MUST】ASRと翻訳が同時に動く（同時稼働はStep 0で計測済み: 最悪4s/文で要件内）
* 上流の更新に追随できること。ggmlはwhisper.cpp / llama.cppそれぞれの都合で動く
* 片方のクラッシュがもう片方を巻き込まないこと

## Considered Options

1. **翻訳側をcdylibに切り出しdlopenする**
2. テキストサイドカー（別プロセス＋stdio）
3. ggmlのバージョンをピン留めして共有する
4. どちらかをMLX等の別バックエンドに置き換える

## Decision Outcome

**Chosen option: 1（cdylib分離）**。`crates/kkm-translate-ggml`をcdylibとしてビルドし、ホスト側`crates/kkm-translate`が`dlopen`＋C ABIで呼ぶ。

dylibは自分専用のggmlのコピーを持ち、macOSのtwo-level namespace（Windowsは DLLごとのシンボル解決）によってホスト側のggmlとは別物として解決される。プロセスは1つのままなので、ADR-153800を破らずに衝突だけを解消できる。

C ABIは4関数に絞る: `kkm_translate_init` / `kkm_translate` / `kkm_translate_free` / `kkm_translate_shutdown`。

### Consequences

**Positive:**
* whisper.cppとllama.cppを互いのバージョン都合から切り離して更新できる
* 単一プロセスのまま。IPC・プロセス監視・配布物の追加が要らない
* 翻訳バックエンドの差し替え境界がC ABIとして明示される

**Negative:**
* ビルドが1コマンドで済まない。cdylibを先に作ってから`.app`に同梱する必要がある（`scripts/build-app.sh`、`tauri.conf.json`の`macOS.frameworks`）
* dylibの探索パスが実行形態で変わる（`.app/Contents/Frameworks` / `target/<profile>`）。`kkm_translate::dylib_candidates`が引き受け、単体テストで固定している
* FFI境界の安全性を自前で持つ。`extern "C"`からのunwindはプロセスごと落とすため、cdylibの全エクスポートが`catch_unwind`で受ける
* 型安全がC ABIの幅（ポインタと文字列）まで落ちる

**Neutral:**
* 翻訳ハンドルはスレッドセーフではない。呼び出しは翻訳ワーカー1本に直列化する（元よりキュー＋逐次処理が要件）

### Confirmation

* `cargo run -p spike --bin cdylib-check`（`--no-default-features --features asr`）: ASRデコードと翻訳の並走でSIGABRTが出ないこと
* `cargo test -p kkm-translate -- --ignored`: 実モデルを読んで実際に訳せること
* アプリを`.app`として起動し、マイク／スピーカー両レーンで訳文が出ること

## Pros and Cons of the Options

### cdylib分離

* Good: 単一プロセスのまま衝突を解消でき、両者を独立して更新できる
* Bad: ビルド手順とdylib探索、FFI境界の面倒を引き受ける

### テキストサイドカー（別プロセス）

* Good: 隔離が最も強く、翻訳側のクラッシュがアプリに波及しない
* Bad: ADR-153800（単一プロセス）に反する。プロセス監視・起動順・配布物が増える
* Bad: 将来Ollama等のHTTPバックエンドを足すなら、そちらで同じ効果が得られる

### ggmlのバージョンピン留め

* Good: 追加の仕組みが要らず、ビルドが最も単純
* Bad: whisper-rsとllama-cpp-2が同じggmlに揃う保証がなく、上流更新のたびに壊れうる
* Bad: 片方をアップデートできない状態に自らを縛る

### 片方を別バックエンドに置換（MLX等）

* Good: 衝突そのものが消える
* Bad: Step 0のMLX比較では性能はほぼ互角で、乗り換える理由が衝突回避しかない
* Bad: mlx-rs / mlx-c FFIの本実装コストと、macOS専用化（Windows対応がMUST）

## More Information

前提: [単一プロセス構成](20260810-153800-rebuild-v2-as-single-process-rust-core.md) / [組み込みローカルLLM翻訳](20260810-153804-embedded-local-llm-translation.md)。実測は[docs/step0-results.md](../step0-results.md)。実装は`crates/kkm-translate-ggml`（cdylib）と`crates/kkm-translate`（ホスト側ローダ）。
