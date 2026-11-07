# Written note onsets against alphaTab 1.9.0 (2026-11-07)

This is a narrow, source-derived timing audit on four licensed OpenScore Lieder
MXL fixtures in [`tests/fixtures/manifest.json`](../../tests/fixtures/manifest.json).
The [per-score result](playback-alpha19-source-onsets.json) pins each input,
Acorde schedule and alphaTab MIDI hash, plus the Acorde CLI binary hash. Audio
files and external packages are not committed.

## Comparison contract

[`compare_musicxml_onsets.py`](../../scripts/compare_musicxml_onsets.py) reads
written durations, backups, forwards, chords, ties and silent cue notes directly
from the MusicXML. It rejects grace notes, repeats, transposition and unsupported
pitch forms. The Acorde schedule uses `playback-report --fermata-multiplier 1.0`
so its beat positions describe the written score without Acorde's optional
expressive fermata stretching. alphaTab 1.9.0 uses its `MidiFileGenerator`
through [`export_alphatab_midi.mjs`](../../scripts/export_alphatab_midi.mjs).
MIDI note-on positions are matched by pitch within two of alphaTab's 960 ticks
per quarter note. This compares note attacks in score beats, not audio attack
quality or note releases.

The [written note-span follow-up](playback-alpha19-source-spans.json) also joins
MusicXML sound ties and checks MIDI/schedule note-offs against written ends.
It excludes spans with slurs, articulations, fermatas, ornaments, pedal marks
in the part, or overlapping equal pitches from its strict duration count:
these can change performed release or make a MIDI note-off ambiguous. The
exclusion is conservative and applies equally to both engines. The report
records the inputs, Acorde binary, alphaTab package and outputs by SHA-256.

| Score | Acorde written ends | alphaTab written ends |
| --- | ---: | ---: |
| Aloha Oe | 376/376 | 135/161 |
| Just for Today | 324/324 | 324/324 |
| Quilter Prelude | 109/109 | 109/109 |
| Webern, So ich traurig bin | 29/29 | 29/29 |
| **Total** | **838/838** | **597/623** |

Of 1,536 source note spans, 698 carry at least one release caveat. alphaTab
has fewer eligible matched spans because its *Aloha Oe* attack positions differ
from the source. Its 26 strict note-end differences are also in that score;
they require a bar-by-bar attribution before being called independent release
defects. These written durations do not measure sustain-pedal sound, SF2
envelopes, audio decay, or subjective articulation quality.

| Score | Source attacks | Acorde matched | alphaTab matched |
| --- | ---: | ---: | ---: |
| Aloha Oe | 644 | 644 | 201 |
| Just for Today | 579 | 579 | 579 |
| Quilter Prelude | 181 | 181 | 181 |
| Webern, So ich traurig bin | 132 | 132 | 132 |
| **Total** | **1,536** | **1,536** | **1,093** |

The 443 alphaTab mismatches are all in *Aloha Oe*. Its authored split measures
start at beats 16 and 48.5, while alphaTab's exported master bars start at 17
and 50. This repeats the prior 1.8.4 observation with 1.9.0. Both tools match
every written onset in the other three fixtures. Import diagnostics and
unsupported forms remain separate from this timing result; it is not a claim
of general playback or audio superiority.

## Whole-measure-rest fermata correction

The first direct comparison of *Just for Today* against written beats appeared
to show only 130/579 Acorde matches. That mixed different clocks: Acorde's
default `fermata_multiplier = 1.5` deliberately delays following events, while
the MusicXML reader and alphaTab MIDI use unextended written beats. The new CLI
option exposes an exact written-beat comparison without changing the default.

The clock audit also exposed a real default-playback defect. Bar 25 is 3/4
and contains MusicXML `<rest measure="yes"/>` with a fermata. The importer
represents it as a plain whole rest, whose written length is the **three-beat
measure**. The fermata hold calculation previously used four beats, placing an
extra two-beat hold inside the following bar. It now uses the authored measure
length and shares one 1.5-beat hold with the simultaneous pitched fermatas at
that barline. The first attack in bar 27 moves from beat 84 to beat 82 in the
default schedule; the final bar's first attack moves from 126 to 124.

With the same 48 kHz stereo SF2 settings as the eight-score audio observation,
the corrected *Just for Today* WAV has 4,070,609 frames versus 4,133,218
before the fix, a 1.304354-second reduction equal to two beats at the source's
92 BPM. The corrected WAV SHA-256 is
`d57f71fdd6cb438b1c6660ee4aa87dcc88a469a57528001feb3abf2b57f47bb0`.
This checks the actual render path, but it does not determine which expressive
fermata policy sounds better. The earlier alphaTab buffer-length comparison
used the defective Acorde schedule and must be interpreted with this correction.

## Reproduction

Build `acorde-cli` with this change and install `@coderline/alphatab@1.9.0`
into a temporary prefix. For each of the four fixture stems in the table:

```sh
acorde playback-report tests/fixtures/openscore_lieder_aloha_oe.mxl \
  --fermata-multiplier 1.0 > /tmp/aloha-written.json
node scripts/export_alphatab_midi.mjs /tmp/acorde-alphatab-1_9_0 \
  tests/fixtures/openscore_lieder_aloha_oe.mxl /tmp/aloha-alpha19.mid 1.9.0
uv run --no-project --with mido==1.3.3 python3 -B \
  scripts/compare_musicxml_onsets.py \
  tests/fixtures/openscore_lieder_aloha_oe.mxl \
  /tmp/aloha-written.json /tmp/aloha-alpha19.mid
```

The Rust regression `whole_measure_rest_fermata_uses_three_four_bar_length`
checks a lone whole-measure rest and simultaneous pitched fermatas. The CLI
regression checks the written-beat override on *Just for Today*. The source
oracle's all-four score pass checks that cue notes do not produce attacks.
