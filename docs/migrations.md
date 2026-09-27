# Migration notes

This page summarizes compatibility-relevant changes from older releases. Detailed release history
is kept in [`CHANGELOG.md`](../CHANGELOG.md); current APIs and feature boundaries are documented
in the crate READMEs and the [notation coverage matrix](notation-coverage.md).

## v0.3–v0.5 — browser contract and hardening

The `Score → LayoutResult → SVG` pipeline and stable `part:staff:measure:voice:note` addresses
remain the browser integration boundary. Versioned render metadata, reviewed browser baselines,
accessibility text, and WASM size/resource checks were added. Hosts continue to own selection,
hover, playback, DOM insertion, and CSS sizing.

## v0.6–v0.7 — parser and validation correctness

MusicXML first-measure attributes are applied correctly. Structural validation now reports empty
scores, missing staves/measures, mismatched staff measure counts, and invalid time signatures.
Consumers that exhaustively match validation errors should handle the added variants.

## v0.8–v0.9 — portable score patches

WASM score patch/apply APIs were added. Patches can represent measure metadata, barlines, rehearsal
marks, volta brackets, and explicit note insertion positions. Unsafe or structurally broad changes
fall back to `ReplaceScore`, preserving target-score data instead of silently dropping it.

## Current release

### Unreleased — compound dynamics, held dynamics in playback, lyric extenders

`Dynamic` gains seven variants (`Fp`, `Sfp`, `Sfpp`, `Pf`, `Sffz`, `Sfzp`, `N`) and now derives
`Copy` and `Eq`; exhaustive matches on it need arms for them. `Lyric` gains `extend: bool`
(serde-defaulted, omitted when false), so struct literals must add the field. Playback and MIDI
export change behaviour: an unmarked note now plays at the staff's last dynamic (see
`DynamicTimeline`) rather than velocity 64, so hosts that compare velocities should refresh
their expectations; notes before any marking still play at 64.

MusicXML import now stores `PercussionInstrument::midi_unpitched` as the 0-based General MIDI
key (the MusicXML value minus one) and export adds one back; scores imported by earlier
versions hold the raw MusicXML value, one higher. Unpitched notes with a resolvable kit
instrument now play that key (`Part::percussion_key`) instead of their display pitch.

`OttavaKind::musicxml_type` now returns MusicXML's meaning (`down` for 8va/15ma, `up` for
8vb/15mb). Scores imported from MusicXML by earlier versions have 8va and 8vb swapped in
`Note::ottava_start`; re-import them. `Note::pitches` under an ottava are the sounding pitch;
SVG output draws them shifted by the ottava.

### v1.2.11 — mid-bar clef changes

`Measure` gains `mid_clefs: Vec<MidMeasureClef>` (a `MeasureLength` offset from the bar's start
and a `Clef`), serde-defaulted and omitted from JSON when empty, so existing score JSON is
unchanged. Code that builds `Measure` with a struct literal must add the field (or start from
`Measure::empty`). `Measure::clef_at` gives the clef in effect at a beat, and
`MeasureLength::from_beats` converts a beat offset. Validation reports `InvalidMidMeasureClef`
for a change outside the bar or out of order. New `Command::SetMidMeasureClefs`,
`ScoreChange::MidMeasureClefsChanged` and `ScorePatch::SetMeasureMidClefs` variants cover
editing, diffing and patching; exhaustive matches on those enums need an arm. Split and join
move mid-bar changes with their notes.

MusicXML, MEI and MSCX import keep a clef read partway through a bar at that point (it used to
move to the next barline), and a clef read after a bar's last note begins the next bar. Their
exports write mid-bar changes before the note they precede. MusicXML import no longer restates
the first staff's clef in every bar that has an `<attributes>` block, MusicXML export now
writes bar clef changes on a part's second and later staves (they were dropped), and MSCX uses
MuseScore's `C3`/`C4` names for the alto and tenor clefs.

### v1.2.10 — staff-relative SVG engraving and clef changes

No Score JSON or Rust API changes. SVG output changes: beams now connect to stems; lyrics,
dynamics, and chord symbols use staff-relative baselines; articulations and ornaments use engraved
glyphs and staff-aware placement; tremolos, lyric hyphens, and flags have new geometry. Hosts that
compare SVG snapshots should refresh them. MusicXML clef changes on later staves now import at the
correct measure, C clef line 4 imports as tenor, and a wholly other-staff voice is no longer
mistaken for cross-staff notation. MEI `<tie>` control events can now preserve a tie on one pitch
of a chord.

### v1.2.9 — per-pitch chord ties and Guitar Pro 3/4/5

`Note` gains `pitch_tie_starts` and `pitch_tie_ends`, parallel to `pitches`. They are empty
unless a chord ties only some of its notes, and are omitted from JSON when empty, so existing
score JSON is unchanged. `tie_start`/`tie_end` stay the chord-level summary ("any pitch"). Read
per-pitch ties with `Note::pitch_tie_start(i)`/`pitch_tie_end(i)` and write them with
`Note::set_pitch_ties`; a vector whose length no longer matches `pitches` (for example after
code that pushes a pitch directly) is ignored in favour of the chord-level flag. MusicXML, MEI,
MSCX and Guitar Pro import keep per-note ties, MusicXML, MEI and MSCX export write them per note, playback
sustains only the tied pitches, and the SVG renderer draws one tie per tied notehead. The
`musicxml.partial-chord-tie` and `gp.partial-chord-tie` diagnostics introduced in 1.2.8 are no
longer emitted. `parse_gp` also reads `.gp3`, `.gp4` and `.gp5` files.

### v1.2.9 — MusicXML note positions and notehead size

MusicXML import no longer fills `Note::offset_x`/`offset_y` from a note's `default-x`/
`default-y`; those are absolute positions in the source engraver's layout, and the renderer
applies `offset_*` as nudges from its own position. Hosts that set `offset_*` keep their
meaning; MusicXML export writes them (added to `relative_*`) as `relative-x`/`relative-y`. SVG
noteheads now have standard (SMuFL) proportions, so rendered widths, spacing and golden SVGs
change; hosts comparing SVG snapshots should refresh them.

### v1.2.8 — Guitar Pro import and tablature rendering

New optional `gp` feature; no existing API changes. The GM range table only widens, so no
previously valid score becomes invalid. MusicXML export now writes `<slide>` in `<notations>`
(older acorde files with it in `<technical>` still import), and it follows cue notes with a
`<backup>`. MEI tablature `tab.course` and `<course n>` now count from the highest string, as MEI
defines: files written by acorde 1.2.7 have their tab strings reversed. SVG tablature headers
change: they draw a TAB clef and no key signature.

### Unreleased — MEI parity with Verovio

No Rust or JSON API changes. MEI output changes shape: every part is written, articulations
are written as `@artic`, ornaments and marks are written as measure-level control events,
barlines are written on `measure@left`/`@right`, and the key is written as `@keysig`. Consumers
that string-match acorde's MEI should parse it as MEI instead. The importer still accepts the
older acorde forms: layer-level `<artic>`/`<ornam>`/`<dynam>`, `<barLine>` inside `<staff>`,
`@key.sig`, and staff-level `@meter.count`. A layer `<mRest/>` now imports as one whole rest,
the canonical measure rest, instead of an empty measure with `multi_rest_count = 1`. An MEI file
whose first `scoreDef` labels its staves now imports as several parts. MuseScore files whose
staves start in a non-treble clef now import with that clef. MusicXML export changes
element order to match the schema, adds `<beam>` and `<tuplet>` elements, and writes part ids
that are not XML names as `P-<id>`. MusicXML import now fills `Note.beam` from `<beam>`, so
imported notes can carry explicit `BeamState` values where they were `None` before.

### Unreleased — measure rests and string techniques

`Articulation` gains `UpBow`, `DownBow`, `Harmonic`, `OpenString`, `Stopped`, and
`SnapPizzicato`; code that exhaustively matches `Articulation` must handle them, and SVG metadata
consumers should accept the new articulation names. A voice containing only one plain whole rest
now lasts its whole measure in every time signature, so code that sums note beats to measure a
voice should call `voice_duration_beats`. `Measure.measure_repeat` is an optional, serde-defaulted
field; exhaustive matches must handle `ValidationError::InvalidMeasureRepeat` and
`ValidationWarning::MeasureRepeatContentDiffers`, and Rust struct literals of `Measure` must add the
field.

### Unreleased — measure lengths and lyric verses

`Measure.actual_length` and `Note.additional_lyrics` are optional, serde-defaulted fields: older
JSON loads unchanged and omits them on save when empty. Rust code that builds `Measure` or `Note`
with struct literals must add the fields (or use `..Measure::empty(..)` / `..Note::new(..)`).
Consumers that exhaustively match `ValidationError` must handle `InvalidMeasureLength` and
`InvalidLyricVerse`. Code that derives a measure's length from its time signature should call
`Measure::duration_beats`, which honors a pickup or irregular length. `SetLyricCmd` gains an
optional `verse`; omitting it keeps the verse-1 behavior. MusicXML measures whose content ends
before the time signature now import at their authored length instead of being padded with rests.

### v1.2.0 — SoundFont materialized decoding

`SoundFontPresetZone` is now constructed through `SoundFontPresetZone::new` or
`SoundFontPresetZone::with_channel_layout`; direct Rust struct literals are no longer supported.
Use `with_channel_layout` when a provider has resolved mono, linked-stereo, or interleaved-stereo
source PCM. `ResolvedPresetZoneMetadata` serializes the added `channel_layout` and
`decode_channels` fields. JSON consumers must ignore unknown additive fields until they opt into
the new audio setup path.

For SF3, use `ResolvedPresetZoneMetadata::decode_sample_region` or
`decode_materialized_sample_region` for a materialized zone. These select the Ogg logical stream
by materialized sample ID and use stream-relative frame/loop coordinates. The older
`decode_sample_region` remains available for its legacy first-stream convenience contract.

For v1.1.3, use the typed `Command::SetMeasureText` command for undoable measure-level
`StyledText` editing. Check the render metadata `contract_version` before consuming newer fields;
v1.1.3 exposes position-aware `text_annotations` in SVG metadata. Contract version 3 adds
optional placement and source-coordinate offset fields; older consumers can ignore these fields.

For v1.1.5, `compatibility_report` is exposed through WASM and the browser
adapter. It compares canonical score JSON values and returns explicit score and deterministic
analysis gate booleans; it does not replace format-specific import/export diagnostics.

For v1.1.7, MSCX import reports malformed numeric fallbacks with stable,
source-located diagnostic codes. `Pitch::to_scientific_name()` preserves extended accidental
spellings and scientific-name parsing rejects accidental overflow. The fuzz lockfile is aligned
with the 1.1.7 workspace crates; consumers should use the report APIs and pinned lockfile when
building reproducible interchange checks.

The same candidate adds `Score.spanners`: typed, stable-ID ranges for slur, glissando, trill line,
pedal, and ottava. Missing `spanners` deserializes as an empty list. Existing note-level flags
(`slur_start`, `pedal_end`, and related fields) remain accepted as legacy input and continue to
produce legacy layout spans; new editing and MusicXML work should use typed spanners. A typed
endpoint takes precedence over a matching legacy endpoint during MusicXML serialization so a
consumer does not emit two copies of the same notation.

Structural commands keep typed endpoints attached to their original notes: insertion and
container edits rebase positional `NoteAddr` values, while deleting or wholesale-replacing an
endpoint removes the complete typed span when its original note cannot be resolved. This avoids
silently retargeting a span; legacy note-level flags and their serialized `NoteAddr` shape remain
compatible.
