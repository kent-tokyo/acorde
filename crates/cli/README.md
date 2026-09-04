# acorde-cli

Command-line conversion and inspection tool for acorde.

~~~bash
cargo install acorde-cli

acorde convert input.mid output.musicxml
acorde info input.musicxml
acorde validate input.musicxml
acorde report input.mei
acorde analyze input.musicxml
acorde benchmark benchmarks/analysis.json --fail-on-mismatch
acorde extract --part 0 input.musicxml part.musicxml
acorde transpose --semitones 2 input.musicxml transposed.musicxml
acorde normalize input.musicxml normalized.musicxml
acorde export-report input.musicxml exported.musicxml
acorde tab-position guitar.musicxml edited.musicxml --part 0 --measure 0 --note 1 --string 2 --fret 3
acorde tab-position edited.musicxml cleared.musicxml --part 0 --measure 0 --note 1 --clear
acorde auto-tab guitar.musicxml guitar-tabbed.musicxml
acorde auto-tab-report guitar.musicxml guitar-tabbed.musicxml
acorde fingering-report guitar.musicxml --policy source-order
acorde fingering-report guitar.musicxml --policy lowest
~~~

`auto-tab-report` prints JSON containing assigned/remaining notes, chord count, total and maximum
fret, while writing the optimized score to the requested output path.
`fingering-report` prints each authored candidate list and the selected value without modifying the
score. Its deterministic policies are `source-order`, `lowest`, and `highest`.

Input supports .musicxml, .mxl, .mid/.midi, .abc, .mei, .mscz, and .mscx. Conversion output is
MusicXML or MIDI. info prints title, counts, tempo, time signature, and duration estimate; validate
exits with status 1 when structural errors are found. report emits the parsed score and structured
import diagnostics as JSON.
analyze emits deterministic chord, melodic-interval, and key-estimate results as JSON.
export-report writes MusicXML or MIDI and emits machine-readable export diagnostics without
embedding the binary/text artifact in the JSON response.
benchmark reads a local JSON manifest and emits corpus metadata, including a content fingerprint,
plus the deterministic suite report.
Paths are relative to the manifest file. `--fail-on-mismatch` makes the command exit with status 1
when any case has a category mismatch. `--expected-fingerprint` makes the command exit with status
1 when the manifest or referenced fixture bytes differ from a recorded fingerprint. A manifest
includes corpus metadata; each case has `name`,
`input`, `coverage`, `provenance`, and expected category counts:

~~~json
{"cases":[{"name":"sample","input":"../tests/fixtures/simple.musicxml","expected":{"chords":0}}]}
~~~

[Repository](https://github.com/kent-tokyo/acorde)

## Tablature validation

`validate` also checks tablature metadata and explicit string/fret positions. This local-only
check does not require a SoundFont or other external asset:

~~~bash
acorde validate tests/fixtures/guitar.musicxml
~~~

An invalid line count, tuning value, string number, or microtone-cent value exits with status 1
and prints the part/staff location.

CLI input and output boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
