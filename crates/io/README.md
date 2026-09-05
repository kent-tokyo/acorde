# acorde-io

In-memory MusicXML, MIDI, ABC, and MuseScore I/O for acorde-core.

Default features are musicxml and midi. Optional features:

| Feature | Provides |
|---|---|
| abc | parse_abc and serialize_abc |
| mscz | parse_mscz and parse_mscx |
| mei | parse_mei and serialize_mei for the documented subset, including scoreDef/staffDef meter, key, and clef |

The default API includes parse_musicxml, parse_mxl, serialize_musicxml, parse_midi,
serialize_midi, and serialize_midi_region. Parsers accept strings or byte slices and never touch
the filesystem.
`parse_musicxml_with_report` preserves simple `figured-bass` figure-number text as typed
display-level `TextStyle::FiguredBass`; prefixes, suffixes, and alterations are retained in
structured figure fields. MusicXML `<unpitched>` percussion notes retain
their `display-step`/`display-octave` staff position and canonical `Note.is_unpitched` flag, and
are re-emitted as `<unpitched>`; sound identity resolves only from explicit instrument
declarations or retained MIDI display keys, while unmatched identities remain source-located
losses.
Note-level MusicXML `instrument@id` is retained as `Note.instrument_id` and re-emitted, without
guessing a concrete sound from the identifier.
Declared `score-instrument` metadata and explicit `midi-unpitched` keys are retained on the owning
Part and re-emitted; missing sound-catalog data is not inferred.
MIDI channel 10 notes are represented as unpitched notes with their key retained for display; no
instrument ID is fabricated.
Figured-bass prefix/suffix decoration and standard alteration values are preserved in structured
figure fields while deterministic visible display text is regenerated.
Standard `time-modification` tuplets are
preserved as `TupletInfo`.
ABC input/output preserves double accidentals and pure quarter-tone spellings; mixed semitone plus
quarter-tone pitch offsets are reported as export losses.

The MEI boundary supports title, multiple numbered staves with up to four layers per staff, scoreDef/staffDef meter, key,
and clef, notes, rests, dots, accidentals, ties, tuplets (`num`/`numbase`), grace notes (`@grace`), repeat barlines, measure tempo (`mm`), text chord labels (`harm`), rehearsal marks (`reh`), directions/navigation marks (`dir`), common dynamics/lyrics, and
common articulations plus ornaments (staccato, tenuto, accent, marcato, fermata, trill, mordent,
turn, and related forms), mRest/multiRest, with
power-of-two durations. Tuplets are represented as per-note `TupletInfo` values; arbitrary MEI
timing constructs outside this contract remain unsupported.
MEI `harm` text without an attachment is preserved as a measure-level `StyledText` with
`TextStyle::ChordSymbol`. A bounded set of attached text labels (C, Cm, C7, Cmaj7, slash basses,
and related qualities) using `@startid` becomes the note's structured `ChordSymbol`; attached
`@chordref` is retained as a raw URI. The bounded `chordDef`/`chordMember` tab-definition fields
are retained in `Score.chord_definitions`; unresolved reference resolution, timing-only placement,
unmodeled barre attributes, and unparseable labels remain source-located loss diagnostics.
Attached MEI `harm@place` is retained as ChordSymbol placement, attached `harm@extender` is
retained as its continuation flag, and attached `harm@deg`/`harm@func` are retained as
harmonic-analysis metadata;
timing-only placement remains
diagnosed because it has no addressed note in the canonical model. Empty `harm` elements are
also source-located as semantic loss.
Attached MEI compact `harm` suffixes in the unambiguous `add`, `alter`, and `no` forms map to
canonical `ChordDegree` values; complex structures remain diagnosed.
MEI simple `fb`/`f` values are retained in order as `Measure.figured_bass` and also regenerate
typed display-level `TextStyle::FiguredBass`; standard leading accidental glyphs map to the
structured alteration field; common `|`/`+` decorations and balanced parentheses are also
structured. Unknown source text remains intact, and `f@extender` is retained
in the canonical figure data. MusicXML figured-bass figure number/alter/prefix/
suffix values are retained as structured `Measure.figured_bass` data.
Unsupported MEI `<f>` children/attributes and empty figure values produce source-located
diagnostics.
MSCX `FiguredBassItem/digit` values use the same ordered boundary; `continuationLine` is retained
as the canonical figured-bass extender flag, while other engraving properties remain outside the
canonical subset and are source-located by diagnostics.
Known prefix/suffix modifier values are preserved semantically and rendered deterministically.
Invalid digit and modifier values are reported with their original value and field path.
Richer MEI and vendor-specific figure semantics remain outside the canonical model and are
diagnosed.
Same-measure/layer piano pedal spans using
`@dir="down"`, `@startid`, and `@endid` are supported; timestamp and other pedal attachment
variants remain source-located loss diagnostics with their source attributes preserved.
`parse_mei_with_report` reports known unsupported elements and flattened staff/layer numbers
instead of silently claiming lossless interchange.
MEI numeric fallbacks for malformed `measure@n`, meter attributes, `tempo@mm`, and multi-rest
counts are also source-located with their preserved values; callers can distinguish a valid
canonical default from source data that was not representable.

MusicXML notes are mapped from voice numbers 1–4 to the corresponding `Measure.voices` entries;
serialization emits the same voice numbers and `<backup>` boundaries for round-trip fidelity.
MIDI pitch-bend events are preserved on `Part::midi_pitch_bends` with their absolute canonical
480-PPQ tick,
source channel, and signed 14-bit value; Controller Change, Program Change, and key/channel
Aftertouch events are likewise preserved with their tick/channel data and MIDI export emits them
on the preserved channels. MIDI delta values exceeding the SMF 28-bit VLQ limit are rejected
instead of being shortened.
Tempo and time-signature changes at canonical measure boundaries are attached to measure metadata;
off-boundary changes are reported rather than silently shifted.
The checked-in public-domain fixture verifies note pitch/duration/rest semantics and bend event
meaning across parse → serialize → parse.
ABC `V:` fields now select stable numbered parts during import, so the exporter/importer pair
preserves multiple part streams; multiple staves and richer ABC directives remain diagnosed
according to the coverage matrix.
`parse_midi_with_report` preserves Controller Change, Program Change, and Aftertouch events with
absolute tick/channel data; it reports unsupported SysEx and Escape events with
their track/event index, tick, and channel where applicable instead of silently claiming full
interchange. It also reports unmatched note-on/note-off events; overlapping same-pitch notes are
paired FIFO per channel. SMPTE timing is normalized to the current PPQ boundary and reported as a
loss diagnostic. `serialize_midi_with_report` reports fractional-pitch rounding when a host has
not supplied explicit pitch-bend events.
Malformed MusicXML `divisions`, `duration`, and `voice` values are source-located with their
original values instead of being indistinguishably replaced by parser defaults.

MSCX tab staffs preserve `StaffType` line/tuning metadata and note-level `string`/`fret` positions
when supplied by the source. MSCX `Tuplet`/`endTuplet` ranges preserve their `actualNotes` and
`normalNotes` ratio on each covered chord/rest. MSCX Tremolo subtypes map to canonical speed-level
articulations; simple `Harmony/name` values and the bounded `harmonyInfo/root` subset are
preserved as typed display text or canonical `ChordSymbol` data; `harmonyInfo/base` maps to slash
bass where present, while richer structured harmony
fields remain source-located diagnostics. Harmony placement is retained in the canonical
`ChordSymbol` when present; `harmonyInfo/function` maps to `harmony_function`. Harmony root/base TPC values outside the bounded
`6..=26` spelling range are rejected with a source-located diagnostic.
Unambiguous compact `Harmony/name` suffixes (`add`, `alter`, `no`) map to canonical
`ChordDegree` values; unknown suffixes are retained verbatim.
Multiple MSCX and MusicXML `Fingering` elements on one note are retained as ordered
`Note.fingerings` candidates; the first value remains mirrored in the canonical `Note.fingering`
field for compatibility.
Part-local `bracket` and `barLineSpan` metadata is retained as `Part.staff_groups` connector
ranges when the referenced span is valid.
One-note versus
two-note tremolo pairing is not represented by the current model.
ABC `^/` and `_/` use the same 50-cent quarter accidental contract as
MEI `qs`/`qf` and MSCX quarter accidental subtypes.
MusicXML export reports tablature capo as an explicit loss because it has no standard
`staff-details` representation.
`parse_abc_with_report` reports unsupported ABC headers and `!decoration!`/`+decoration+`
constructs with line-based source locations.
`serialize_abc_with_report` reports omitted staves/voices and microtones outside the supported
quarter-tone spelling subset. It also reports ABC's lack of a canonical tablature staff,
string/fret positions, and guitar-specific techniques as source-located losses.
`parse_mscx_with_report` reports known unsupported MuseScore elements such as tremolos,
ottavas, glissandos, and harmony with source paths.

Each supported format also exposes `*_with_report` wrappers returning `ImportReport` or
`ExportReport<T>`. The MEI export report identifies score fields outside its canonical subset
(for example tablature and host-specific playback techniques) instead of silently dropping them.
Reports carry structured diagnostics with severity, source location, preserved
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
