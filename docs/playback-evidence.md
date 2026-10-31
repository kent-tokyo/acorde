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

This swing correction is a bounded event-timing result. The same-input PCM comparison below
uses a straight-timing score; cross-browser playback timing remains unmeasured.

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

As a range check, Acorde's native zone materializer cannot select bank 0/program 0 from the
1.3 MiB `sonivox.sf2` bundled with alphaTab 1.8.4: a render of the independent *Just for Today*
MXL fails with `SoundFont preset (0, 0) was not found`. That SoundFont has a Piano 1 preset, but
the materializer rejects its zones because it cannot represent several envelope/filter
generators. This is a synthesis coverage gap tracked in
[Issue #203](https://github.com/kent-tokyo/acorde/issues/203); the failed render is not an
alphaTab quality comparison.

## Full-SF2 GM render and fixed-rate comparison (2026-10-31)

The optional `--full-sf2` CLI mode uses RustySynth 1.3.7 (MIT) for SF2 filters, modulation, and
envelopes while retaining Acorde's frame schedule. It does not alter the provider-neutral zone
materializer or the default native mode. Its synthesis block is eight frames, so note attacks and
releases can be up to seven output samples later than their scheduled frame. Reverb and chorus are
disabled. The mode is available at 16–192 kHz and continues to reject unsupported score
expression. The alphaTab 1.8.4 bundled `sonivox.sf2` is loaded from its installed npm package,
not copied into this repository.

The full-SF2 mode also applies authored signed 14-bit pitch bend and MIDI CC 1, 7, 10, 11, and 64
on the frame schedule. A two-voice real-SF2 regression checks that CC7 reaches both channels
after voice splitting; another checks that a bend changes only audio after its authored frame.
CC64 holds sound across a rest. Authored MIDI program changes also select the next note's preset,
as tested below; other unsupported or unrouted automation fails explicitly. These regressions
show control realization, not alphaTab audio parity on a real-score controller corpus.

On the independent *Just for Today* MXL (score SHA-256
`79a19736fcce71c0acf4b39feff63723d2ff75e29285ea2ab594f9b2b6a835ff`), Acorde rendered
all 579 scheduled notes using that SF2 (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`). The 16 kHz mono
PCM16 WAV has 1,315,131 frames, SHA-256
`3dd31d0f22017fb2f617dd9c59d55ff3c032daf0397db0a0a550a7b2f9ecd740`, and zero clipped
samples. At 48 kHz stereo, its WAV has 3,945,392 frames and zero clipped samples (SHA-256
`bb93c4ce7567074c06e576b944eee34a01eb74b42992e19fe0f2a649efceb198`). The difference
in full-file durations from alphaTab includes Acorde's fermata hold and release-tail policy; it
is not a measured onset error.

```sh
npm install --prefix /tmp/acorde-alphatab-1_8_4 --no-save --no-package-lock @coderline/alphatab@1.8.4
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
cargo run -p acorde-cli --locked -- audio-render \
  tests/fixtures/openscore_lieder_just_for_today.mxl "$SF2" /tmp/acorde-gm-16k.wav \
  --full-sf2 --sample-rate 16000 --channels 1
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_just_for_today.mxl "$SF2" \
  /tmp/alphatab-gm-16k.f32le 16000
python3 scripts/compare_playback_audio.py /tmp/acorde-gm-16k.wav \
  /tmp/alphatab-gm-16k.f32le 0,652,1304,1957,2609
```

In this run, alphaTab's public export API emitted 1,283,520 stereo float32 frames at 16 kHz
(SHA-256 `afd4eb0f1bb5ceff8dadb9e1a2bf8aa5b3f67a32cbbbac13d264241c274c7ee4`).
**2,542,108 of 2,567,040 float samples were non-finite**, starting at frame 4,950. Its short
`simple.musicxml` render with the same SF2 and rate also produced 33,160 non-finite samples.
The comparison script now reports these explicitly and skips affected waveform correlations.
At 48 kHz, alphaTab exported the same real score with zero non-finite samples (SHA-256
`e35b715158b49ea6852e3844a2d69840d50badaf6ca2736e8ef86a645cdb0ef5`). Thus Acorde
has a fixed-score, fixed-SF2 advantage in **finite 16 kHz output**, but this does not establish
better 48 kHz sound quality, wider repertoire support, or an overall audio win. Full-SF2 rendering
uses an external Rust synthesizer; broader Acorde-native generator coverage and a listening corpus
remain open in [Issue #203](https://github.com/kent-tokyo/acorde/issues/203).

### Explicit PCM16 peak leveling and channel-level audit (2026-10-31)

`acorde audio-render` now accepts `--normalize-peak-dbfs` from -48 to -0.1.
The option scales the completed PCM16 WAV deterministically, without changing
its sample count or relative note levels, and reports samples already at the
PCM rail. It is opt-in and cannot repair clipping or recover precision lost
before PCM16 encoding. `scripts/compare_playback_audio.py` now reports each
file's hash, the peak of individual channel samples, PCM rail samples, and
alphaTab float samples outside [-1, 1], separately from downmixed metrics.

On `simple.musicxml` with the same alphaTab-bundled `sonivox.sf2`, 48 kHz and
two channels, both exports have 192,000 frames. The unmodified Acorde WAV
(SHA-256 `5924323107446e40972f5eddd46ea1aae265bdb437353bda716c52886a2edd02`)
has per-channel peak 0.075897 and RMS 0.012075; alphaTab 1.8.4's float32
export (SHA-256 `781fc384d6480e691ebe9de12d3ef0c45680d0dbd00257a833fb084d3e96b57c`)
has peak 0.592075 and RMS 0.093004. The chosen -4.56 dBFS Acorde export
(SHA-256 `66fdbf239aa45dfcbd00bbb073b3a753c652aa5cfd38a6afd7ccdcce27663095`)
has peak 0.591553, RMS 0.094117 and zero rail samples. This peak was chosen
after observing alphaTab's output; it is a level-control demonstration, not
an independent sound-quality result. A short periodic-wave correlation has
an ambiguous window near 1 second and does not measure onset accuracy.

The pinned CC0 *Aloha Oe* MXL, same SF2, 48 kHz stereo, and a standard
-1 dBFS Acorde target produces 2,928,000 frames, peak 0.891235 and zero
rail samples (SHA-256
`6dcf2cda0cc1800afd0818e866584844b2cbd5988b7cbd61c0a82e13cfe7abb2`).
The existing alphaTab float32 export has 2,970,048 frames, peak 1.244791
and 76 finite channel samples whose absolute value exceeds 1.0. Float output
above 1.0 is not by itself evidence of audible clipping; playback gain and
device conversion are outside this measurement. The known split-bar timing
difference and 73 Acorde MusicXML import diagnostics remain separate issues.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
acorde audio-render tests/fixtures/simple.musicxml "$SF2" /tmp/acorde-simple-leveled.wav \
  --sample-rate 48000 --channels 2 --full-sf2 --normalize-peak-dbfs=-4.56
acorde audio-render tests/fixtures/openscore_lieder_aloha_oe.mxl "$SF2" \
  /tmp/acorde-aloha-leveled.wav --sample-rate 48000 --channels 2 \
  --full-sf2 --normalize-peak-dbfs=-1
python3 -B scripts/compare_playback_audio.py /tmp/acorde-simple-leveled.wav \
  /tmp/alphatab-sonivox-simple-48k.f32le 0,500,1000,1500,2000
```

### Authored program changes in full-SF2 audio (2026-10-31)

Full-SF2 rendering now applies a MIDI program change at its scheduled frame to
every voice channel routed from the same source part and MIDI channel. Later
note-ons retain that selected program instead of resetting to the score's
initial program. The core playback report for the violin fixture now lists
programs 0 and 40 at 0 and 0.5 seconds. A requested preset absent from the SF2 returns
`PresetNotFound`; the basic native SF2 path still rejects this automation.

The two self-authored MIDI fixtures in `tests/fixtures/` play C4 at tick 0 and
E4 at tick 480 (120 BPM). One selects violin program 40 at tick 480; the other
reselects piano program 0. Both were rendered at 16 kHz mono using alphaTab
1.8.4's bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`).
Both Acorde WAVs have 32,000 frames. Their first 8,000 frames are byte-identical;
their later samples differ. The violin-change WAV SHA-256 is
`101817b61abdee6d491aa0395436d377b1a4c583cf5eb46c7441fd2ca4fefd63`;
the piano-control WAV SHA-256 is
`a34b2377e9d9fa84134f6271989fc19a5f8360b5da2d180485c4d1942844744a`.
The Rust regression also checks same-frame routing and that a program change
overrides a stale per-note program in the offline manifest.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
cargo run -q -p acorde-cli -- audio-render \
  tests/fixtures/program_change_piano_violin.mid "$SF2" /tmp/acorde-program-change.wav \
  --sample-rate 16000 --channels 1 --full-sf2
cargo run -q -p acorde-cli -- audio-render \
  tests/fixtures/program_change_piano_piano.mid "$SF2" /tmp/acorde-program-control.wav \
  --sample-rate 16000 --channels 1 --full-sf2
```

The repository's alphaTab audio-export script uses `ScoreLoader`; that API
returned `UnsupportedFormatError` for the synthetic MIDI fixture, so this
check is an Acorde audio regression, not a same-input alphaTab comparison.
It does not establish timbre, dynamic, or release superiority. A licensed
MusicXML score with an equivalent mid-score program change and a wider
listening corpus remain needed for Issue #203.

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

### Same-score PCM lag at the split bar (2026-10-31)

The same Aloha Oe MXL and alphaTab 1.8.4 bundled `sonivox.sf2` were rendered again at
48 kHz. The source MXL contains an initial 80 BPM tempo and starts physical measure 5 at
beat 16 (12.0 seconds); alphaTab's exported MIDI starts it at beat 17 (12.75 seconds).
Acorde's full-SF2 path rendered 2,928,000 mono PCM16 frames and alphaTab's public
audio exporter rendered 2,970,048 stereo float32 frames. The alphaTab export had no
non-finite samples. The input MXL and SF2 hashes are pinned above; the rerendered
Acorde WAV SHA-256 is `4ebdbec8048a2d20bd3a19f9ab2c983552395ed3c72e82c5182c744c34d6a803`
and alphaTab raw audio SHA-256 is
`6c91393e3f60723766ec3676a57f74bcbf1bda132a0412c2020800919217d3a1`.

`compare_playback_pcm_lag.py` downmixes both outputs to mono and finds the maximum
gain-independent normalized waveform correlation in each 500 ms window within a
±1 second search. Positive lag means the matching alphaTab audio occurs later:

| Acorde window start | alphaTab lag | Cosine |
| ---: | ---: | ---: |
| 9 s | 0.000 ms | 0.865940 |
| 10 s | −0.625 ms | 0.983777 |
| 12 s | +749.312 ms | 0.782256 |
| 15 s | +749.292 ms | 0.747214 |
| 18 s | +749.333 ms | 0.863830 |

The shift is visible in actual 48 kHz PCM after the authored split bar, while the
earlier windows align within one millisecond. Together with the independent
MusicXML onset oracle and the exported-MIDI timing, this supports the narrow
conclusion that Acorde follows this score's split-measure timing more faithfully
than alphaTab 1.8.4's default export. Correlation identifies matching audio
passages, not individual note attacks or an absolute hardware playback latency.
It does not establish better timbre, dynamics, release behavior, or overall audio
quality. The unequal tail lengths, output channels, and gain policies are not
compared as quality metrics.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
cargo run -q -p acorde-cli --locked -- audio-render \
  tests/fixtures/openscore_lieder_aloha_oe.mxl "$SF2" /tmp/acorde-aloha-48k.wav \
  --sample-rate 48000 --channels 1 --full-sf2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_aloha_oe.mxl "$SF2" /tmp/alphatab-aloha-48k.f32le 48000
uv run --no-project --with 'numpy==2.5.3' --with 'scipy==1.17.1' \
  python3 -B scripts/compare_playback_pcm_lag.py \
  /tmp/acorde-aloha-48k.wav /tmp/alphatab-aloha-48k.f32le \
  --sample-rate-hz 48000 --windows 9,10,12,15,18
```

### Part mix in stereo on the same score (2026-10-31)

The *Aloha Oe* MXL declares four initial part volumes of `78.7402` percent and pans of
`-33, 0, -33, 36` degrees. Before this change, the importer discarded them. It now maps
volume to MIDI CC7 and front-hemisphere pan to CC10 at tick zero. The imported values are
`(100,40), (100,64), (100,40), (100,89)` for `(CC7,CC10)`. The conversion quantizes to
seven bits; MusicXML's rear-hemisphere pan is diagnosed because CC10 has no corresponding
position. MusicXML's [volume](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/volume/)
and [pan](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/pan/) definitions
provide the source scales.

Using alphaTab 1.8.4's bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`),
both Acorde stereo renders have 2,928,000 frames at 48 kHz and 644 authored playback events.
The prior WAV SHA-256 is `f78bdaee8c7bef1e01245cf7cfbd0759cc6d88dcd7ca183cfa024be2f1a90460`;
the corrected WAV SHA-256 is `7078ce89dd8382119a51231b6d508e10b28dac490ba612e11040e1296ce6f5b7`.
In the first five seconds, right/left RMS changes from **1.0001** to **1.9287** and the
number of frames with different left/right PCM values changes from 349,274 to 2,896,840
over the full output. The revised PCM16 output has zero clipped samples. alphaTab's float32
export for the same score and SF2 has a first-five-second right/left RMS ratio of **1.4834**;
its 2,970,048-frame length reflects the already documented timing difference. The pan law,
gain, and voice balance still differ, so these figures demonstrate restored stereo placement
on this score, not superior sound quality.

The same-output-channel timing check uses the stereo WAV above and alphaTab's
48 kHz stereo float32 export (SHA-256
`6c91393e3f60723766ec3676a57f74bcbf1bda132a0412c2020800919217d3a1`).
After downmixing each to mono for gain-independent correlation, the windows at
9 and 10 seconds align within 0.625 ms. The windows at 12, 15 and 18 seconds
place alphaTab 749.312, 749.292 and 749.333 ms later respectively. Both
exports now have two channels, so their channel-count difference no longer
explains this fixed-score timing result. The source-derived MusicXML onset
oracle still supplies the expected side of the split-bar boundary. This
comparison does not judge stereo imaging or sound quality.

```sh
uv run --no-project --with 'numpy==2.5.3' --with 'scipy==1.17.1' \
  python3 -B scripts/compare_playback_pcm_lag.py \
  /tmp/acorde-aloha-stereo-after.wav /tmp/alphatab-aloha-stereo-review.f32le \
  --sample-rate-hz 48000 --windows 9,10,12,15,18
```

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
cargo run -q -p acorde-cli --locked -- audio-render \
  tests/fixtures/openscore_lieder_aloha_oe.mxl "$SF2" /tmp/acorde-aloha-stereo-after.wav \
  --sample-rate 48000 --channels 2 --full-sf2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_aloha_oe.mxl "$SF2" \
  /tmp/alphatab-aloha-stereo-review.f32le 48000
```

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

### Overlapping unison duration in MIDI export (2026-10-31)

On the pinned CC0 *Just for Today* MXL (SHA-256
`79a19736fcce71c0acf4b39feff63723d2ff75e29285ea2ab594f9b2b6a835ff`), the
current playback JSON contains 579 attacks. Four fermata notes overlap later
attacks at the same pitch across two passages. The previous MIDI export put each
pair on one channel. A FIFO reader then assigned the earlier note-off to the
held note and the later note-off to the short note: eight durations differed
from Acorde's own playback schedule by 0.5 beat.

The exporter now puts a colliding attack on a spare channel that is not used by
another authored part or control stream. It mirrors that part's program,
pitch-bend, controller and aftertouch messages. The revised MIDI SHA-256 is
`6f92936d0826714280c3cf2ccd0fbabac68f7560e656f6009dcc96d94631b809`.
The same FIFO comparison reports **579/579** matching pitch-and-beat attacks,
**0** duration differences over 0.01 beat, and at most **0.110 ms** onset-time
difference from MIDI tempo quantization. It reports eight duration differences
against the pre-change MIDI (SHA-256
`a26c8e9daf965408197e4898e5e96a9fc7342a7d67819f323a15c2fc2f631092`).
The exported track marks each auxiliary channel. On reimport, Acorde suppresses
its extra score automation only when that channel's events exactly mirror the
original. An edited or foreign marker cannot hide a distinct control stream.
Other MIDI readers still receive the mirrored events needed for playback. If
all 15 pitched channels are occupied, export fails explicitly rather than
emitting ambiguous overlapping note-offs. On this fixture, reimport and a
second export retain 579 note-ons, 579 note-offs, six controller events and
three program changes; these counts no longer grow.

alphaTab 1.8.4's MIDI export of this same MXL (SHA-256
`a46691fe284051a3cdb3d6f66588f68e06dd746b7aff8ccc11d24481aea283b8`)
also matches 579/579 pitch-and-beat attacks; 22 note durations differ from
Acorde's *chosen* default fermata hold. A fermata has no fixed performance
duration in the source, so this is not evidence that either renderer has more
accurate fermatas. This change improves Acorde's internal MIDI/playback
consistency; it does not change PCM synthesis or establish an audio-quality win.

```sh
target/debug/acorde playback-report tests/fixtures/openscore_lieder_just_for_today.mxl \
  > target/just-for-today-playback.json
target/debug/acorde convert tests/fixtures/openscore_lieder_just_for_today.mxl \
  target/just-for-today-separated.mid
node scripts/export_alphatab_midi.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_just_for_today.mxl target/just-for-today-alphatab.mid
uv run --no-project --with 'mido==1.3.3' \
  python scripts/compare_playback_onsets.py \
  target/just-for-today-playback.json target/just-for-today-separated.mid
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

## Written grace timing against the same-score MIDI (2026-10-31)

Issue [#249](https://github.com/kent-tokyo/acorde/issues/249) identifies two
written B4 grace notes in Chopin's Prelude Op. 28 No. 4. Acorde previously
preserved them in MusicXML and MSCZ but omitted both playback attacks. The
default event schedule now gives each eighth grace a deterministic 0.125-beat
slot before its following main note. It takes that slot from the preceding
rhythmic position, truncating a preceding sounding note if necessary. A grace
at playback start borrows from the following main note instead. Consecutive
graces share the slot in written order. Cue notes remain silent. The policy
does not claim to reproduce a particular pianist's grace interpretation.

The issue's MusicXML was read locally from the private fisarmonica repository
(SHA-256 `b774112bfdf99d60b600b8bb24996c99cad9fd1c19b2ced66cf45d8bbd75ed89`);
it is not copied into Acorde. The independent [Mutopia source MIDI](https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=468)
has SHA-256 `c1d7e05aa4e6e2baeec2e9108402a969c16d5df302b82fd66d110634697f2e3b`.
After MusicXML → MSCZ → playback, Acorde emits 600 attacks, matching the source
MIDI's count. The two grace B4 onsets are 43.875 and 75.875 beats; the source
MIDI has 43.885417 and 75.885417. The following A4 notes remain at 44 and 76
beats in both. The preceding F♯4 notes end 0.010417 beat earlier than in the
source MIDI. The final fermata duration still differs by design. The source
MIDI's tempo map differs from the imported MusicXML's 120 BPM, so this is a
beat-position comparison, not an absolute-seconds claim.

With `@coderline/alphatab@1.8.4`, the same MusicXML produces a MIDI file
(SHA-256 `f8dae2042c8d8786f7aaffb6f1928736f19c160e639eaa7325ac399d9f62ef5f`)
whose two B4 attacks are at 44 and 76 beats and whose following A4 attacks are
at 44.125 and 76.125. Acorde is closer to the independent source MIDI on these
four onsets. The alphaTab MIDI has 603 attacks overall, so this focused result
does not establish general playback or audio-quality superiority. A checked-in
synthetic MusicXML → MSCZ regression reproduces the same rhythmic shape without
depending on the private source file.
Without ornament realization, the complete score's full-SF2 WAV reports
`unrealized note expression` for a `Turn` in measure 17. This remains an
explicit default boundary rather than silently dropping the mark.

### Opt-in ornament WAV on the same score

`audio-render --realize-ornaments` uses the existing versioned
`ornament-arpeggio-v1` event policy. Once a turn, trill or related mark has
become individual attacks, those attacks no longer carry the same mark as an
unhandled instruction; other articulations remain. A note with multiple
different ornament marks is still left unhandled and produces an explicit
provider error. The default playback policy is unchanged.

On the same Chopin MusicXML after MSCZ round trip, using alphaTab 1.8.4's
bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`),
Acorde full-SF2 renders **603 events** to a 48 kHz stereo PCM16 WAV. It has
2,520,000 frames, SHA-256
`e65b241d6bf7da6fe96f556cd8c710a0241fa1506c6dcabfd6df0a180d620dd9`,
sample peak 0.174286 and zero PCM rail samples. alphaTab's public export on
the same MusicXML, SF2 and rate gives 2,424,000 stereo float32 frames, SHA-256
`f6b487319d722bf86cd6b509f863c2f9cffe64f8f26851f952dafe8e899e26b0`,
sample peak 0.667700, no samples outside [-1, 1], and no non-finite samples.
The different output lengths include Acorde's one-second release tail and its
chosen final-fermata hold. The peak difference reflects different gain policies.
These figures establish complete, finite output on this score, not better
sound quality or equal interpretation. Gain-independent local waveform
correlations vary substantially across the score, so listening and a broader
permissioned corpus remain necessary under Issue #203.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
acorde audio-render /tmp/acorde-chopin-prelude-op28-4.mscz "$SF2" \
  /tmp/acorde-chopin-grace-turn-48k.wav --full-sf2 --realize-ornaments \
  --sample-rate 48000 --channels 2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  /tmp/acorde-chopin-prelude-op28-4.musicxml "$SF2" \
  /tmp/alphatab-chopin-48k.f32le 48000
python3 scripts/compare_playback_audio.py /tmp/acorde-chopin-grace-turn-48k.wav \
  /tmp/alphatab-chopin-48k.f32le 0,500,1000,2000,5000,10000,20000,30000,40000
```

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
