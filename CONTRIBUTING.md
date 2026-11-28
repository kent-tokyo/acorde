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
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-features --locked --all-targets -- -D warnings
python3 -B scripts/check_release_consistency.py
git diff --check
```

Run the focused feature, fuzz, WASM, browser, or package checks when the edited surface needs
them. Keep commands transactional: invalid input must return a typed error without mutating the
score or recording history.

## Compatibility policy

The published crates follow [Cargo's SemVer rules](https://doc.rust-lang.org/cargo/reference/semver.html)
for Rust source compatibility, not only for stored score JSON. Serde defaults keep old JSON
readable, but they do not keep Rust struct literals or exhaustive `match` expressions compiling.

- **Patch releases** (`x.y.Z`) contain fixes only. Do not add public enum variants, public struct
  fields, or required trait items; do not remove or rename public items or change their
  signatures.
- **Minor releases** (`x.Y.0`) may add public items, such as new functions, types, or variants of an
  enum marked `#[non_exhaustive]`. They must not break code that compiled against the previous
  minor release.
- **Major releases** (`X.0.0`) are the only place for source-breaking changes. List each one, with the
  consumer action it needs, in [`docs/migrations.md`](docs/migrations.md).

`cargo semver-checks` runs in CI against the latest version on crates.io. If it reports a
breaking change, either make the change compatible (for example, add a constructor instead of a
new required field) or hold it for the next major release. Prefer `#[non_exhaustive]` on new public
error enums and on enums or structs that are expected to grow.

Before a release, `python3 -B scripts/check_release_consistency.py` must report that the
workspace version, internal dependency versions, `docs/scorecard.json`, the newest `CHANGELOG.md`
release heading, and the `ROADMAP.md` baseline agree. CI and the publish workflow run it.

## Small, reviewable changes

Explain the supported semantic slice, intentional losses, test fixtures, and host-owned behavior.
Avoid claiming full compatibility with MusicXML, MEI, MIDI, MuseScore, Verovio, alphaTab, or
music21 unless a pinned, reproducible external comparison proves that exact claim.
