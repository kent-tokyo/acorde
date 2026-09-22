# Contributing to acorde

acorde is a synchronous, platform-neutral score library. Keep UI, filesystem access, audio
devices, fonts, PDF generation, and print dialogs in hosts such as Composer.

## Before opening a change

- Keep `core` independent from I/O and layout; do not add Tauri, async runtimes, or filesystem
  access to core, io, or layout.
- Add `#[serde(default)]` to model additions so existing score JSON remains readable.
- For a `Note` or `Measure` field, add the matching command/history behavior, parser and
  serializer support, and an explicit loss diagnostic for unsupported interchange formats.
- Put self-authored or permission-cleared score fixtures in `tests/fixtures`; do not fetch test
  corpora at runtime.
- State capability boundaries precisely. A deterministic local test is not an external-tool,
  browser, audio-quality, or publication-quality comparison.

## Required checks

```bash
cargo fmt --check
cargo test --all
cargo clippy --all --all-targets -- -D warnings
git diff --check
```

Run the focused feature, fuzz, WASM, browser, or package checks when the edited surface needs
them. Keep commands transactional: invalid input must return a typed error without mutating the
score or recording history.

## Small, reviewable changes

Explain the supported semantic slice, intentional losses, test fixtures, and host-owned behavior.
Avoid claiming full compatibility with MusicXML, MEI, MIDI, MuseScore, Verovio, alphaTab, or
music21 unless a pinned, reproducible external comparison proves that exact claim.
