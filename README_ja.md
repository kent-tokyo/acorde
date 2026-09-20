# acorde

Rust と WebAssembly 向けのプラットフォーム非依存な楽譜ライブラリ（v1.2.0）です。

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
印刷向けの中立的なページ配置契約は [print-layout.md](docs/print-layout.md)、ページ単位の
SVG要件は [print-svg-contract.md](docs/print-svg-contract.md) にまとめています。
PDF変換、フォント解決、プリンタ接続、印刷プレビューUIはホスト側の責務です。
SoundFontの`SoundFontPresetZone` APIでは bank/program と key/velocity からサンプル領域を
選択でき、Composer側でSoundFontのgenerator解析を重複実装する必要がありません。
materialized SoundFont snapshot には source PCM の channel layout と decode channel count も含まれます。
SF3では snapshot の decode API を使うことで sample ID に対応する Ogg stream を選択できます。SF2の
linked stereo は左右のmono source regionの明示的な組として扱います。

## 利用例

```toml
[dependencies]
acorde = "1.2.0"
acorde-render-svg = "1.2.0"
```

ABC と MuseScore 入力を有効にする場合：

```toml
acorde = { version = "1.2.0", features = ["abc", "mscz", "mei"] }
```

`acorde-io` の既定 feature は `musicxml` と `midi` です。`abc` は ABC の読み書き、
`mscz` は `.mscz`/`.mscx` の読み込みと決定的なcanonical subset出力、`mei` は文書化されたMEIサブセットの入出力を追加します。パーサーはメモリ上の入力を受け取り、
ファイルは読みません。
MSCXのキー署名、拍子、テンポ、音高、TPCなどの数値が不正で安全なcanonical値へ
フォールバックされた場合、入力位置付き診断として報告します。ピッチの科学的表記は
拡張された臨時記号の連続を保持し、範囲をオーバーフローする入力はエラーにします。
MusicXMLのvoice番号は4つの`Measure.voices`編集スロットと独立して保持され、1と5のように
欠番を含む番号も往復変換で変えません。
標準の`backup`/`forward`カーソル移動は明示的な休符へ変換し、カーソルの巻き戻し不足や小節長超過は
黙って並べ替えずエラーにします。
タブ譜の弦番号/フレットとMusicXMLの微分音（小数`alter`）を保持できます。ABCの`^/`/`_/`、
MEIの`qs`/`qf`による一般的な四分音臨時記号にも対応します。
タイは休符を終点として描画せず、SVGメタデータには編集・再生同期用の型付き
`tie_start`/`tie_end`が含まれます。
glissandoとcross-staff配置もMusicXMLとの往復変換で保持されます。SoundFontはサンプルデコードを
内包せず、ライセンスを管理するアプリケーション側rendererとの境界として提供します。
形式変換後の比較には `acorde compatibility-report source candidate` を使えます。位置ベースの
意味差分と両ファイルの import 診断を出力しますが、ロスレス互換性は主張しません。
CIで意味差分を失敗扱いにする場合は `--fail-on-differences` を追加します。
型付きの情報損失診断も失敗扱いにする場合は `--fail-on-loss` を使います。
レポートには判定結果を示す `semantic_equivalent` と `lossless` も含まれます。`lossless` は
scoreが同値で、型付き変換損失がない場合にtrueです。
`analysis_changed_categories` には決定的分析で変化したカテゴリも含まれます。
差分 gate は score と分析の両方を対象とし、分析側の判定は `analysis_equivalent` で確認できます。
WASMの `compatibility_report` とブラウザアダプターの `compatibilityReport()` でも、正規化済み
score JSON同士の同じ判定を利用できます。形式固有の損失は各 `*_report` APIで確認します。
`SampleDecoder` と `SampleRenderer` により、codecと音声出力をホスト側へ委譲する型付き接続点も提供します。
入出力APIは型付きの`ImportReport`/`ExportReport`で変換診断を返します。WASMでも対応する
importと、MusicXML、MEI、MIDI、ABC、MSCX、MSCZのexportについてレポート付きAPIを利用できます。

## CLI

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --measures-per-system 3 --systems-per-page 4 --title-page --fail-on-issues \
  --final-page-policy balance --scale 1.05
acorde print-report input.musicxml --preset letter-part --part 0 --measures-per-system 3 \
  --running-title "Suite" --page-number-in-footer
acorde info input.musicxml
acorde validate input.musicxml
acorde validate guitar.musicxml       # タブ譜の線数・調弦・弦番号も検証
acorde extract --part 0 input.musicxml part.musicxml
acorde transpose --semitones 2 input.musicxml transposed.musicxml
acorde normalize input.musicxml normalized.musicxml
acorde export-report input.musicxml exported.musicxml
acorde tab-position guitar.musicxml edited.musicxml --part 0 --measure 0 --note 1 --string 2 --fret 3
acorde auto-tab guitar.musicxml guitar-tabbed.musicxml
acorde auto-tab-report guitar.musicxml guitar-tabbed.musicxml
acorde tab-performance-report guitar-tabbed.musicxml --bpm 120 --fail-on-diagnostics
acorde playback-report input.musicxml --bpm 120 --loop-start 0 --loop-end 3
acorde playback-compare expected.json actual.json --fail-on-mismatch
```

`validate` はタブ譜の線数、調弦値、明示された弦番号もローカルで検証します。SoundFontや
ネットワーク接続は必要ありません。
`render` は同じ入力形式を決定的なSVGへ変換します。`--width`、`--staff-size`、
`--measures-per-system` で出力ジオメトリを指定でき、アドレス用フックは既定で有効です。
`render-report` はレポート自身の `render_report_schema_version`、入力診断、renderer の preflight 診断、レンダリング結果、およびローカルの
決定的な `fnv1a64-*` 形式の `svg_fingerprint` を JSON で出力します。失敗時は
`rendered: false` と `render_error` を記録し、不完全な SVG は出力しません。フィンガープリントは
ローカルでのバイト列再現性を示す証拠識別子であり、公開用の暗号学的ハッシュではありません。
`--fail-on-issues` で問題を CI の失敗として扱えます。
`print-report` は入力形式・スキーマ・import診断・renderer preflight診断と、`layout` 内の物理ページ・システム、改ページ理由、
出版メタデータ、小節テキスト注釈、資源・スパン診断を含むバージョン付きJSONレポートを出力します。
PDF生成やOS印刷は
行いません。`--preset a4-score`/`letter-score` は全体譜、`--preset a4-part`/`letter-part`
は `--part` と組み合わせたパート譜です。その他のレイアウトオプションは、選択したプリセット
の決定論的な初期値を上書きします。
`--running-title`、`--header-text`、`--footer-text`、`--page-number-in-footer`、
`--no-part-names` で出版メタデータも指定できます。フォント選択やScoreの意味論は変更しません。
`--fail-on-issues` を指定するとimportまたはrenderer診断をJSONに残したまま終了コード1にできます。
`--scale`、`--first-system-measures`、`--final-page-policy`、`--notation-break-policy`、
`--pickup-policy` で既存の決定論的なシステム分割ポリシーも指定できます。
`tab-position --clear` で明示位置を解除できます。各インデックスは0始まりで、`--string`だけ
1始まりです。
`auto-tab` は未指定の単音・コードに対し、フレット負荷と前後のポジション移動を抑える
運指を自動選択します。
`auto-tab-report` は割り当て数、未割り当て数、コード数、フレット負荷を決定論的なJSONで
表示し、最適化済みスコアも出力します。
`tab-performance-report` は明示された弦・フレットを再生イベントへ対応付け、調弦・カポ・
ピッチ不一致を型付きJSON診断として出力します。位置を推測して補完することはありません。
`--fail-on-diagnostics` を指定すると、診断がある場合に終了コード1になります。
`playback-report` は、ブラウザやComposer側のスケジュール結果と比較できる決定論的な期待再生
イベント列を出力します。`--loop-start` と `--loop-end` で物理小節の範囲も指定できます。
JSON出力前にcoreの比較イベント上限も適用します。
WASMの期待イベント生成にも同じ上限を適用します。
`playback-compare` は期待列とhost側の実測JSONを明示的な時間許容差で比較し、
`--fail-on-mismatch` でCIゲートとして利用できます。入力JSONは64 MiBに制限されます。

## 開発

```bash
cargo test --all-features --locked
cargo clippy --all --all-features --locked -- -D warnings
```

ブラウザ向けの呼び出し順と検証方法は [browser-rendering.md](docs/browser-rendering.md)、
[browser-support.md](docs/browser-support.md) を参照してください。変更履歴は
[CHANGELOG.md](CHANGELOG.md) にあります。
過去バージョンからの移行概要は [migrations.md](docs/migrations.md) にまとめています。

## ライセンス

MIT または Apache-2.0 のデュアルライセンスです。
交換形式ごとの対応範囲と情報欠落の境界は、[記譜対応マトリクス](docs/notation-coverage.md)
にまとめています。
