# acorde

プラットフォーム非依存の Rust / WebAssembly 楽譜基盤ライブラリ（v1.2.3）です。

シリアライズ可能なスコアモデル、Undo/Redo可能な編集、範囲を明示した記譜入出力、論理レイアウト、
決定的SVG、再生イベント、分析、WASMバインディングを提供します。ライブラリ本体は同期・UI非依存で、
ファイルを読み書きしません。

```text
入力バイト列/文字列 → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
```

## クレート

| クレート | 役割 |
|---|---|
| `acorde-core` | スコアモデル、コマンド、検証、再生、理論ヘルパー |
| `acorde-io` | MusicXML/MXL、MIDI、任意機能のABC・MEI・MSCZ/MSCX |
| `acorde-layout` | ピクセル非依存の論理配置と印刷ページメタデータ |
| `acorde-render-svg` | 決定的なRust/WASM SVGレンダラー |
| `acorde-analysis` | 決定的で説明可能な和声・SATB分析 |
| `acorde-soundfont` | 任意機能のSF2/SF3/プロバイダ連携境界 |
| `acorde-wasm` | JavaScriptバインディング |
| `acorde-cli` | ファイル変換・検査CLI |
| `acorde` | core、I/O、layoutのアンブレラクレート |

SVGが必要な場合は `acorde-render-svg` を直接依存に追加します。アンブレラクレートは再エクスポートしません。

## 利用開始

```toml
[dependencies]
acorde = "1.2.3"
acorde-render-svg = "1.2.3"
```

標準ではMusicXMLとMIDIが有効です。ABC、MEI、MSCZ/MSCXは明示的に有効化します。

```toml
acorde = { version = "1.2.3", features = ["abc", "mei", "mscz"] }
```

```rust
use acorde_core::{Command, Score, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
let score: &Score = engine.score();
# let _ = score;
```

## 対応範囲と責務

MusicXML/MXLが最も広い交換経路です。MIDI、ABC、MEI、MSCZ/MSCXは文書化した部分集合であり、
完全ロスレス互換を主張しません。変換時の省略・正規化・意味差分は `ImportReport`、`ExportReport`、
`compatibility-report` で確認できます。

acordeはスコア意味論、論理ジオメトリ、決定的SVG、ブラウザ/WASM契約を担当します。ファイルUI、
音声合成、フォント解決・埋め込み、PDF、印刷ダイアログ、ブラウザE2Eはホスト側の責務です。SoundFont
プロバイダは再生イベントを利用できますが、合成と音源ライセンスはホストが管理します。

詳細は[記譜対応マトリクス](docs/notation-coverage.md)、保守的な機能一覧は
[scorecard](docs/scorecard.md)、外部ツールの限定観測は[相互運用証跡](docs/external-interoperability-evidence.md)
を参照してください。

## CLI

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde validate input.musicxml
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
acorde playback-report input.musicxml --bpm 120
```

このほか `info`、`report`、`preflight`、`analyze`、`benchmark`、`extract`、`transpose`、
`normalize`、タブ譜割当・再生検査、再生比較、変換レポートを提供します。完全なコマンド一覧は
`acorde --help` を参照してください。ファイルを扱うのはCLIであり、ライブラリAPIはメモリ上の入力を受け取ります。

## 文書

- [English README](README.md)
- [記譜対応と既知の情報欠落](docs/notation-coverage.md)
- [移行メモ](docs/migrations.md)
- [印刷レイアウト契約](docs/print-layout.md) と [ページSVG契約](docs/print-svg-contract.md)
- [ブラウザ契約](docs/browser-rendering.md) と [ブラウザ対応検査](docs/browser-support.md)
- [性能証跡](docs/performance.md)、[セキュリティ契約](docs/security/threat-model.md)、[貢献ガイド](CONTRIBUTING.md)

## 開発

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-features --locked --all-targets -- -D warnings
```

変更箇所に応じて、パッケージ、fuzz、WASM、ブラウザ、外部ツールの検査も実行してください。
フィクスチャの出所と機能境界もレビュー対象です。

## ライセンス

MIT または Apache-2.0（選択可）。
