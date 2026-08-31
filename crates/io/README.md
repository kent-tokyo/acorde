# acorde-io

In-memory MusicXML, MIDI, ABC, and MuseScore I/O for acorde-core.

Default features are musicxml and midi. Optional features:

| Feature | Provides |
|---|---|
| abc | parse_abc and serialize_abc |
| mscz | parse_mscz and parse_mscx |
| mei | parse_mei and serialize_mei for the documented subset |

The default API includes parse_musicxml, parse_mxl, serialize_musicxml, parse_midi,
serialize_midi, and serialize_midi_region. Parsers accept strings or byte slices and never touch
the filesystem.

Each supported format also exposes `*_with_report` wrappers returning `ImportReport` or
`ExportReport<T>`. Reports carry structured diagnostics with severity, source location, preserved
value, and loss reason fields; an empty diagnostic list means no loss was detected by that API.
Serialized reports include `schema_version` (currently `1`); report readers should check this
field before relying on future diagnostic fields. Reports written by older versions default to
the current schema version when deserialized.

Parser and archive trust-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
Non-archive parser inputs use a 64 MiB baseline rejection limit; callers should enforce tighter
budgets when their host resource envelope requires it.
MIDI decoding also caps events at 500,000, and ABC token scanning caps each logical line at 1 MiB.
MXL and MSCZ archives also reject a total declared uncompressed size above the archive budget
before selecting or reading an entry, and reject duplicate or traversal-style entry names.

~~~rust
use acorde_io::{parse_musicxml, serialize_musicxml};

# fn main() -> Result<(), acorde_io::Error> {
# let xml = "<score-partwise version=\"3.1\"></score-partwise>";
let score = parse_musicxml(xml)?;
let output = serialize_musicxml(&score)?;
# let _ = output;
# Ok(())
# }
~~~

[API documentation](https://docs.rs/acorde-io) · [Repository](https://github.com/kent-tokyo/acorde)
