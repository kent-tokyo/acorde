# acorde

[![CI](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/kent-tokyo/acorde/graph/badge.svg?branch=main)](https://app.codecov.io/gh/kent-tokyo/acorde)
[![docs.rs](https://docs.rs/acorde/badge.svg)](https://docs.rs/acorde)

プラットフォーム非依存の Rust / WebAssembly 楽譜基盤ライブラリです。

> **リリース注意:** v1.2.19 と v1.2.20 は `SvgRenderOptions` の公開フィールド追加により、
> 完全な構造体リテラルを使う Rust 利用側をコンパイル不能にする場合があります。レンダラー設定APIを
> 導入する前に、互換性を回復した v1.3.0 へ更新してください。詳細は[移行メモ](docs/migrations.md)です。

シリアライズ可能なスコアモデル、Undo/Redo可能な編集、範囲を明示した記譜入出力、論理レイアウト、
決定的SVG、再生イベント、分析、WASMバインディングを提供します。
ライブラリ本体は同期・UI非依存で、ファイルを読み書きしません。

ワークスペースの最小サポート Rust バージョン（MSRV）は **Rust 1.88** です。

```text
入力バイト列/文字列 → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
                                                               └────────→ acorde-pdf → PDF
```

## クレート

| クレート | 役割 |
|---|---|
| `acorde-core` | スコアモデル、コマンド、検証、再生、理論ヘルパー |
| `acorde-io` | MusicXML/MXL、MIDI、任意機能のABC・MEI・MSCZ/MSCX・Guitar Pro 3〜8入力 |
| `acorde-layout` | ピクセル非依存の論理配置と印刷ページメタデータ |
| `acorde-render-svg` | 決定的なRust/WASM SVGレンダラー |
| `acorde-pdf` | Harumiによる任意のベクターPDF出力（Rust 1.88以上） |
| `acorde-omr` | 任意の、レビュー必須のPNG認識提案アダプター |
| `acorde-analysis` | 決定的で説明可能な和声・SATB分析 |
| `acorde-soundfont` | 任意機能のSF2/SF3/プロバイダ連携とSF2 WAV生成 |
| `acorde-wasm` | JavaScriptバインディング |
| `acorde-cli` | ファイル変換・検査CLI |
| `acorde` | core、I/O、layoutのアンブレラクレート |

SVGが必要な場合は `acorde-render-svg` を直接依存に追加します。アンブレラクレートは再エクスポートしません。

## 利用開始

```toml
[dependencies]
acorde = "=1.3.6"
acorde-render-svg = "=1.3.6"
```

標準ではMusicXMLとMIDIが有効です。ABC、MEI、MSCZ/MSCX、Guitar Pro 3〜8入力は明示的に有効化します。

```toml
acorde = { version = "=1.3.6", features = ["abc", "mei", "mscz", "gp"] }
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

MusicXML/MXLが最も広い交換経路です。MIDI、ABC、MEI、MSCZ/MSCX、Guitar Pro 3〜8（`.gp3`/`.gp4`/`.gp5`/`.gpx`/`.gp`、入力のみ）は文書化した部分集合であり、
完全ロスレス互換を主張しません。変換時の省略・正規化・意味差分は `ImportReport`、`ExportReport`、
`compatibility-report` で確認できます。

acordeはスコア意味論、論理ジオメトリ、決定的SVG、ブラウザ/WASM契約を担当します。`acorde-pdf` は
Harumiを使う任意のメモリ内アダプターであり、フォント、UI、印刷、ブラウザE2Eはホスト側の責務です。
任意のSF2 WAV生成機能はホストから渡された音源を使います。音源ライセンスとデバイス再生は
ホストが管理します。

対応範囲は[記譜対応マトリクス](docs/notation-coverage.md)、現行機能は
[scorecard](docs/scorecard.md)、外部ツールの限定観測は
[相互運用証跡](docs/external-interoperability-evidence.md)を参照してください。

## CLI

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde validate input.musicxml
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
acorde playback-report input.musicxml --bpm 120
acorde audio-render input.musicxml licensed.sf2 output.wav --full-sf2
acorde omr printed-score.png draft.mscz --report draft.omr.json
acorde omr-evaluate printed-score.png reference.mscz --report evaluation.json
acorde omr-apply-report provider-result.json draft.mscz --report application.json
```

このほか検査、分析、変換レポート、タブ譜、再生関連のコマンドを提供します。完全な一覧は
`acorde --help` を参照してください。ファイルを扱うのはCLIです。`omr` は高コントラストPNGを対象にした
レビュー必須の限定機能で、診断付き結果からはMSCZを書き出しません。範囲と評価方法は
[OMR提案契約](docs/omr-proposal-contract.md)と[検証境界](docs/omr-validation.md)を参照してください。

## 文書

[文書案内](docs/README.md)から用途別に参照してください。主な入口は、[記譜対応と既知の情報欠落](docs/notation-coverage.md)、
[移行メモ](docs/migrations.md)、[ブラウザ契約](docs/browser-rendering.md)、[PDF出力契約](docs/pdf-export.md)です。

[English README](README.md)、[セキュリティ契約](docs/security/threat-model.md)、
[貢献ガイド](CONTRIBUTING.md)は独立した入口として維持します。

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
