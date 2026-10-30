# Playback and tablature evidence

This document records reproducible local playback and tablature evidence. It separates
deterministic score/event contracts and basic native WAV output from browser and audio-quality
evidence.

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
repository still has no PCM-output comparison or cross-browser playback timing measurement.

## Native SF2 WAV output (2026-10-31)

`acorde audio-render` uses the core's integer-frame offline schedule and a caller-supplied SF2
to write PCM16 WAV. The renderer decodes matching SoundFont zones, resamples with linear
interpolation, mixes notes at their scheduled frame, and bounds the WAV at 64 MiB. SF2 note-off
release decays to silence within its SoundFont release time; score pedal spans hold released notes
until pedal-up on the same part. The CLI reserves one second of release tail by default and
accepts `--release-tail-ms` to change it. The renderer rejects MIDI controller automation
(including CC64), program change, pitch bend, ornament, guitar technique, microtonal, and SF3
semantics. It has no effects or live audio device. A score whose requested preset is absent from
the SF2 fails with a typed error.

The licensed Upright Piano SF2 fixture is recorded in `tests/fixtures/README.md`. Reproduce a
short MusicXML render from the repository root:

```sh
cargo run -p acorde-cli --locked -- audio-render \
  tests/fixtures/simple.musicxml \
  tests/fixtures/UprightPianoKW-small-20190703.sf2 \
  /tmp/acorde-simple-sf2.wav --sample-rate 8000 --channels 1
```

The command produces a 4-second, 8-kHz mono PCM16 WAV with five scheduled events, including one
second of release tail. A Rust
regression constructs a quarter-rest then a quarter-note at 120 BPM and checks that the first
4,000 output frames are silent and later frames contain audio. Separate resampling regressions
check fractional-rate interpolation, loop-boundary interpolation, and silence after an unlooped
sample ends. Another real-SF2 regression checks that the release tail decays to silence and that
pedal-down holds a note across a rest until the pedal-up mark. These tests verify timing and basic
PCM generation; they do not measure audio quality or establish superiority over alphaTab's
SoundFont synthesizer.

## Same-input PCM baseline against alphaTab 1.8.4 (2026-10-31)

The checked-in synthetic `simple.musicxml` (SHA-256
`55c2d8d2546100437460474b8bc685b173493c3696303cbad2eb85254acbfd7d`) and the licensed
Upright Piano SF2 (SHA-256
`cf2a98eb38a32c4954b4b6e2caae4112d62dd8e892eceefdd7942b0e7d01ac2f`) were given to
both Acorde and `@coderline/alphatab@1.8.4`. alphaTab's
[public audio export API](https://docs.alphatab.net/docs/guides/audio-export)
produces interleaved stereo float32 samples. The script pins the package version and writes the
raw samples without changing alphaTab's synthesis. Acorde writes mono PCM16 here. The comparison
downmixes alphaTab to mono and uses a gain-independent cosine similarity within a 100 ms window
starting 100 ms after each written note onset. It searches a maximum 16-frame alignment shift.
The local run used Node.js 24.5.0, npm 11.5.1, Python 3.13.6, and Rust 1.97.0.

```sh
npm install --prefix /tmp/acorde-alphatab-1_8_4 --no-save --no-package-lock @coderline/alphatab@1.8.4
cargo run -p acorde-cli --locked -- audio-render tests/fixtures/simple.musicxml \
  tests/fixtures/UprightPianoKW-small-20190703.sf2 /tmp/acorde-simple.wav \
  --sample-rate 8000 --channels 1
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/simple.musicxml tests/fixtures/UprightPianoKW-small-20190703.sf2 \
  /tmp/alphatab-simple.f32le 8000
python3 scripts/compare_playback_audio.py /tmp/acorde-simple.wav \
  /tmp/alphatab-simple.f32le 0,500,1000,1500,2000
```

The Acorde WAV contains 32,000 frames (SHA-256
`a6e3de8601284632768f1121eecd4e874ebebcdbc9ae6df7341bd72460ada8cb`); alphaTab emits
32,064 frames (raw PCM SHA-256
`87d10c86f7b1d5bac176a04e9cdb2a1503d02ebb24248100e832ccec35abb8d8`). The five
alignment cosines are **0.999965, 0.990789, 0.999180, 0.996500, 0.999464**. The selected
alphaTab array shifts are **0, -5, 0, -9, 0 frames**. These shifts are signal alignment values,
not independent note-onset error measurements. Different channel layouts and gain policies also
prevent interpreting the peaks (Acorde 0.520203, alphaTab 0.463316) as loudness quality. This
single synthetic piano score does not establish audible or general playback superiority.

As a range check, Acorde's current zone materializer cannot select bank 0/program 0 from the
1.3 MiB `sonivox.sf2` bundled with alphaTab 1.8.4: a render of the independent *Just for Today*
MXL fails with `SoundFont preset (0, 0) was not found`. That SoundFont has a Piano 1 preset, but
the materializer rejects its zones because it cannot represent several envelope/filter
generators. This is a synthesis coverage gap tracked in
[Issue #203](https://github.com/kent-tokyo/acorde/issues/203); the failed render is not an
alphaTab quality comparison.

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
and physical measure lengths. It allows a two-tick MIDI rounding window (2/960 beat for this
alphaTab export), which is important for tuplets; the result here is unchanged. Both exports
contain **644** note attacks. Acorde matches all
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

## Pinned OpenScore Lieder timing sample (2026-10-30)

The [40-score selection and per-file results](playback-corpus-2026-10-30.json) use OpenScore/Lieder
at commit `38c5db510224d9facdc4b08d741fc788cfb58ea8`. The selection is the first 40 MXL
paths ordered by the SHA-256 of each UTF-8 repository path. The narrow independent MusicXML
onset reader accepted 16 scores and rejected 24: 21 contain grace or cue notes, two have a
backup before the measure start, and one uses navigation or transposition outside the reader.
The accepted scores contain **15,189** written pitch-and-beat attacks. After fixing ties that
change voice or staff and rests with display-position children, Acorde emits exactly
**15,189/15,189** matching attacks with no extras. The checked-in Webern and Quilter MXL files
are the two focused regressions; their source-derived onset fingerprints are asserted in the
I/O integration suite. Matching allows two MIDI ticks for tuplet rounding.

alphaTab 1.8.4's default MIDI export also matches all 15,189 written attacks in this sample.
It emits 173 further attacks, including tremolo realizations, which the narrow written-onset
reader does not classify. Those are not evidence of 173 alphaTab errors. The sample establishes
Acorde correctness only within the reader's supported subset and does not show general timing
superiority over alphaTab.

## Evidence boundary

This local evidence proves deterministic score projection, event scheduling/position calculation,
typed diagnostics, bounded JSON boundaries, and basic native SF2-to-WAV output. It does not prove:

- Web Audio timing or rendered PCM equivalence;
- browser-driver or cross-browser visual equivalence;
- Verovio/MuseScore engraving or publication equivalence;
- music21 agreement; or
- publication-ready multipage PDF, music-font embedding, or OS printing.

Those items remain open until permissioned fixtures, pinned host/tool versions, and the required
external review environments are available.
