# Interchange evidence contract

This project treats format support as a semantic contract, not as a claim of byte-for-byte
conversion. The current fixture inventory is [tests/fixtures/manifest.json](../tests/fixtures/manifest.json).
Every fixture records its format, provenance/license, SHA-256, evidence mode, and expected losses.
The integration suite verifies the manifest checksums against compile-time embedded fixture bytes,
so the evidence check does not read from the filesystem or fetch data during tests.

## Evidence modes

- `semantic`: parse → canonical score JSON → serialize → parse, followed by field-level comparison.
- `import-only`: the parser and diagnostics are tested; no exporter is claimed for that format.
- `decode-render`: a provider/codec boundary is tested with bounded decoded data and deterministic
  rendering; this is not score interchange evidence.

Semantic identity is distinct from source-byte identity. Normalization of ordering, whitespace,
quantization, or container metadata is acceptable only when the report identifies the boundary.
An unsupported value must be rejected or reported with a source location, preserved value where
available, and a loss reason. A successful parse alone does not promote a matrix cell to `yes`.
MusicXML `<unpitched>` notes retain display placement and the canonical `Note.is_unpitched` flag,
then re-emit as `<unpitched>`; sound identity resolves only through explicit instrument
declarations or retained MIDI display keys, while unmatched notes remain source-located losses.
The MSCX tablature boundary imports ordered `<Fingering>` values into canonical
`Note.fingerings` and synchronizes the legacy first candidate in `Note.fingering`, with
MusicXML/MSCX semantic comparison coverage; alternate-fingering policy and renderer glyph
fidelity remain outside the local gate.
MusicXML note-level `instrument@id` is preserved as optional `Note.instrument_id` and re-emitted;
the ID is not guessed into a GM/percussion sound, so sound-catalog mapping remains a declared loss
boundary.
Declared MusicXML `score-instrument` entries and explicit `midi-unpitched` keys are also retained
on the owning Part and serialized back into `part-list`; absent declarations are not inferred.
MIDI channel 10 notes are marked `Note.is_unpitched` while retaining their MIDI key as display
placement; because MIDI does not provide a MusicXML-style instrument ID in this path, the parser
leaves `Note.instrument_id` empty.
The MSCX report also identifies top-level `Staff` elements that are not referenced by a `Part`,
including their source path and ID, so incomplete part ownership is not reported as lossless.
MuseScore 4.x Parts whose child Staff elements use only `eid` retain their child count, and
same-part staves are emitted and reparsed through explicit MusicXML note `staff` tags.
MuseScore `Part`-local `bracket` and `barLineSpan` metadata is also mapped to bounded
`Part.staff_groups` connector ranges; missing or invalid ownership is not inferred.
MusicXML figured-bass figure number, prefix, suffix, and raw alteration values are retained in
structured `Measure.figured_bass` entries and survive parse/serialize/parse; the same entries
regenerate typed display text deterministically. Empty figured-bass elements remain diagnosed.
MEI simple `fb`/`f` values populate ordered `Measure.figured_bass` entries and regenerate typed
`TextStyle::FiguredBass`; richer MEI and vendor-specific figure semantics remain source-located
diagnostics. Standard `time-modification` tuplets are
preserved as `TupletInfo`.
ABC reports identify unsupported header fields and decoration delimiters by line; the body parser
continues to use the declared note/rest/chord subset.
 MEI measure tempo (`mm`), plain-text `harm` chord labels, `reh` rehearsal marks, and `dir` directions/navigation marks are preserved in the supported subset. Unattached `harm` remains a measure-level `StyledText` display label; a bounded set of attached labels using `startid` becomes structured `ChordSymbol` data on the addressed note. Attached `chordref` is retained as raw canonical metadata; unattached `chordref`, timing-only placement, and unparseable labels receive source-located diagnostics. MEI `octave` spans are now supported for note-addressed `dis`/`dis.place` ranges; same-measure/layer note-addressed piano `pedal dir="down"` spans using `startid`/`endid` are also supported. Timestamp, release-only, and half-pedal variants remain explicitly reported as unsupported, with available `dir`, ID, and timing attributes preserved in the diagnostic. MEI export reports identify unrepresentable canonical score fields by score path, so an empty
import report cannot be mistaken for a lossless export contract.
Attached MEI `harm@place` is retained as ChordSymbol placement; timing-only placement remains
diagnosed because it has no addressed note in the canonical model. Unmodeled `harm` attributes
such as `rendgrid` are source-located diagnostics; attached `harm@extender` is retained as a
canonical ChordSymbol continuation flag, and attached `harm@deg` is retained as harmonic-analysis
text. Unattached `harm@deg` remains diagnosed rather than being guessed as a chord extension.
Attached MEI `harm@func` and MusicXML `<function>` are retained as the canonical
`harmony_function` token and compare equivalently. The standard
MEI generic `harm@type` classification is retained as optional canonical metadata;
unattached MEI `harm@func` and empty MEI `harm` elements are source-located rather than
silently discarded.
The same non-silent-loss rule applies to empty MSCX `harmonyInfo/function` values.
Vendor-specific MSCX harmony children remain explicitly outside the canonical subset; MuseScore
upstream [Issue #20996](https://github.com/musescore/MuseScore/issues/20996) records an
`Harmony/extension` case whose value semantics require a pinned mapping before promotion.
MusicXML and ABC export report its loss with a source path and preserved value.
The self-authored `tests/fixtures/interchange_harm_analysis.mei` fixture pins this combined
`harm@deg`/`harm@extender`/`harm@type`/`harm@chordref` contract with a semantic round-trip and
manifest checksum. These optional fields are serde-defaulted for legacy JSON. When exporting to
MusicXML or ABC, the unsupported MEI-only fields produce source-located loss diagnostics with
their preserved values rather than being silently discarded.
Attached `harm@chordref` references are retained as raw URIs. The bounded `chordDef`/`chordTable`/
`chordMember` slice now preserves IDs, labels/types, fret position, source tuning spelling,
member IDs, pitch (including the bounded MEI quarter-tone accidental subset), string/course, fret,
fingering, and bounded barre range data; richer harmony and unmodeled
barre attributes remain outside the canonical model and are source-located when encountered.
MusicXML and ABC export report each retained chord definition as a source-path loss because those
formats have no equivalent reusable chord-definition container.
Invalid chord-definition positions or member values, and barre references that do not resolve to
member IDs, are retained only as diagnostics with their source path; they are never silently
coerced into a valid fingering.
Unknown attributes on the bounded chord-definition elements are likewise reported individually
with the original attribute value.
`chordMember` and `barre` elements outside a `chordDef` are diagnosed as orphaned rather than
silently discarded.
Duplicate `xml:id`/`id` values across the bounded chord-definition/member slice are also diagnosed
with the repeated value and source attribute path, because barre and `harm@chordref` references
require globally unique identifiers.
The same uniqueness check covers note and other ID-bearing elements used by `startid`/`endid`
references, so ambiguous span or harmony targets are reported before semantic comparison.
Local `harm@chordref` fragments are checked against the bounded `chordDef` ID set; unresolved
fragments are diagnosed with their source attribute and preserved value, while external URIs are
retained without attempting network or catalog resolution.
MEI simple figured-bass `fb` figure text is preserved as typed display-level
`TextStyle::FiguredBass`; ordered `<f>` values also populate `Measure.figured_bass` and
round-trip through the MEI serializer. The source figure text remains lossless in the figure
fields. The cross-format regression normalizes MEI accidental fields and MusicXML prefix fields
to the same alteration semantics before comparing the projections. MusicXML fractional `alter`
values are source-diagnosed when integer-semitone plus cent decomposition cannot preserve them
exactly, and MEI export diagnoses note pitch combinations outside its supported accidental
subset. These checks protect the display/playback boundary without claiming backend audio
equivalence. MSCX accidental subtypes outside the supported quarter-tone subset are likewise
source-located and preserved in diagnostics instead of being silently normalized to natural.
Empty MusicXML harmony functions are also source-located as malformed rather than retained as an
empty canonical token.
field, standard leading accidental glyphs map to the structured `alter` field, common `|`/`+`
decorations and balanced parentheses map to structured prefix/suffix fields, and MEI
`f@extender` is preserved as a boolean figure property. Unknown figure text remains intact in
the number field. Attachment meaning remains outside the bounded canonical subset and is not
presented as semantic equivalence.
Unsupported `<f>` children/attributes and empty figure values are source-located diagnostics
rather than silent flattening.

MSCX `FiguredBassItem/digit` values are imported in source order into `Measure.figured_bass` and
regenerated as typed display text; the checked-in `interchange_figured_bass.mscx` fixture covers
this path. Native `continuationLine` is retained as the canonical figured-bass extender flag;
parenthesis, timing, and other engraving properties are intentionally outside this bounded
projection and produce a source-located loss diagnostic when encountered.
Known native prefix/suffix modifier values are retained as semantic names and rendered with a
deterministic visible accidental marker.
Invalid MSCX figure digits and modifier values are likewise rejected from the silent path and
reported with their preserved source value.
MIDI import/export preserves Controller Change, multiple Program Change, and key/channel
Aftertouch events with canonical 480-PPQ tick and channel data. Tempo and time-signature changes at canonical measure boundaries are
preserved as measure metadata; off-boundary changes receive a diagnostic. MIDI export reports fractional pitches that are rounded to note keys; exact cents are not
claimed as audio equivalence unless an explicit pitch-bend stream is present.
MusicXML export cannot carry the canonical MIDI pitch-bend event stream, so
`serialize_musicxml_with_report` emits one source-located
`musicxml.export-unsupported-midi-pitch-bend` loss diagnostic per event, preserving its
canonical tick, channel, and signed value in the diagnostic. This is an explicit format
boundary and does not perform timing or pitch transformation.
The checked-in public-domain MIDI corpus now includes the original perfect-fifth fixture plus
Wikimedia Commons interval fixtures covering signed negative (`-1850`) and positive (`1437`)
nonzero pitch-bend values; each is checksum-pinned and round-tripped by semantic event value.
This three-file public-domain set is a permitted smoke corpus, not a held-out population benchmark.
MusicXML export reports tablature capo because the canonical field is not part of the standard
staff-details representation. MEI imports and exports simple note-addressed octave spans using
`octave @dis`/`@dis.place` and same-measure/layer note-addressed pedal spans using `@dir="down"`, `@startid`, and `@endid`; timestamp and release-only pedal forms remain outside the current subset and retain their source timing attributes in diagnostics. MusicXML preserves `bend-alter` as canonical bend cents; non-start slide,
hammer-on, or pull-off details remain diagnosed because the current canonical technique model
stores the technique kind but not its endpoint direction.
ABC export reports omitted non-primary voices/staves, tablature staff/string/fret/technique
fields, and unsupported fractional cents rather than claiming that its compact notation is a
complete Score serialization.
MSCX simple `Harmony/name` values are preserved as measure-level chord-symbol display labels.
The bounded `harmonyInfo/root` plus common `name` subset also attaches a canonical `ChordSymbol`
to the following chord, and `harmonyInfo/base` maps to its slash-chord bass; placement is retained
in the canonical model. `harmonyInfo/function` is retained as the same canonical
`harmony_function` field used by MEI and MusicXML. MusicXML degree value/alter/type and MEI attached compact harm suffixes are
represented by `ChordDegree`; complex MSCX/MEI degree structures remain source-located diagnostics.
Unknown MSCX `Harmony` and `harmonyInfo` attributes are also reported with their XML path and
preserved value rather than being silently discarded.
MSCX compact `Harmony/name` suffixes in the unambiguous `add`, `alter`, and `no` forms are also
split into canonical `ChordDegree` values; unknown suffixes remain verbatim rather than guessed.
Out-of-range root/base TPC values (outside the bounded MuseScore `6..=26` spelling range) are
rejected from the canonical mapping and retained in a source-located diagnostic, preventing
unbounded accidental-name allocation.
The local semantic suite also compares the resulting chord-label projection between MEI and MSCX;
this is display-label equivalence, not structured harmonic analysis equivalence.

The next compatibility phases use this baseline in order: MEI subset expansion, MSCZ/MSCX
structure and notation, MIDI event semantics, microtonal notation/playback separation, tablature
instrument semantics, and finally cross-format held-out equivalence.
The OpenScore Lieder repository is a promising external MSCX corpus candidate: its repository
states that the scores are CC0 and distributes `.mscx` files. A specific release, selected files,
and checksums must still be pinned before adding it to the fixture manifest.
The pinned fixture `openscore_lieder_aloha_oe.mscx` for
`Liliuokalani,_Queen_of_the_Hawaiian_Islands/_/Aloha_Oe/lc6650166.mscx`
from the repository (MuseScore 3.6.1, 207,715 bytes, SHA-256
`f588fc91a36629d6aa079721571ce6e2afb8c8dfafa77935c8198b2d3ec5fd51`) parsed as 4 parts and 23
measures with an empty import diagnostic list. It is an import-only external smoke gate; the
separate MuseScore 4.x compressed-MSCZ pair is covered below.
As a second public CC0 screening pass, the pinned OpenScore StringQuartets tree at commit
`4e3240eb2109a3fd74dbbdca0a1688f5375c2903` was inspected. Its README states that the scores are
CC0 and uses uncompressed `.mscx`; a representative file declares MuseScore 3.6.2. This
provides another 3.x-format reference, but it does not satisfy the missing MuseScore 4.x or
compressed `.mscz` held-out gate, so no additional large fixture is added solely from this scan.
The MuseScore OMR benchmark is a separate CC0-1.0 source and distributes real MSCZ files. The
small `mscz/score_file_1003.mscz` item at Hugging Face revision
`e27f6a8634e80ad0997af8a806c8dc00e45c4a07` is pinned as
`openscore_omr_score_1003.mscz` (SHA-256
`77ec1090af66b21a726e29d9e36e119d4a4ef112d25a4f758b5d00e8bd865711`). It parses with zero
diagnostics into one part and four measures, providing a real compressed-MSCZ smoke gate; a
broader held-out corpus comparison remains outstanding. A second pinned item,
`score_file_1033.mscz` (SHA-256
`5c863d3b3dd1d055aeb74e3dabe3343090277578c4b9dd14d94c988b8b1a6687`), also declares MuseScore
4.6.3 and parses with zero diagnostics. Together they provide the declared two-file MuseScore
4.x compressed-MSCZ smoke pair; corpus-wide held-out coverage remains separate. The second file
contains ten structured `Harmony` elements, which are reported as the declared unsupported
display-label boundary rather than being silently discarded. Three additional MuseScore 4.6.3
CC0 MSCZ samples were measured with zero diagnostics and add 1-part/14-measure, 4-part/72-measure,
and 1-part/22-measure structural coverage. The five-file set is still a smoke corpus, not a
corpus-wide accuracy claim.
The machine-readable report records the measured diagnostic/part/measure counts for all five
MSCZ samples and the three public-domain MIDI pitch-bend fixtures; these are parser observations,
not quality or compatibility scores.
The integration test sample_measurements_match_current_parser_output re-parses every listed
MSCZ and MIDI fixture and checks those recorded counts and bend values, preventing stale
measurements from being accepted as release evidence.
Each of the five MSCZ samples also passes the explicit MSCZ → canonical Score → MusicXML → Score
semantic projection for pitch, duration, rest, dot, part/staff/measure/voice identity, and the
bounded ChordSymbol root/kind/bass fields. The
comparison normalizes only exporter-added trailing rests used to complete an incomplete measure;
internal rests remain compared.
The MIDI fixture suite also verifies that the MIDI → canonical Score → MusicXML boundary reports
every pitch-bend event as an explicit loss instead of silently dropping it.

Canonical percussion resolution is now explicit: an unpitched note resolves to a declared
`PercussionInstrument` by note-level `instrument_id` first, then by its retained MIDI display key.
If neither matches, the API returns no instrument rather than guessing a sound identity.

MEI nested `staffGrp` containers are retained as `Part.staff_groups` ranges with their connector
symbol and barline-through setting. The outer `staffGrp` remains an MEI structural container and
is not incorrectly exposed as an inter-part `PartGroup`.

MSCX and MusicXML preserve multiple `Fingering` elements per note as ordered
`Note.fingerings` candidates, while mirroring the first candidate in legacy `Note.fingering`.
The core also exposes a non-mutating deterministic selection policy (source order, lowest, or
highest candidate). The SVG renderer gives multiple string/fret positions deterministic horizontal
breathing room; external font metrics and renderer glyph fidelity remain outside the local gate.

## Phase 6 release-gate interpretation

The local 6A–6G gates are complete for the documented slices: fixtures, semantic projections,
deterministic reports, malformed-input checks, and source-located loss diagnostics are checked
in and test-validated. This does not promote partial format cells to complete interchange.
`docs/interchange-report.json` also records a machine-checked BUILD/MEASURE/GATE contract for
each phase; the integration test rejects a phase whose three evidence descriptions are absent.
The WASM Node runner compiles successfully but skips the browser-only tests; the Chrome runner
currently cannot produce completion evidence because this environment has no ChromeDriver.
Independent held-out corpus comparison and host/backend audio equivalence remain external gates;
their absence is recorded in `docs/interchange-report.json` and must remain visible in release
notes until separately measured. The same report explicitly lists complex MEI/MSCX harmony,
permissioned tablature corpus coverage, and external engraving glyph metrics as pending gates.
The two embedded MSCX payloads were also checked in separately with their source archive paths
and SHA-256 values, and direct MSCX parsing reproduces the same zero/declared-diagnostic boundary.

## Deferred transformation review

Issue #18's beat-aware MIDI quantization is intentionally not implemented. It would transform
note timing using beat/downbeat anchors, and publicly available patent material describes related
MIDI quantization and beat-position correction workflows. This project does not treat a web patent
listing as a legal opinion or a clearance result. Until jurisdiction, claim scope, and legal status
are reviewed by qualified counsel, the safe local boundary is raw timing preservation plus typed
diagnostics; no beat-aware timing transformation is advertised or exposed.

### Issue #18 patent-risk gate (2026-09-04)

The issue's proposed transformation is not low-risk enough to implement without counsel review.
JP7683706B2 is displayed by Google Patents as an active Japanese grant and describes MIDI timing
modification workflows that include beat-position correction and MIDI quantization. The older
US5734118A and US7589271B2 records are displayed as expired, but their disclosed/claimed MIDI
quantization or MIDI-to-notation timing concepts still demonstrate that the surrounding field is
crowded; an expired US record does not clear other jurisdictions or later patent families.
This is a risk screen, not an infringement or validity opinion. Accordingly, Issue #18's
beat-aware timing transform is rejected/deferred, while raw tick preservation, provenance, and
source-located diagnostics remain permitted local work. Reconsideration requires a jurisdiction-
and-claim-specific review by qualified patent counsel.
