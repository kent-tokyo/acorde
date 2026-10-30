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

## alphaTab 1.8.4 / independent CC0 score (2026-10-30)

The checked-in OpenScore Lieder *Just for Today* MXL is an independent 41-measure, two-part
score (source SHA-256 `79a19736fcce71c0acf4b39feff63723d2ff75e29285ea2ab594f9b2b6a835ff`).
The fixture manifest records its revision and CC0 source. Acorde previously played six
zero-duration cue notes and used 120 BPM for the piano even though the voice part held the
score's initial 92 BPM direction. Score-wide tempo marks now govern all parts, metronome clicks,
cursor positions, and offline render timing; cue notes no longer make sounding events.

The same MXL was imported by `@coderline/alphatab@1.8.4` (npm integrity
`sha512-VN5rfTZZWgA63Ny1aDKCp02k3Qm9CHhg4Q9AnK0kHm7G+fNDNZo36TeToPDFoJ6VpB9+AHcCrHwHFUP1tKqdsw==`)
and exported to SMF1.0 using its `MidiFileGenerator`. The resulting MIDI SHA-256 is
`a46691fe284051a3cdb3d6f66588f68e06dd746b7aff8ccc11d24481aea283b8`.
The Acorde `playback-report` JSON SHA-256 is
`0dfa33d071903c49e4663e6bc14c883587a583f4f044bcdc8d053adb36b3a3e6`.
Reproduce with Node.js, Cargo, and optional `mido==1.3.3`:

```sh
npm install --prefix /tmp/acorde-alphatab-1_8_4 --no-save --no-package-lock @coderline/alphatab@1.8.4
node scripts/export_alphatab_midi.mjs /tmp/acorde-alphatab-1_8_4 tests/fixtures/openscore_lieder_just_for_today.mxl /tmp/acorde-alphatab-1_8_4/score.mid
cargo build -p acorde-cli --locked
target/debug/acorde playback-report tests/fixtures/openscore_lieder_just_for_today.mxl > /tmp/acorde-alphatab-1_8_4/acorde.json
uv run --no-project --with 'mido==1.3.3' python scripts/compare_playback_onsets.py /tmp/acorde-alphatab-1_8_4/acorde.json /tmp/acorde-alphatab-1_8_4/score.mid
```

The selected projection matches all **579** note attacks by written pitch and beat onset;
neither output has an unmatched attack. The maximum start-time difference is **0.110 ms**,
consistent with MIDI's integer microseconds-per-quarter tempo encoding. The 22 note-duration
differences above 0.01 beat are all on Acorde notes with fermatas: Acorde's default hold
multiplier is 1.5 while this alphaTab MIDI export keeps the written duration. The script does
not compare audio samples, velocities, sound quality, or broader score coverage. This one-score
timing agreement is not evidence of overall playback superiority.

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
