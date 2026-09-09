# Playback and tablature evidence

This document records the reproducible local evidence for Phase 10.7. It deliberately separates
deterministic score/event contracts from host-owned audio and browser evidence.

## Local gate

Run from the repository root:

```text
cargo test --all
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The tests cover the bounded SoundFont materialization fixtures, playback timing comparison,
tablature performance projection, CLI playback reports, and CLI playback comparisons. The core
regression suite also verifies that `to_playback_events_bounded` preserves the canonical schedule
for ordinary scores. The event bound is `MAX_PLAYBACK_COMPARISON_EVENTS`; comparison mismatches
are retained as typed, bounded diagnostics.

## CLI reproduction

Generate an expected schedule for a checked-in fixture, then compare it with an unchanged copy:

```text
cargo run -p acorde-cli -- playback-report tests/fixtures/simple.musicxml --bpm 120 \
  > /tmp/acorde-expected.json
cp /tmp/acorde-expected.json /tmp/acorde-actual.json
cargo run -p acorde-cli -- playback-compare /tmp/acorde-expected.json /tmp/acorde-actual.json \
  --fail-on-mismatch
```

The comparison must report `within_tolerance: true`, an empty `mismatches` array, and equal
expected/actual event counts. `playback-report` also accepts inclusive physical measure bounds:

```text
cargo run -p acorde-cli -- playback-report tests/fixtures/simple.musicxml \
  --bpm 120 --loop-start 0 --loop-end 0
```

`tab-performance-report --fail-on-diagnostics` is the corresponding local gate for authored
string/fret positions. It reports missing positions, invalid strings, unavailable tuning, and
pitch mismatches without inventing a fingering.

`tablature_round_trip_report(score_json)` verifies that score-model JSON persistence retains the
tablature configuration and authored per-pitch candidates. This is a model-persistence check, not
a MusicXML/MSCX or external-editor round-trip claim.

## Evidence boundary

This local evidence proves deterministic parsing, score projection, event scheduling, typed
diagnostics, and bounded JSON boundaries. It does not prove:

- Web Audio timing or rendered PCM equivalence;
- browser-driver or cross-browser visual equivalence;
- Verovio/MuseScore engraving or publication equivalence;
- music21 agreement; or
- PDF generation, font embedding, or OS printing.

Those items remain open until permissioned fixtures, pinned host/tool versions, and the required
external review environments are available.
