# Acorde roadmap

This roadmap tracks intended work, not released capabilities or delivery dates. The
[scorecard](docs/scorecard.md) describes implemented contracts, the
[notation coverage matrix](docs/notation-coverage.md) describes tested format support, and the
[changelog](CHANGELOG.md) records releases. Issue acceptance criteria remain authoritative for
each item below.

## Current baseline

Version 1.3.9 provides a Rust score model with undoable commands, bounded format I/O, SVG and
print-page rendering, playback scheduling, analysis primitives, a file-based CLI, and WASM
bindings. Import and export reports identify known losses; they do not guarantee that every
real-world score round-trips without change. The library crates remain synchronous and do not
own host file access or UI.

## Priorities

1. **Preserve score meaning across formats and edits.** Keep global tempo through MSCZ export
   ([#379](https://github.com/kent-tokyo/acorde/issues/379)), incoming partial ties through
   alternate endings ([#282](https://github.com/kent-tokyo/acorde/issues/282)), and positioned
   figured bass ([#276](https://github.com/kent-tokyo/acorde/issues/276)). Check playback and
   re-imported score semantics on licensed real-score fixtures as well as minimal examples.
2. **Make ordinary printed scores dependable.** Address same-onset voice alignment
   ([#299](https://github.com/kent-tokyo/acorde/issues/299)), slur placement around other
   voices ([#296](https://github.com/kent-tokyo/acorde/issues/296)), and hairpin geometry
   ([#295](https://github.com/kent-tokyo/acorde/issues/295)). Complete page-SVG/PDF review
   against a fixed Verovio corpus ([#183](https://github.com/kent-tokyo/acorde/issues/183));
   compare missing marks, collisions, legibility, and clipping separately from page count.
3. **Complete reviewable accordion arrangements.** Continue physical button and harmony
   planning ([#358](https://github.com/kent-tokyo/acorde/issues/358)), melody-preserving
   reduction on real scores ([#359](https://github.com/kent-tokyo/acorde/issues/359)),
   transposition with instrument constraints
   ([#377](https://github.com/kent-tokyo/acorde/issues/377)), and profiled accompaniment
   patterns ([#378](https://github.com/kent-tokyo/acorde/issues/378)). Report source-note
   omissions, generated harmony, button actions, and performer-review status explicitly.
4. **Strengthen playback and audio evidence.** Extend the same-score SF2 comparison with
   alphaTab ([#203](https://github.com/kent-tokyo/acorde/issues/203)). Keep timing,
   synthesis defects, format losses, and listening results as separate observations.

## MCP integration (planned)

Expose existing Acorde capabilities to MCP clients through a separate host adapter. The
adapter should depend on the public Rust crates or CLI rather than add protocol or file-access
responsibilities to the score model or WASM library.

1. **Local, read-only server.** Implemented as `acorde mcp --root <dir>`; unreleased.
   Offer stdio tools for score inspection, structural validation,
   import/compatibility diagnostics, and an SVG preview. Define bounded inputs and structured
   outputs, preserve source addresses and diagnostics, and constrain file access to explicitly
   configured roots. Verify the chosen MCP SDK against Rust 1.89 and test tool discovery and
   calls with an MCP client.
2. **Reviewed editing.** Implemented as MCP sessions with `preview_edit`/`apply_edit`,
   undo/redo and `--allow-write` exports for ten `Command` types; unreleased. Expose selected existing `Command` operations only after the server
   can show a proposed diff, validate the result, and retain undo/redo semantics. An output
   write needs an explicit destination and must not silently overwrite the source. Reuse the
   existing import/export loss reports when converting edited scores.
3. **Remote access only if needed.** Specify authentication, per-request authorization, and
   resource limits before adding an HTTP transport. Local stdio support alone is not a claim
   that Acorde is available as a hosted MCP service.

MCP support is complete when a client can discover and invoke the documented tools on a pinned
licensed score, receive the same results and diagnostics as the underlying Acorde APIs, and
reject invalid or out-of-scope requests without changing a score or file. Editing requires
additional diff, validation, undo/redo, and write-boundary checks.

## Keeping this roadmap current

Update an item's issue link or remove it when its acceptance criteria are verified. Record
implemented behavior in the scorecard and coverage matrix, release changes in the changelog,
and measured comparisons in their evidence documents. Do not turn a fixed-fixture result into
a general compatibility, engraving, or audio-quality claim.
