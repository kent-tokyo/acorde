# Archived evidence

This historical record is linked from the [evidence index](../playback-evidence.md).

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

The measurements and hashes in this section describe the 2026-10-31 implementation.
The subsequent shared-clock fermata change delays later attacks instead of deliberately
overlapping them, so rerunning these commands on the newer implementation will produce
different timings and hashes. The historical observations below are not a current benchmark.

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
