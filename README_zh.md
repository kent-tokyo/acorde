# acorde

[![CI](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/kent-tokyo/acorde/graph/badge.svg?branch=main)](https://app.codecov.io/gh/kent-tokyo/acorde)
[![docs.rs](https://docs.rs/acorde/badge.svg)](https://docs.rs/acorde)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](README.md#license)
[![crates.io](https://img.shields.io/crates/v/acorde.svg)](https://crates.io/crates/acorde)
[![Ask DeepWiki](https://img.shields.io/badge/Ask-DeepWiki-blue.svg)](https://deepwiki.com/kent-tokyo/acorde)

[English](README.md) · [日本語](README_ja.md) · 简体中文

面向 Rust 和 WebAssembly 的跨平台乐谱基础库。

> **版本提示：**v1.2.19 和 v1.2.20 为公开的 `SvgRenderOptions` 增加了字段，
> 使用完整结构体字面量的 Rust 代码可能因此无法编译。使用渲染器配置 API 前，
> 请升级至恢复兼容性的 v1.3.0 或更新版本。详情见[迁移说明](docs/migrations.md)。

acorde 提供可序列化的乐谱模型、支持撤销和重做的编辑命令、明确限定支持范围的记谱格式读写、
逻辑排版、确定性的 SVG 输出、播放事件生成、分析基础功能和 WASM 绑定。
各库 crate 均为同步接口，不依赖 UI，也不直接读写文件。

工作区的最低支持 Rust 版本（MSRV）为 **Rust 1.89**。

```text
输入字节/文本 → acorde-io → Score → acorde-layout → LayoutResult
                                         └────────→ acorde-render-svg → SVG
                                                                  └──→ acorde-pdf → PDF
```

## Crate 一览

| Crate | 用途 |
|---|---|
| `acorde-core` | 乐谱模型、编辑命令、校验、播放和乐理辅助功能 |
| `acorde-io` | MusicXML/MXL 和 MIDI；可选启用 ABC、MEI、MSCZ/MSCX、Guitar Pro 3–8、Humdrum `**kern` 和 Plaine & Easie 导入 |
| `acorde-layout` | 与像素无关的逻辑排版和打印分页元数据 |
| `acorde-render-svg` | 输出确定性 SVG 的 Rust/WASM 渲染器 |
| `acorde-pdf` | 通过 Harumi 导出可选的矢量 PDF（要求 Rust 1.89 或更新版本） |
| `acorde-omr` | 可选的 PNG 图像识谱模块，输出供人工审核的有限范围提案 |
| `acorde-analysis` | 结果可解释、可复现的和声与四声部（SATB）分析 |
| `acorde-soundfont` | 可选的 SF2/SF3/音源提供方集成，以及使用 SF2 渲染 WAV |
| `acorde-wasm` | JavaScript 绑定 |
| `acorde-cli` | 基于文件的转换、检查工具和本地 MCP 服务器 |
| `acorde` | 统一重新导出 core、I/O 和 layout 的入口 crate |

`acorde` 不重新导出 `acorde-render-svg`；需要 SVG 渲染时，请单独添加该依赖。

## 快速开始

```toml
[dependencies]
acorde = "=1.3.13"
acorde-render-svg = "=1.3.13"
```

默认启用的输入输出格式为 MusicXML 和 MIDI。其他格式需要显式启用对应功能：

```toml
acorde = { version = "=1.3.13", features = ["abc", "mei", "mscz", "gp"] }
```

```rust
use acorde::core::{Command, ScoreEngine, SetTempoCmd};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = ScoreEngine::new();
    engine.apply(Command::SetTempo(SetTempoCmd { bpm: 144 }))?;
    engine.undo()?;
    println!("{} BPM", engine.score.settings.tempo_bpm);
    Ok(())
}
```

## 格式支持与宿主职责

MusicXML/MXL 是支持范围最广的乐谱交换格式。MIDI、ABC、MEI、MSCZ/MSCX、
Guitar Pro 3–8（`.gp3`/`.gp4`/`.gp5`/`.gpx`/`.gp`，仅导入）、
Humdrum `**kern`（`.krn`，仅导入）和 Plaine & Easie（`.pae`，仅导入）
只支持文档列明的子集，不保证无损往返。需要检查转换中的遗漏、规范化或语义差异时，
请使用 `ImportReport`、`ExportReport` 或 `compatibility-report`。

acorde 负责乐谱语义、逻辑几何、确定性的 SVG 输出，以及带版本约束的浏览器/WASM 接口。
`acorde-pdf` 是可选的内存中 Harumi 适配层；字体、UI、打印和浏览器端到端呈现由宿主程序负责。
可选的 SF2 WAV 渲染使用调用方提供的音源；音源许可与设备播放也由宿主程序管理。

已验证的支持范围见[记谱覆盖矩阵](docs/notation-coverage.md)，现有功能见
[功能清单](docs/scorecard.md)，外部工具的有限互操作观察见
[互操作证据](docs/external-interoperability-evidence.md)。

## 命令行工具

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde validate input.musicxml
acorde mcp --root ./scores
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
acorde playback-report input.musicxml --bpm 120
acorde audio-render input.musicxml licensed.sf2 output.wav --full-sf2
acorde omr printed-score.png draft.mscz --report draft.omr.json
acorde omr-evaluate printed-score.png reference.mscz --report evaluation.json
acorde omr-apply-report provider-result.json draft.mscz --report application.json
```

还提供检查、分析、转换报告、六线谱及播放相关命令；完整列表请运行 `acorde --help`。
文件访问由 CLI 负责。`omr` 只面向高对比度 PNG 的有限场景，结果须经人工审核；
具体限制和禁止静默写入的规则见[OMR 提案约定](docs/omr-proposal-contract.md)与
[验证边界](docs/omr-validation.md)。

## 文档

从[文档导航](docs/README.md)了解当前接口约定，从[路线图](ROADMAP.md)了解规划。
常用参考资料包括[记谱覆盖矩阵](docs/notation-coverage.md)、
[迁移说明](docs/migrations.md)、[浏览器接口约定](docs/browser-rendering.md)和
[PDF 导出边界](docs/pdf-export.md)。比较乐曲分析能力时，请先阅读
[带来源定位的评估约定](docs/analysis-evaluation.md)及对应任务的证据，
再判断与 music21 的差距。

[英文 README](README.md)、[日文 README](README_ja.md)、
[安全约定](docs/security/threat-model.md)和[贡献指南](CONTRIBUTING.md)可分别作为入口。

## 开发

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-features --locked --all-targets -- -D warnings
```

请根据修改范围运行相应的 crate、模糊测试、WASM、浏览器或外部工具检查。
测试样例的来源与功能边界也是代码审查的一部分。

## 许可证

可任选 MIT 或 Apache-2.0 许可证。
