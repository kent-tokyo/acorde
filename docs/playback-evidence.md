# Playback and tablature evidence

This document records reproducible local playback and tablature evidence. It deliberately separates
deterministic score/event contracts from host-owned audio and browser evidence.

## Local gate

Run from the repository root:

```text
cargo test --all-features --locked
cargo clippy --all-features --all-targets --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The tests cover bounded SoundFont materialization fixtures, score-to-event projection, playback
position calculation, timing comparison, tablature performance projection (including authored
guitar techniques and bend alteration cents), CLI playback reports, and CLI comparisons. The core
suite verifies that `to_playback_events_bounded` preserves an ordinary score schedule. The event
bound is `MAX_PLAYBACK_COMPARISON_EVENTS`; mismatches remain typed, bounded diagnostics.

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
expected/actual event counts. `playback-report` accepts inclusive physical measure bounds:

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

## Swing pair timing (2026-10-30)

The [alphaTab 1.8.4 MIDI generator](https://github.com/CoderLine/alphaTab/blob/v1.8.4/packages/alphatab/src/midi/MidiFileGenerator.ts#L822-L870)
checks the duration, beat-grid position, and next beat before applying triplet feel. Acorde's
`PlaybackOptions.swing` previously stretched an isolated matching note, shifting the onset of
the following note. It now applies the requested ratio only to an adjacent, aligned pair of plain
rhythmic units; a rest can occupy either unit. The first measure's authored pickup length is
positioned on the full bar's beat grid. Non-finite or out-of-range ratios leave timing straight.
The `swing_*` tests in `crates/core/src/model/playback.rs` cover these cases.

This is a bounded event-timing correction, not a same-input alphaTab audio comparison. The
repository still has no bundled synthesizer, PCM-output comparison, or cross-browser playback
timing measurement.

## Evidence boundary

This local evidence proves deterministic score projection, event scheduling/position calculation,
typed diagnostics, and bounded JSON boundaries. It does not prove:

- Web Audio timing or rendered PCM equivalence;
- browser-driver or cross-browser visual equivalence;
- Verovio/MuseScore engraving or publication equivalence;
- music21 agreement; or
- publication-ready multipage PDF, music-font embedding, or OS printing.

Those items remain open until permissioned fixtures, pinned host/tool versions, and the required
external review environments are available.
