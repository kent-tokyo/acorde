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

## alphaTab 1.8.4 / split-measure CC0 score (2026-10-30)

The independent OpenScore Lieder *Aloha Oe* MXL is pinned in the fixture manifest (23 physical
measures, four parts; SHA-256
`b195ae477a10cbaee281d9b728cac001ae1d50fe4527078ccae14092eee4936c`). The same
repository's [source MSCX](https://github.com/OpenScore/Lieder/blob/38c5db510224d9facdc4b08d741fc788cfb58ea8/scores/Liliuokalani,_Queen_of_the_Hawaiian_Islands/_/Aloha_Oe/lc6650166.mscx)
explicitly marks a 3/4 measure followed by a 1/4 measure, then a 7/8 measure followed by a 1/8
measure. The checked-in MSCX fixture has identical bytes (SHA-256
`f588fc91a36629d6aa079721571ce6e2afb8c8dfafa77935c8198b2d3ec5fd51`).

With default import settings, Acorde reads those physical measure lengths as **3, 1, 3.5, and
0.5 quarter-note beats**. Its first attacks in physical measures 5 and 14 start at absolute beats
**16 and 48.5**. alphaTab 1.8.4's `ScoreLoader` and `MidiFileGenerator` treat the preceding
3-beat and 3.5-beat measures as **4 beats** each: `masterBars[5].start` is 17 beats and
`masterBars[14].start` is 50 beats (960 ticks per beat). This is a concrete timing case where
Acorde preserves the authored split-measure positions and alphaTab's default MIDI export does not.
The first Acorde comparison exposed a second bug: the two chorus parts had no explicit length on
their silent split measures, so their later attacks drifted 1.5 beats. Playback now resolves a
physical measure's authored length across all staves. The Rust regression checks the resumed
chorus timing and a source-derived fingerprint of every attack.

Reproduce with the same pinned alphaTab package, then inspect its bar starts and compare emitted
note onsets:

```sh
node scripts/export_alphatab_midi.mjs /tmp/acorde-alphatab-1_8_4 tests/fixtures/openscore_lieder_aloha_oe.mxl /tmp/acorde-alphatab-1_8_4/aloha-oe.mid
target/debug/acorde playback-report tests/fixtures/openscore_lieder_aloha_oe.mxl > /tmp/acorde-alphatab-1_8_4/aloha-oe.json
uv run --no-project --with 'mido==1.3.3' python scripts/compare_playback_onsets.py /tmp/acorde-alphatab-1_8_4/aloha-oe.json /tmp/acorde-alphatab-1_8_4/aloha-oe.mid
uv run --no-project --with 'mido==1.3.3' python scripts/compare_musicxml_onsets.py tests/fixtures/openscore_lieder_aloha_oe.mxl /tmp/acorde-alphatab-1_8_4/aloha-oe.json /tmp/acorde-alphatab-1_8_4/aloha-oe.mid
```

The alphaTab MIDI SHA-256 is
`ec3665d5ee2bf115d2ef32c6b9607d937112c14aeb772f324c5f6dda88a89ca5`; the Acorde
JSON SHA-256 is `241f10d4f044034224122823583804bb9231190b32a362df57413d2a88ab15ef`.
The independent, narrow MusicXML onset reader checks written durations, backups, chords, ties,
and physical measure lengths. Both exports contain **644** note attacks. Acorde matches all
**644/644** written pitch-and-beat onsets; alphaTab's default MIDI export matches **201/644**.
The source onset fingerprint is
`775250e82772f839c7a535f0d76a4b1fd478dd13d1757a300e8077e0d85cbe21`.
This is a same-score win on written note onset timing for this fixture. The oracle rejects score
features it does not implement, and this result does not establish broader repertoire accuracy,
audio synthesis, velocity, or sound quality.

## MIDI export timing parity (2026-10-30)

The core playback schedule and `acorde convert … .mid` are separate delivery paths. On *Just for
Today*, the previous MIDI export emitted six silent cue notes as attacks (**585** instead of
**579**). It also read tempo only from the first staff, and measured irregular bars from the
first staff even when another part supplied the authored length. MIDI export now skips cue notes,
uses the score-wide tempo and physical measure length, and applies the default fermata, staccato,
and accent note behavior used by `to_playback_events`.

The corrected *Just for Today* MIDI SHA-256 is
`ba55c164f7c112a3f21feb9f61ef4098568d9cf5019d94b77c4eeb7275f8667a`.
Its **579/579** pitch-and-beat onsets match Acorde playback JSON. The FIFO MIDI note-on/off
comparison still reports eight note-duration differences, so this is an onset parity claim. On
*Aloha Oe*, the Acorde MIDI SHA-256 is
`25f8453f9920584298b93d7b77cd9cdee9f64573bd9d526b25912429ed0a6682`; all **644/644**
attacks match both the playback JSON and the independent MusicXML onset oracle. The alphaTab
1.8.4 default MIDI result on that same score remains **201/644** source matches.

Reproduce the two Acorde MIDI exports after `cargo build -p acorde-cli --locked`:

```sh
target/debug/acorde convert tests/fixtures/openscore_lieder_just_for_today.mxl /tmp/acorde-just-for-today.mid
target/debug/acorde convert tests/fixtures/openscore_lieder_aloha_oe.mxl /tmp/acorde-aloha-oe.mid
uv run --no-project --with 'mido==1.3.3' python scripts/compare_playback_onsets.py /tmp/acorde-alphatab-1_8_4/acorde.json /tmp/acorde-just-for-today.mid
uv run --no-project --with 'mido==1.3.3' python scripts/compare_musicxml_onsets.py tests/fixtures/openscore_lieder_aloha_oe.mxl /tmp/acorde-alphatab-1_8_4/aloha-oe.json /tmp/acorde-aloha-oe.mid
```

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
