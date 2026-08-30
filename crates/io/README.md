# acorde-io

In-memory MusicXML, MIDI, ABC, and MuseScore I/O for acorde-core.

Default features are musicxml and midi. Optional features:

| Feature | Provides |
|---|---|
| abc | parse_abc and serialize_abc |
| mscz | parse_mscz and parse_mscx |

The default API includes parse_musicxml, parse_mxl, serialize_musicxml, parse_midi,
serialize_midi, and serialize_midi_region. Parsers accept strings or byte slices and never touch
the filesystem.

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
