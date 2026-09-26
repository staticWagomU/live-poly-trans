# Step 0: CoreAudio Process Tap の検証結果

2026-08-19, M4 Pro / macOS 26.5。スパイクは `crates/spike/src/bin/tap_check.rs`、
再現は `scripts/tap-check.sh`。

## 結論

**RustからProcess Tapでシステム音声を取得できる。Swift薄ヘルパーへのフォールバックは不要。**
ADR-153803の「Process TapのRust FFIは自前検証が必要（失敗時はSwift薄ヘルパー）」という
リスクは解消した。

```
callbacks: 495, samples: 506880, peak: 0.5169, rms: 0.08169
PASS: captured non-silent system audio from Rust (peak 0.5169)
```

取得フォーマットは 48kHz / 2ch / float32。既存のマイクレーンと同じく
`StreamResampler` で16kHz monoに落とせるため、レーン以降のパイプラインは共通のまま。

## 使ったAPIと構成

`objc2-core-audio` 0.3.2（features: `AudioHardware`, `objc2`）で全て賄える。

1. `CATapDescription::initStereoGlobalTapButExcludeProcesses(&[])` — 全プロセス出力のステレオmix
2. `AudioHardwareCreateProcessTap` → tapのAudioObjectID
3. `kAudioTapPropertyFormat` でtapのASBDを取得
4. `AudioHardwareCreateAggregateDevice` — privateなaggregateにtapを載せる
5. `AudioDeviceCreateIOProcIDWithBlock`（+ 明示的なDispatchQueue）→ `AudioDeviceStart`

aggregateの辞書は以下。デフォルト出力デバイスを **main sub-device と sub-device list の
両方** に入れる（クロック源）。

| キー | 値 |
|---|---|
| `kAudioAggregateDeviceMainSubDeviceKey` | デフォルト出力デバイスのUID |
| `kAudioAggregateDeviceSubDeviceListKey` | `[{ kAudioSubDeviceUIDKey: 同UID }]` |
| `kAudioAggregateDeviceTapListKey` | `[{ kAudioSubTapUIDKey: tapのUUID, kAudioSubTapDriftCompensationKey: true }]` |
| `kAudioAggregateDeviceTapAutoStartKey` | `true` |
| `kAudioAggregateDeviceIsPrivateKey` | `true` |

## 失敗が全て「無言」である点に注意

このAPIは条件を満たさなくてもエラーを返さない。全ての呼び出しが `noErr` を返し、
IOProcも正常な周期で発火し、バッファ形状（512フレーム×2ch×f32）も正しいまま、
**中身だけが全てゼロ**になる。デバッグ時にこれで2ラウンド消耗した。

### 罠1: 署名済み.appを`open`で起動しないと権限が下りない

CLIバイナリを直接実行すると、TCCの「responsible process」が親のターミナルになる。
ターミナルに「システム音声の録音」権限がないと、**ダイアログも出ず、エラーも返らず、
ゼロ埋めバッファが返る**。カメラ権限がないときに黒フレームが返るのと同じ設計。

`Info.plist`（`NSAudioCaptureUsageDescription` 必須）を持つ.appバンドルにして
`open` で起動すると、アプリ自身がresponsible processになり権限が適用される。
`scripts/tap-check.sh` がこのバンドル化・署名・起動を全部やる。

### 罠2: 何か再生されていないとIOサイクルが回らない

出力デバイスがアイドルだとHALはIOを止めており、tapを載せてもコールバックが1回も
来ない。正常なtapが壊れているように見える。スクリプトが `afplay` で音を鳴らしながら
実行するのはこのため。

### 切り分けに使った対照実験

Rust側のFFIを疑って時間を溶かさないよう、**同じフローをSwiftで書いて比較した**
（`AudioToolbox`/`CoreAudio`を直接叩く80行程度）。Swift版も1バイト違わず同じ挙動
（コールバックは来る／中身は全部ゼロ）だったため、バインディングとFFIの疑いが消え、
探索範囲が権限側に絞れた。またHALに `kAudioProcessPropertyIsRunningOutput` を
問い合わせて「OSは再生中のプロセスを認識している」ことも確認し、「音が鳴っていない」
説を否定した。

## Step 3（スピーカーレーン）への申し送り

- **権限UXが必須**: 本番アプリでも `Info.plist` に `NSAudioCaptureUsageDescription` が要る。
  権限がない状態は例外ではなく「無音」として現れるので、`peak == 0` が一定時間続くことを
  異常として検知し、システム設定へ誘導する必要がある
- **配布形態の制約**: Tauriは.appバンドルを吐くのでこの要件は自然に満たせるが、
  `cargo run` での開発中は権限が下りない点に注意
- デフォルト出力デバイスの変更（ヘッドホン抜き差し等）への追従は未検証。
  aggregateのsub-deviceが古いデバイスを指し続ける可能性があり、Step 3で要確認
