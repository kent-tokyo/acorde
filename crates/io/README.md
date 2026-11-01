# acorde-io

In-memory interchange for `acorde-core`. Parsers accept text or bytes and serializers return text
or bytes; this crate never opens files. The CLI owns paths and file handles.

| Feature | Default | API boundary |
|---|---:|---|
| `musicxml` | yes | MusicXML import/export and compressed MXL import |
| `midi` | yes | Standard MIDI import/export and region export |
| `abc` | no | Bounded ABC import/export |
| `mei` | no | Bounded MEI import/export |
| `mscz` | no | MSCX/MSCZ canonical-subset import/export |
| `gp` | no | Guitar Pro 3–8 import only |

The default calls are `parse_musicxml`, `parse_mxl`, `serialize_musicxml`, `parse_midi`,
`serialize_midi`, and `serialize_midi_region`. Optional formats follow the same naming pattern.
Use `parse_*_with_report` and `serialize_*_with_report` when a caller must inspect
normalization, unsupported source data, or output loss. An empty report means that API detected no
loss; it is not a whole-format compatibility guarantee.

`serialize_musicxml` writes only authored beam states, so an unmarked voice stays unmarked
through interchange. `serialize_musicxml_with_auto_beams` adds beat-based beams to unmarked
voices for display readers that need explicit groups; use it only when that visual change is
intended.

```rust
use acorde_io::{parse_musicxml, serialize_musicxml};

# fn main() -> Result<(), acorde_io::Error> {
# let xml = "<score-partwise version=\"3.1\"></score-partwise>";
let score = parse_musicxml(xml)?;
let output = serialize_musicxml(&score)?;
# let _ = output;
# Ok(())
# }
```

## Interchange policy

MusicXML/MXL is the broadest path. MIDI projects timed events into a score and is not a notation
round-trip format. ABC, MEI, MSCX/MSCZ, and Guitar Pro use documented subsets. Preserve the source
file and inspect typed reports whenever vendor extensions, exact placement, complex TAB semantics,
or byte identity matter.

Reports carry a schema version, severity, source location when available, preserved value, and loss
reason. Invalid values use a visible diagnostic rather than being silently replaced. Parsers keep
up to four canonical editing voices per staff/measure. MusicXML source voice numbers can exceed
four across a score when each measure fits those slots; a fifth distinct voice in the same
staff/measure is rejected instead of merged.

## MuseScore tie interoperability test

The MSCZ tie-location tests run without external applications. To additionally check a generated
MSCZ by opening it in MuseScore and inspecting its exported MIDI, set `ACORDE_MUSESCORE` to the
MuseScore executable and run:

```sh
ACORDE_MUSESCORE=/path/to/mscore cargo test -p acorde-io --all-features --test mscz_tie_locations musescore_external
```

The external test is skipped when the variable is unset. It validates the produced MIDI even if
MuseScore exits abnormally after writing it; any such exit is reported in the test output.

## Resource and trust boundary

Inputs have finite size and archive budgets. MXL/MSCZ reject traversal-style or duplicate entries
and excessive declared uncompressed size before extraction. MIDI event counts and ABC logical-line
lengths are capped. Hosts with smaller resource budgets should enforce them before calling this
crate.

See the [notation coverage matrix](../../docs/notation-coverage.md) for feature-level support,
the [interchange evidence contract](../../docs/interchange-evidence.md) for the checked scope, and
the [security contract](../../docs/security/threat-model.md) for input handling rules.

[API documentation](https://docs.rs/acorde-io) · [Repository](https://github.com/kent-tokyo/acorde)
