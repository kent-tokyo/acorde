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
acorde mcp --root ./scores
acorde analyze-vertical score.krn --infer-key
~~~

Use `acorde <command> --help` for the full surface. Commands are grouped as conversion and
inspection (`convert`, `info`, `validate`, `report`, `preflight`), SVG/print rendering, analysis,
score edits (`extract`, `transpose`, `normalize`), tablature, playback, and machine-readable
export/compatibility reports.

Input accepts MusicXML/MXL, MIDI, ABC, MEI, MSCX/MSCZ, Guitar Pro 3–8, Humdrum `**kern` (`.krn`),
and Plaine & Easie (`.pae`). Conversion writes
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

`analyze-vertical` emits opt-in simultaneous-note chord candidates with written
measure/beat positions and source addresses. `--infer-key` uses the first
Aarden-Essen global-key candidate; otherwise written key changes apply.

`auto-tab-report`, `tab-performance-report`, and `fingering-report` provide deterministic
tablature and fingering diagnostics. `playback-report` produces the expected event schedule, while
`playback-compare` checks a host-observed schedule with explicit tolerances.

`omr` is a review-first, high-contrast PNG subset. It writes MSCZ only for a complete,
high-confidence, diagnostic-free proposal; callers supply the meter and retain source, provider,
consent, and review UI. `omr-evaluate` is a per-pair observation gate, and `omr-apply-report`
revalidates an untrusted report before writing. See the [proposal contract](../../docs/omr-proposal-contract.md)
and [validation boundary](../../docs/omr-validation.md).

`mcp` serves score tools to a local MCP client over stdio. Inspection tools are `score_info`,
`validate_score`, `import_report` (diagnostics and renderer issues, not the score),
`compatibility_report`, and `render_svg_preview` (SVG returned inline). Each `--root` is a
directory the tools may use; paths are resolved relative to the first root, symlinks are followed
before the root check, and inputs over 64 MiB or results over 8 MiB are rejected. Domain failures
return MCP tool errors. Stdout carries only protocol messages.

Editing is reviewed and in memory. `open_session` imports a file into an undoable session;
`preview_edit` returns the semantic diff and validation of one command and a `proposal_id`
without changing the session; `apply_edit` commits only that latest, still-current, valid
proposal; `undo_edit`, `redo_edit`, and `session_diff` (cumulative diff from the opened file)
follow the engine history. Exposed commands are `set_metadata`, `set_tempo`, `set_tempo_at_measure`, `set_key_signature`,
`set_time_signature`, `set_part_name`, `set_dynamic`, `transpose_staff_region`, `add_measure`, and
`delete_measure`, in the serde shape of `acorde_core::Command`. `write_score` exports to an
explicit destination and returns the same loss report as `export-report`; it is disabled unless
the server is started with `--allow-write`, never writes over the session's source, replaces
another existing file only with `overwrite: true`, and refuses scores with validation errors.
`set_tempo` changes the default tempo, which a first-measure tempo mark overrides; MusicXML export
then writes only that mark and reports `musicxml.export-shadowed-default-tempo`, so use
`set_tempo_at_measure` to change a marked tempo. Check the returned loss report for other limits. A client configuration looks like:

~~~json
{ "mcpServers": { "acorde": { "command": "acorde", "args": ["mcp", "--root", "/path/to/scores", "--allow-write"] } } }
~~~

The server does not listen on a network port.

[Repository](https://github.com/kent-tokyo/acorde)

`validate` also checks tablature tuning, string/fret positions, and microtone values. CLI input and
output rules are documented in the [security contract](../../docs/security/threat-model.md).
