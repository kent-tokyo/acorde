# acorde-cli

Command-line conversion and inspection tool for acorde.

~~~bash
cargo install acorde-cli

acorde convert input.mid output.musicxml
acorde convert input.musicxml output.abc
acorde convert input.musicxml output.mei
acorde info input.musicxml
acorde validate input.musicxml
acorde report input.mei
acorde preflight input.musicxml
acorde preflight input.musicxml --fail-on-issues
acorde analyze input.musicxml
acorde benchmark benchmarks/analysis.json --fail-on-mismatch
acorde extract --part 0 input.musicxml part.musicxml
acorde transpose --semitones 2 input.musicxml transposed.musicxml
acorde normalize input.musicxml normalized.musicxml
acorde export-report input.musicxml exported.musicxml
acorde export-report input.musicxml exported.abc
acorde export-report input.musicxml exported.mei
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
MusicXML, MIDI, ABC, or MEI. info prints title, counts, tempo, time signature, and duration estimate; validate
exits with status 1 when structural errors are found. report emits the parsed score and structured
import diagnostics as JSON.
preflight emits renderer capability issues with stable score source locations as JSON before SVG
generation.
`--fail-on-issues` keeps the JSON output but exits with status 1 when any issue is found, which is
useful for CI gates.
The repository fixture `tests/fixtures/render_preflight_unsupported.musicxml` demonstrates the
failure path without external resources.
analyze emits deterministic chord, melodic-interval, and key-estimate results as JSON.
export-report writes MusicXML, MIDI, ABC, or MEI and emits machine-readable export diagnostics
without embedding the binary/text artifact in the JSON response.
compatibility-report parses two local score files and emits their deterministic positional semantic
diff plus source-located import diagnostics from both sides. It reports differences and does not
claim lossless interchange.
`--fail-on-differences` preserves the JSON report and exits with status 1 when semantic changes
are found, making the command suitable for a compatibility gate.
`--fail-on-loss` independently fails when either side reports a typed information-loss diagnostic.
The JSON includes explicit `semantic_equivalent` and `lossless` booleans so consumers do not need
to infer gate status from counts.
It also includes `analysis_changed_categories`, covering deterministic chord, interval, key,
cadence, voice-leading, SATB, motif, and phrase-boundary changes.
`--fail-on-differences` fails when either the score diff or the analysis category diff is non-empty;
`analysis_equivalent` exposes that second decision explicitly.
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
