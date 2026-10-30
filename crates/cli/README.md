# acorde-cli

Command-line conversion and inspection tool for acorde.

~~~bash
cargo install acorde-cli
acorde --help
acorde convert input.mid output.musicxml
acorde validate input.musicxml
acorde render input.musicxml output.svg
acorde render input.mscz output.svg --auto-beams
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
~~~

Use `acorde <command> --help` for the full surface. Commands are grouped as conversion and
inspection (`convert`, `info`, `validate`, `report`, `preflight`), SVG/print rendering, analysis,
score edits (`extract`, `transpose`, `normalize`), tablature, playback, and machine-readable
export/compatibility reports.

Input accepts MusicXML/MXL, MIDI, ABC, MEI, MSCX/MSCZ, and Guitar Pro 3–8. Conversion writes
MusicXML, MIDI, ABC, MEI, and the documented canonical MSCX/MSCZ subset; it does not promise full
MuseScore parity. `render` writes deterministic SVG with optional interactive score-address hooks.
`render --auto-beams` adds meter-aware display beams for unmarked short notes without editing
the input score. The default render keeps authored beam state.
`print-report` returns host-neutral page and system layout JSON, not a PDF or printer job.

Report commands are intended for automation: `render-report` and `preflight` expose renderer
issues; `export-report` exposes typed conversion losses; `compatibility-report` compares two
scores; and `benchmark` checks a local manifest. Their `fnv1a64-*` values identify local input
bytes and are not publication hashes or compatibility scores. `--fail-on-*` keeps JSON output while
making the selected condition a non-zero exit.

`auto-tab-report`, `tab-performance-report`, and `fingering-report` provide deterministic
tablature and fingering diagnostics. `playback-report` produces the expected event schedule, while
`playback-compare` checks a host-observed schedule with explicit tolerances.

`omr` is a review-first, high-contrast PNG subset. It writes MSCZ only for a complete,
high-confidence, diagnostic-free proposal; callers supply the meter and retain source, provider,
consent, and review UI. `omr-evaluate` is a per-pair observation gate, and `omr-apply-report`
revalidates an untrusted report before writing. See the [proposal contract](../../docs/omr-proposal-contract.md)
and [validation boundary](../../docs/omr-validation.md).

[Repository](https://github.com/kent-tokyo/acorde)

`validate` also checks tablature tuning, string/fret positions, and microtone values. CLI input and
output rules are documented in the [security contract](../../docs/security/threat-model.md).
