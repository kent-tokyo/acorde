# Local transcription prototype

This prototype runs Basic Pitch or an optional piano transcription model locally. Its browser
workspace loads a WAV, plays selected ranges slowly, and lets the user review and edit note
candidates. It exports MIDI and MusicXML through Acorde.

For a Python 3.11 environment and an Acorde CLI example built from this checkout:

```sh
python3.11 -m venv .venv-transcription
.venv-transcription/bin/pip install 'basic-pitch==0.4.0' onnxruntime 'setuptools<81'
cargo build -p acorde-cli --bin acorde --example transcription_score
.venv-transcription/bin/python examples/transcription/server.py
```

Open the printed `127.0.0.1` address. Choose a WAV, set a start and end time, and create
candidates for that range. Creating candidates replaces notes whose starts and MIDI pitches
fall in the selected ranges and preserves notes outside them. The models receive up to two seconds of context on each side, and only
notes whose estimated attacks fall in the selected range are added. Their estimated releases can
extend beyond that range. A detected note reaching the end of the context is flagged for review;
longer held notes still require checking. Attacks in the 50 ms before the range appear as separate
review candidates; they are not added automatically. A nearby existing note is flagged, and
adding a candidate can be undone. This window does not catch every boundary error. Restrict the
pitch range when
investigating a melody or bass line. Select notes in the piano roll and edit their fields, or add
and remove notes. The range buttons assign all notes whose starts and pitches fall in the selected
ranges to one hand. The initial split at MIDI note 60 is a guess. Chord candidates are pitch-class
matches on left-hand notes whose starts fall within the chosen time window. Groups containing
just one pitch class are shown as bass candidates. Octaves are not distinguished in this list.
Selecting a note also shows overlapping notes one or two octaves away that start within 120 ms;
select them to compare. This is a review hint, not a determination that they are duplicate keys.
Chord and bass candidates whose pitch classes are already sounding are marked for reattack
review. A repeated key or button can produce the same overlap, so the mark does not remove
notes or decide whether an additional attack occurred.
The candidate list can use all notes to investigate chords spanning the initial hand split.
This also includes right-hand melody notes and does not assign a candidate to a physical hand.
Notes from different hands may be combined into one group, so a bass or chord candidate
can disappear when this option is selected. Compare both source settings with the recording.

Set the playback range and speed to compare the original with the candidate notes. Enter the
tempo as a whole-number BPM, meter and time of the first complete bar before export. For a pickup, enter its length in
sixteenth notes. The pickup begins before that first bar, and notes before the pickup begins
must be removed or the bar position changed. MusicXML marks the shortened first bar as implicit.
MIDI represents it using a short initial time signature followed by the chosen meter. The editor
rejects export and preview when overlapping notes of the same pitch are assigned to the same
hand, including overlap introduced by MIDI tick rounding. Adjust their durations or hand assignments
before exporting; otherwise MIDI note-off messages cannot unambiguously represent them. The editor
retains recording times. Acorde's MIDI importer quantizes the MusicXML output, so compare the
output with the original and correct notation errors. The importer supports four voices per
staff and may shorten or merge overlapping notes when that limit is exceeded. MusicXML uses one part with treble and
bass staves when both hands have notes. The MIDI export uses separate channels for the two hands.
The grand-staff MusicXML carries one part-level MIDI channel, so downstream MIDI playback of
that file can merge overlapping equal pitches across the staves. Generate the score preview and choose a measure to
set the corresponding recording loop. After changing notes or score settings, regenerate the
preview before selecting a measure.

The editor can save its note data and start-boundary review candidates as JSON with a SHA-256 hash of the original recording when
available. After restoring a session, selecting the same WAV verifies that hash. A different
file is reported without changing the saved fingerprint; boundary candidates are cleared when
the selected file cannot be matched to the saved hash. Analyzing the different file alongside
existing notes clears the fingerprint because the session no longer has a single known source;
undo restores it. Older sessions without a hash remain usable and are marked as unverified.
The WAV stays in the browser and is temporarily written on the local machine for inference.
The server accepts files up to 64 MiB and listens on localhost.

This is an experimental tool. `evaluation.md` records initial piano and accordion observations.
Accordion button and register inference, paired register performances, and a participant timing
comparison remain unverified. `listening-study.md` describes a later listening session.

## Optional piano model

For the optional Transkun piano model, install `transkun==2.0.1` in a Python environment
with compatible PyTorch and torchaudio versions. Launch this server with
`--transkun /path/to/that/environment/bin/transkun` and choose Transkun in the page.
Each request starts a local CPU inference process. The uploaded WAV is converted to 16-bit
PCM for this model and inference receives the same range context as the other models.
Results and model provenance are recorded in `evaluation.md`.

Install `piano-transcription-inference==0.0.6` and `torch` in the same environment. Download the
[official model checkpoint](https://zenodo.org/record/4034264) and pass its local path with
`--piano-model /path/to/checkpoint.pth`. Select the piano model in the page. The server requires
the checkpoint to be present before loading it. Model downloads, licenses, and version choices
should be reviewed for the intended distribution.

## Checks

The Python tests cover MIDI timing, hand assignment, repeated notes, grand-staff output, a
pickup that crosses the first full bar, and evaluation alignment. `evaluate.py` accepts
`--estimate-offset`, `--start` and `--end` in recording seconds. Build the converter example first, then run:

```sh
ACORDE_TRANSCRIPTION_CONVERTER=target/debug/examples/transcription_score \
  .venv-transcription/bin/python -m unittest discover -s examples/transcription -p 'test_*.py'
```

Set `ACORDE_TEST_BIN=target/debug/acorde` to also check playback timing for the pickup. With
Playwright installed separately and the server running, `node examples/transcription/smoke.cjs`
checks session restore, assignment, chord candidates, MusicXML, range replacement, undo, slow
playback settings and pickup export. Set `CHROME` to a browser executable if required.

The evaluator also lists unmatched reference notes, unmatched estimated notes, and offset
errors for onset-matched notes. Unmatched estimates carry proximity flags for a reference
onset an octave away and for a nearby onset-matched estimate of the same pitch. These flags
identify notes to inspect and do not establish whether a note is a harmonic or duplicate.
Interpret offset errors only when reference note releases have been independently validated.
For a MusicXML export, save `acorde playback-report score.musicxml` as JSON and pass that
file as the estimate with `--estimate-playback-report`. Use `--estimate-offset` to align
its time zero with the original recording, just as for an exported MIDI.

For a fixed collection of aligned MIDI files, `compare_corpus.py manifest.json` reports
per-excerpt and note-weighted aggregate onset matches, false positives, missed notes,
precision, recall and F1 for two models. A manifest is a JSON array, with paths relative to
the manifest and optional hashes to verify the input files:

```json
[
  {
    "id": "excerpt-a",
    "reference": "reference.mid",
    "baseline": "baseline.mid",
    "candidate": "candidate.mid",
    "start": 0,
    "end": 30,
    "sha256": {"reference": "replace-with-reference-sha256"}
  }
]
```

Omit `sha256` when hashes are not available. Use only aligned files with a common time
origin and record the selection, split, model version and audio provenance separately.
An optional `audio` path adds the recording's SHA-256 to the report and can be included in
the `sha256` checks. The WAV is not read by the transcription models in this comparison.
The report includes per-excerpt recall changes so that aggregate results do not hide a
recording where the candidate model misses more notes.

`maps-comparison.json` fixes the three MAPS excerpts and their MIDI hashes described in
`evaluation.md`. The referenced recordings and model outputs are external files. Place
them in a `maps` directory beside the manifest before running the comparison. The MAPS
reference MIDI has a high tick resolution that exceeds `pretty_midi`'s event limit; create
each `.scaled.mid` with `python rescale_reference.py source.mid output.scaled.mid`. This
rounds absolute positions to 960 ticks per beat. The manifest checks the resulting hash.
`idmt-room8-comparison.json` similarly fixes four additional IDMT-PIANO-MM room8
performances and includes the source audio hashes. Its external files belong in a `room8`
directory beside the manifest. See `evaluation.md` for the conditions and split limitations.

For independently annotated accordion performances, `evaluate_accordion.py reference.json
estimate.json` evaluates right-hand key onsets, bass-button onsets and chord-button onsets
separately. In the browser, assign notes to the intended hands and select the chord grouping
window and source before using “候補を評価用に保存” to create the estimate file. Its bass and chord
entries are the same pitch-class candidates shown in the left-hand list, including repeated
and ambiguous candidates. Chord onset is the first estimated onset in each group. A
seventh candidate with an omitted fifth is exported as `seventh`. Assignments, grouped
onsets and candidates require checking against the recording. Choosing all notes also exports
possible left-hand candidates involving right-hand notes, and the original right-hand notes
remain in the right-hand array. Each JSON file uses recording
seconds and must contain all three arrays:

```json
{
  "right": [{"time": 1.2, "pitch": 64}],
  "bass": [{"time": 1.2, "root": 7}],
  "chord": [{"time": 1.7, "root": 7, "quality": "major"}]
}
```

Right-hand pitches are MIDI numbers and bass/chord roots are pitch classes with C = 0.
Chord qualities are `major`, `minor`, `seventh` and `diminished`. Matching requires the
same pitch or root and chord quality, with one-to-one onset matching within 100 ms by
default. `--tolerance` changes this window. Reference and estimate must be independently
aligned to the same recording; the evaluator does not infer buttons or hands from audio.
