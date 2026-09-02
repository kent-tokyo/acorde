# acorde

Rust と WebAssembly 向けのプラットフォーム非依存な楽譜ライブラリ（v1.0.4）です。

シリアライズ可能なスコアモデル、Undo/Redo 可能なコマンド、各種フォーマット入出力、
論理レイアウト、決定的な SVG レンダリング、再生イベント、WASM バインディングを提供します。
コアライブラリは同期処理・UI 非依存で、ファイルシステムへアクセスしません。

## クレート

| クレート | 役割 |
|---|---|
| `acorde-core` | スコアモデル、コマンド、検証、再生、音楽理論ヘルパー |
| `acorde-io` | MusicXML/MXL と MIDI。ABC、MuseScore MSCZ/MSCX は feature で追加 |
| `acorde-layout` | ピクセル非依存の行・スパン・ビーム・連符・臨時記号情報・印刷ページ配置 |
| `acorde-render-svg` | core/layout を使う Rust/WASM SVG レンダラー |
| `acorde-wasm` | JavaScript 向け I/O、編集、レイアウト、SVG バインディング |
| `acorde-cli` | ファイル変換・情報表示・検証 CLI |
| `acorde` | core、io、layout のアンブレラクレート |
| `acorde-soundfont` | オプションのSF2/SF3メタデータ検証・再生境界 |

`acorde` は `acorde-render-svg` を再エクスポートしません。SVG が必要な場合は直接依存します。
印刷向けの中立的なページ配置契約は [print-layout.md](docs/print-layout.md) にまとめています。
PDF変換、フォント解決、プリンタ接続、印刷プレビューUIはホスト側の責務です。

## 利用例

```toml
[dependencies]
acorde = "1.0.4"
acorde-render-svg = "1.0.4"
```

ABC と MuseScore 入力を有効にする場合：

```toml
acorde = { version = "1.0.4", features = ["abc", "mscz", "mei"] }
```

`acorde-io` の既定 feature は `musicxml` と `midi` です。`abc` は ABC の読み書き、
`mscz` は `.mscz`/`.mscx` の読み込み、`mei` は文書化されたMEIサブセットの入出力を追加します。パーサーはメモリ上の入力を受け取り、
ファイルは読みません。
MusicXMLのvoice番号1〜4は`Measure.voices`に保持され、MusicXMLの往復変換でも維持されます。
タブ譜の弦番号/フレットとMusicXMLの微分音（小数`alter`）を保持できます。ABCの`^/`/`_/`、
MEIの`qs`/`qf`による一般的な四分音臨時記号にも対応します。
glissandoとcross-staff配置もMusicXMLとの往復変換で保持されます。SoundFontはサンプルデコードを
内包せず、ライセンスを管理するアプリケーション側rendererとの境界として提供します。

## CLI

```bash
acorde convert input.mid output.musicxml
acorde info input.musicxml
acorde validate input.musicxml
acorde extract --part 0 input.musicxml part.musicxml
acorde transpose --semitones 2 input.musicxml transposed.musicxml
acorde normalize input.musicxml normalized.musicxml
acorde export-report input.musicxml exported.musicxml
```

## 開発

```bash
cargo test --all
cargo clippy --all -- -D warnings
```

ブラウザ向けの呼び出し順と検証方法は [browser-rendering.md](docs/browser-rendering.md)、
[browser-support.md](docs/browser-support.md) を参照してください。変更履歴は
[CHANGELOG.md](CHANGELOG.md) にあります。

## ライセンス

MIT または Apache-2.0 のデュアルライセンスです。
交換形式ごとの対応範囲と情報欠落の境界は、[記譜対応マトリクス](docs/notation-coverage.md)
にまとめています。
