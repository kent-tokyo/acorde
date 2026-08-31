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
~~~

Input supports .musicxml, .mxl, .mid/.midi, .abc, .mei, .mscz, and .mscx. Conversion output is
MusicXML or MIDI. info prints title, counts, tempo, time signature, and duration estimate; validate
exits with status 1 when structural errors are found. report emits the parsed score and structured
import diagnostics as JSON.
analyze emits deterministic chord, melodic-interval, and key-estimate results as JSON.
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
