# Archived evidence

This historical record is linked from the [evidence index](../playback-evidence.md).

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
