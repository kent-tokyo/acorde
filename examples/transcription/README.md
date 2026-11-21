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
