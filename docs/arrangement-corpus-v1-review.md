# Arrangement corpus v1: structural baseline

The eight original source scores in `tests/fixtures/arrangement/corpus-v1.json`
use explicit chord positions and a fixed tempo of 120 BPM. Four are development
cases and four are holdout cases. The core test renders each source with five
opt-in profiles, always adding one chordal-pad part and one independent bass part.
`tests/fixtures/arrangement/baseline-v1.json` freezes all 40 results.

| Profile | Pad | Voicing | Bass | Pad events | Bass events | Pad motion, written / playback | Bass motion, written / playback |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| `held-root` | Held | Root position | Root pulse | 57 | 63 | 183 / 215 | 42 / 52 |
| `offbeat-fifth` | Offbeat pulse | Smooth | Root/fifth | 177 | 63 | 56 / 68 | 233 / 267 |
| `arpeggio-approach` | Up/down arpeggio | Smooth | Approach | 121 | 63 | 56 / 68 | 187 / 217 |
| `held-open` | Held | Open | Root pulse | 57 | 63 | 58 / 70 | 42 / 52 |
| `metered-ending` | Meter pulse with final phrase hold | Global smooth | Root/fifth | 144 | 63 | 56 / 68 | 233 / 267 |

The numbers count playback note events, including repeat expansion. The bass
patterns have the same event count in this corpus; their pitches differ.
Pad motion is the summed symmetric nearest-tone distance between chord changes;
the playback column includes the repeat-back transition. The two smooth
profiles share pad pitches, so their pad-motion totals match despite different
rhythms and bass lines. Bass motion is the summed absolute semitone distance
between successive bass attacks. Higher movement does not mean better bass.
The open profile has the same attack count as the held root-position profile,
but its chord-tone octaves differ. All five profiles produce different combined
pad-and-bass event fingerprints
for every case. The test also checks valid scores, unchanged source part JSON,
unchanged source playback pitches and timing, and exactly two new parts.

The snapshot stores the source fingerprint, explicit chord plan, selected
patterns, metrical groups, phrase start ticks, instrument programs, onset
positions and duration counts at 480 units
per beat, pitch-class counts, MIDI range, event fingerprints for both new
parts, and pad/bass motion in written and playback order. These measurements
establish repeatable structural differences. They do
not assess phrase shape, voice-leading quality, melody clearance, balance, or
whether any profile sounds better. All five profiles use the same supplied
harmony; harmonic diversity is still unmeasured here.

Before accepting musical-quality claims, listen to each fixed source and its
five results using the same sound source, tempo and level. Record concrete
issues such as a pad masking the melody, a bass leap at a chord boundary, an
unconvincing 6/8 or 5/8 accent, and a weak phrase ending. Review the holdout
only after choosing changes from the development cases. The snapshot test runs
over the holdout to protect structure; it does not substitute for that review.

The reusable listening pack is generated with:

```sh
cargo run -p acorde-cli --example arrangement_listening_pack -- target/arrangement-listening-v1 development
```

Serve the generated directory locally and open `http://127.0.0.1:8765/` to
compare the original and three unified proposals per development case:

```sh
python3 -m http.server 8765 --bind 127.0.0.1 --directory target/arrangement-listening-v1
```

Direct `file://` access was denied by Chrome in this environment; the loopback
page and proposal WAV playback were verified in Chrome. The pack contains 16 WAV files,
16 MusicXML scores, four exact source-score JSON files and `manifest.json`
with candidate plans, resolved options, features and difference codes. The
source-score JSON is the input for replaying the source-bound proposal
contracts. The proposal request includes a bounded
countermelody: each development case has one accompaniment-only reference and
two answer textures, so the three-candidate comparison covers a change in
orchestration as well as rhythm and harmony. Every WAV is 22,050 Hz mono,
120 BPM, rendered
with the same included upright-piano SF2 and fixed gain, without per-file
normalization. The measured development pack has no sample-rail hits. The
original and proposals have equal duration within each case, including the
repeat-expanded major case. Its target directory is a generated local artifact,
not a committed fixture. Generate the holdout later by replacing the final
argument with `holdout`; do not use it to tune the development choices.

The schema-2 manifest now carries `event_summary_schema_version: 1`. Each
candidate retains the source and proposal MusicXML SHA-256, the source part
count, and per-part written attack positions by measure, written duration and
pitch-class counts, repeat-expanded playback onsets, durations, pitch classes,
MIDI range and a velocity-inclusive event SHA-256. The verifier independently
recalculates these fields from the saved source JSON in full WASM and the
resolved proposal in both full and viewer WASM. The added metadata does not
alter the WAV, MusicXML, source JSON or review-page bytes in the three
development packs.

The page labels the recordings as 案 1–3 and hides their internal profiles until
`案の構成を見る` is opened. Each case has fields for the best and weakest proposal,
whether the differences are audible, and an issue note with the measure and
role. `レビューJSONを表示` creates a copyable schema-2 record with the source
fingerprint, exact candidate IDs and SHA-256 of the source and candidate WAVs.
The hashes bind a judgment to the actual audio if an algorithm changes while
the source and candidate IDs stay the same. No response is sent anywhere
automatically.
Save a submitted review JSON outside the generated pack, then bind it to the
actual audio:

```sh
node scripts/check-arrangement-listening-review.mjs target/arrangement-listening-v1 review.json
```

The checker verifies case order,
source fingerprints, candidate IDs and every source/proposal WAV SHA-256. It
reports the number of selected best/weakest pairs and any `no` or `unsure`
distinctness answers. A `yes` answer needs both selections; `no` and `unsure`
need an explanatory note. A passing hash check does not judge musical quality.

## Current development-pack verification (2026-11-14)

The regenerated pack has four source cases, twelve proposals, sixteen WAVs and
sixteen MusicXML files. Eight proposals contain countermelody attacks, two per
case. Every WAV is mono 22,050 Hz, source and proposals have identical frame
counts within a case, and no sample reaches the 16-bit rail. The CLI validates
all sixteen MusicXML files. Source IDs are fixed by case and position;
generated part IDs in exported scores are fixed by case and proposal. Two
independent schema-2 pack builds produced 38 byte-identical files. Its WAV,
MusicXML and source-score JSON files match the earlier schema-1 pack byte for
byte; only the manifest and review page gained audio hashes. All twelve
resolved candidates replay from their saved source JSON in
both full and viewer WASM, with the same playback events as the exported
MusicXML. The full Rust test suite, workspace Clippy with
warnings denied, and Clippy for the listening-pack example pass. Full and viewer
`wasm-pack build --dev` packages expose the proposal, replay and region APIs;
both packages execute a three-proposal/replay/region smoke flow in Node from
the same parsed MusicXML source. This proves the generated WASM packages run,
and Chrome displayed PASS for the same flow in both package profiles. The
listening page loaded all four cases and sixteen audio controls in Chrome; its
first source WAV played and the playback position advanced to five seconds.

The full and viewer `wasm-pack test --headless --chrome crates/wasm` suites
both passed in the current checkout, including the proposal replay and region
test. The first sandboxed invocation stopped with `Operation not permitted`
before test results; the permitted rerun completed successfully. None of these
mechanical or browser checks supplies a listening judgment.

## Eight-measure development phrase

`tests/fixtures/arrangement/phrase-development-v1.json` adds a separate
eight-measure development melody without changing the fixed eight-case corpus
or its holdout. It authors 2+2 harmony groups in 4/4 and phrase endings in
measures 4 and 8. Countermelody is inactive in measures 1–2 and 5–6, then
thins at the two endings. This fixture opts into restarting the motif when
the countermelody enters in measures 3 and 7; the fixed shorter development
pack and saved contracts without the option retain their earlier playback.
The core regression checks those sections, five
different harmony-onset sequences, phrase-end attack reduction, source
preservation, proposal replay and a locked regional insertion. Without
the authored grouping, narrow-register alternatives still return five
complete proposals after open harmony and approaching bass are rejected.

Generate and serve its listening pack separately:

```sh
cargo run -p acorde-cli --example arrangement_listening_pack -- target/arrangement-listening-longform-v1 longform
python3 -m http.server 8766 --bind 127.0.0.1 --directory target/arrangement-listening-longform-v1
```

Open `http://127.0.0.1:8766/` and compare the source with five proposals.
Two proposals have a countermelody and three do not; the set includes five
different profiles across three harmonic plans. The third uses metered
harmony with a countermelody, and the fifth uses offbeat harmony. The fifth
and fourth have identical per-measure attack counts but different onsets.
The six WAVs have identical duration, 22,050 Hz mono
format and no sample-rail hits; all six MusicXML scores validate. The five
saved contracts replay in full and viewer WASM with the same playback events
as their MusicXML exports. Recheck this contract with:

```sh
wasm-pack build crates/wasm --target web --dev --out-dir ../../target/wasm-profiles/full-arrangement-dynamics
wasm-pack build crates/wasm --target web --dev --out-dir ../../target/wasm-profiles/viewer-arrangement-dynamics --no-default-features --features viewer
node scripts/check-arrangement-listening-pack.mjs target/arrangement-listening-longform-v1 target/wasm-profiles/full-arrangement-dynamics target/wasm-profiles/viewer-arrangement-dynamics
```

Two independent schema-2 builds produced fifteen byte-identical files. The
four-case development pack was also reproducible across two builds, with its
WAVs, MusicXML scores and source JSON unchanged from schema 1. In the
eight-measure pack, only the two countermelody proposals changed their audio
and MusicXML after the opt-in section-entry restart; the source and other
three proposals retained their previous exports. The manifest and review JSON
now include WAV SHA-256 values, so earlier judgments cannot be mistaken for
judgments of changed audio. The full Rust suite, workspace Clippy, example
Clippy and full/viewer Chrome WASM suites passed after these changes. Chrome
loaded the five-proposal page, played the second proposal WAV and displayed
schema-2 review JSON. Phrase quality still requires listening judgment; this
pack remains development material.

## Eight-measure compound-meter development phrase

`tests/fixtures/arrangement/phrase-compound-development-v1.json` is a separate
6/8 development melody with six eighth notes per measure and authored 3+3
groups. It does not change the fixed eight-case corpus or open its holdout.
Unlike the 4/4 phrase, the normal countermelody has only one sounding group per
measure, so restricting a phrase ending to the first group would not thin it.
This fixture instead authors a one-measure answer in measures 3 and 7 and rests
that role in measures 4 and 8. The section-entry option restarts the answer at
measure 7. The phrase-end positions still shape the harmony.

Generate and serve its separate pack with:

```sh
cargo run -p acorde-cli --example arrangement_listening_pack -- target/arrangement-listening-compound-v1 longform-compound
python3 -m http.server 8767 --bind 127.0.0.1 --directory target/arrangement-listening-compound-v1
```

At `http://127.0.0.1:8767/`, compare the source with five proposals. The set
contains five profiles across three harmonic plans, including two with a
countermelody. The core regression checks the source part, exact replay, the
two one-measure answers and five distinct profiles. Full and viewer WASM
replayed all five saved contracts with the same playback events as the
MusicXML exports. Two independent pack builds produced fifteen byte-identical
files; regenerating the earlier short and 4/4 longform packs left their files
byte-identical. All six MusicXML files validate. The six WAVs are mono at
22,050 Hz, each 275,625 frames long, with a peak sample magnitude of at most
7,291 out of 32,767. Chrome loaded the page, started proposal 2 playback and
displayed schema-2 review JSON. These checks establish a reproducible
comparison; the compound-meter accent and phrase response still need a
listening judgment.

## Signal-level balance check

`scripts/check-arrangement-listening-pack.mjs` also checks the WAV format,
matching frame counts and sample rails. With the same renderer gain and no
normalization, the full arrangement RMS is 1.71–2.80 times the source RMS in
the short development pack, 2.23–3.03 times in the 4/4 eight-measure pack,
and 1.81–2.35 times in the 6/8 eight-measure pack. The added notes naturally
increase total signal energy. These ratios flag volume balance for listening
review; they do not prove melody masking or perceived loudness. Keep the
original gain when comparing and record any specific measure where the melody
becomes hard to follow.
The ensemble API now accepts optional sustained levels for harmony, bass and
countermelody. None is selected in these canonical packs: regenerating all
three packs after the API addition produced byte-identical files, and current
full/viewer WASM builds replay their saved contracts. Choose any role-level
change from the listening review rather than treating an RMS ratio as the
correct velocity setting.
