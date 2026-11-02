# Archived changelog

These release notes are preserved from the [main changelog](../../CHANGELOG.md).

## [1.2.20] - 2026-10-28

- **MSCZ archive index.** `serialize_mscz` now writes the standard
  `META-INF/container.xml` index alongside `score.mscx`, so consumers can pass the generated
  archive directly to MuseScore-compatible readers without reconstructing the ZIP.
- **ABC key signatures.** Parsing now applies a declared key signature when converting unmarked
  notes to pitches, while explicit accidentals retain their written meaning.
- **Documentation.** Consolidated interoperability and capability evidence, and refreshed the
  renderer benchmark baseline on the release candidate.

## [1.2.19] - 2026-10-28

- Added a finite, positive `SvgRenderOptions::staff_gap` in staff spaces. The default remains 10,
  so hosts can tighten a multi-staff system without resizing notation.
- Consolidated public documentation and release/migration entry points without changing format or
  host-boundary guarantees.

## [1.2.18] - 2026-10-25

- **Longa duration.** `Duration::Longa` represents four whole notes exactly. MusicXML `long`,
  MEI `@dur="long"`, and MuseScore `longa` now round-trip instead of being normalized to a
  breve; playback, commands, layout, and SVG retain its 16-quarter-note length.

- **ABC explicit beaming.** Whitespace now closes an ABC 2.1 explicit beam group: adjacent
  eighth notes and shorter durations render as one group, while spaces, rests, longer notes,
  overlays and barlines separate them. This is preserved through the browser SVG API as well.

- **PDF export.** `acorde-pdf` now requires Harumi 1.23.7 and passes bounded SVG directly to its
  native converter. Engraving Bézier curves, elliptical arcs, and notehead ellipses therefore
  remain vector geometry instead of being flattened to polygons before PDF generation.

- **MSRV.** The workspace MSRV is now Rust 1.88, matching the optional Harumi-backed
  `acorde-pdf` crate. CI checks the full all-feature workspace at that toolchain.

- **Browser playback transport.** `PlaybackOptions::tempo_multiplier` lets hosts change speed
  without erasing written tempo changes or ramps; invalid multipliers safely normalize to `1.0`.
  `bpm_override` now consistently suppresses the written tempo map, as its contract states.
  `compute_playback_position` uses the same ramp integration as emitted events, so a browser
  measure/beat cursor cannot drift from its schedule. The dependency-free browser fixture adds
  Rewind, seek, a visible measure/beat cursor, an explicit inclusive measure-range loop, and a
  repeat/rest/meter-change transport fixture with a bounded host scheduling trace, audible
  count-in, and focus-safe Space-key Play/Pause controls.

- **Review-first OMR hand-off.** The optional `acorde-omr` crate accepts a bounded,
  provider-neutral `RecognitionReport`, validates an explicit caller-supplied meter, produces a
  reviewable command plan, and only then applies it through `ScoreEngine` before serializing
  MSCZ. Malformed, non-ready, or diagnostic reports never produce an MSCZ file.

- **Archive safety.** MusicXML/MXL, MSCZ, and Guitar Pro archive parsing now uses `zip` 9.1 and
  rejects invalid archive entry names with a typed error. The checked lockfile no longer resolves
  the advisories affecting `zip` 2.4.

- **Release verification.** CI now checks the complete all-feature workspace at Rust 1.88, and
  release packaging/publishing includes `acorde-omr` and `acorde-pdf` in dependency order.
  Archive creation is checked before publication; registry verification runs once each internal
  dependency has been published.

## [1.2.17] - 2026-09-28

- **Endings for several passes.** `VoltaBracket` gains `numbers` (with `passes`, `plays_on`,
  `label` and `parse_passes`) for an ending played on several passes (`1, 2.`, `1.–3.`), and
  playback repeats the section until the last ending's pass — `|: A |1, 2. B :|3. C` plays A B
  A B A C (it played A B A and stopped: the second pass found no ending). MusicXML `<ending
  number="1, 2">`, MEI `<ending label="1., 2.">`, MuseScore `<endings>1,2</endings>`, ABC `[1,2`
  and Guitar Pro multi-pass endings (reported as lost in 4 alphaTab files) all keep every pass.

- **MuseScore multi-measure rests.** MuseScore draws multi-measure rests from the
  `createMultiMeasureRests` style rather than per bar, so they were lost on import and reported
  as lost on export. Import now groups runs of bars empty in every staff (broken, as in
  MuseScore, by signatures, tempo, rehearsal marks, texts, special barlines and repeats) when
  the style is on, and export turns it on when the score has multi-measure rests.

- **Fingerings of chord notes.** `Note::pitch_fingerings` keeps which chord note a fingering
  belongs to. MusicXML wrote every fingering of a chord on its first note; MuseScore import kept
  only the last note's fingering and export repeated the chord's fingerings in every note, as
  bare digits MuseScore does not read (it expects `<Fingering><text>`). Each note now carries
  its own. Guitar Pro thumb fingering (0) and fingerings on chord strings import instead of
  being reported as lost.

- **Tapping and vibrato.** `Articulation` gains `Tap`, `LeftHandTap` and `Vibrato`: Guitar Pro
  tapping (GP7 `Tapped`/`LeftHandTapped`, GP3–5 beat tapping) and note/beat vibrato — reported
  as lost in 15 and 11 alphaTab test files — now import, draw in SVG ("T", a circled "T", a
  short wavy line) and round-trip through MusicXML (`<tap>`, `<tap hand="left">`,
  `<other-technical>vibrato</other-technical>`) and MEI (`@artic="tap"`). Slaps and pops
  are still reported.
- **Scoops, plops, doits and fall-offs.** `Articulation` gains `Scoop`, `Plop`, `Doit` and
  `Falloff`, read and written as MusicXML `<scoop/>`/`<plop/>`/`<doit/>`/`<falloff/>`, MEI
  `@artic` `scoop`/`plop`/`doit`/`fall` and MuseScore `<ChordLine>` (all three dropped them
  before), drawn at the notehead in SVG, and imported from Guitar Pro slides into and out of a
  note (GP3–7), which were reported as lost in 14 alphaTab test files.
- **Guitar Pro 7 wah marks.** Beat `<Wah>` open and closed import as the "o" and "+" marks
  (`OpenString`, `Stopped`), as MuseScore's guitar palette writes them; they were reported as
  lost.

- **Chord labels in other spellings.** MEI `<harm>` labels and Guitar Pro chord names are parsed
  from text; `C+`, `DMaj7`, `D7+`, `B♭6`, `F♯ø7`, `E°7`, `A-7`, `GΔ`, `Dsus` and grouped
  alterations (`G7(b9, #11)`) were not recognised and stayed plain text (MEI) or were lost (GP).
  They now become chord symbols: 51 of the 57 labels in the chord-symbol test file converted
  by Verovio import as symbols (the rest are omissions and named chords such as `CN6`).

- **MEI tempo words and words in `<dynam>`.** `<tempo>` kept its metronome value but dropped
  its text (Allegro, Andante con moto), and a `<dynam>` holding words rather than a dynamic
  (Verovio writes MusicXML's cresc., dim. there) was dropped whole — about half of the texts
  in Verovio-converted corpus scores. Tempo words become the bar's tempo text and are written
  back (with `@staff`/`@tstamp`, which Verovio draws); dynam words become expression text.

- **MuseScore rehearsal marks, repeat navigation and breath marks.** MSCX import ignored
  `<RehearsalMark>`, `<Marker>` (segno, coda, fine, to coda), `<Jump>` (D.C./D.S. al Fine/al
  Coda) and `<Breath>` (breath marks, caesuras), and export reported rehearsal marks and
  navigation as not written and dropped breath marks silently. All four now read and write,
  so a D.S. al Coda plays back after a MuseScore round trip. The staff-transposition loss
  report is gone too (MSCX has written the part's transposition since the last release).
- **MuseScore trill lines, expression and system text.** `<Spanner type="Trill">` (the "tr"
  with its wavy line) was dropped on import and reported as lost on export; it now maps to the
  trill line and trill mark both ways. MuseScore 4's `<Expression>` (dolce, espressivo…) and
  `<SystemText>` were ignored; they import as expression and generic text. A bar's expression
  text is written as expression-style text instead of being reported as lost. Glissandos
  (`<Spanner type="Glissando">` in the notes they join) were reported as unsupported on both
  import and export; they now round-trip.
- **MSCX bar texts reach MuseScore.** Texts without an offset were written as `<Text>`, which
  MuseScore only reads inside frames, so staff and expression texts vanished when the file was
  opened there; they are written as `<StaffText>` now.

- **Breves and 128th notes.** `Duration` stopped at whole notes and 64ths: breves (MusicXML
  `breve`, MEI `dur="breve"`, MuseScore `breve`, GP7 `DoubleWhole`) became whole notes and
  128ths became 64ths, so the bars that held them over- or under-filled and played out of time.
  `Duration` gains `Breve` and `HundredTwentyEighth`, read and written by every format,
  drawn in SVG (breve bars and rest, five flags and beams); still shorter values take the
  128th. One more corpus score now validates (204 of 205).

## [1.2.16] - 2026-09-28

- **Ghost notes (parenthesized noteheads).** `Note::pitch_parentheses` marks noteheads drawn
  in parentheses, per chord member. Read and written as MusicXML `<notehead parentheses="yes">`,
  MEI `@enclose="paren"` and MuseScore `<ghost>1</ghost>`; imported from Guitar Pro ghost notes
  (GP7 `<AntiAccent>`, which was dropped silently, and GP3–5 note flags, which were reported
  as lost — ghost notes in 16 alphaTab test files); drawn with parentheses in SVG.

- **Guitar Pro chord names.** Chords attached to beats (GP7/GP6 `DiagramCollection` items,
  GP3–5 beat chords) were dropped whole; their names now become chord symbols (root, kind,
  degrees and slash bass, parsed like MEI chord labels) in 16 of alphaTab's test files. The
  fret-grid diagrams are still reported as not imported.

- **Transposing instruments across formats.** MusicXML `<transpose>` kept only `<chromatic>`,
  so a guitar or double bass (`<octave-change>-1`) and a bass clarinet played an octave high;
  the octave now counts, a `number` applies it to one staff (it was always the first), and
  export writes `<diatonic>` and `<octave-change>`. MEI now reads and writes `@trans.semi`/
  `@trans.diat`, and MuseScore files read and write `transposeChromatic`: MuseScore stores
  concert pitch, so a clarinet part was imported at concert pitch and exported with its written
  notes as if they sounded; written pitches now come from `tpc2` and concert pitches are written
  back with both spellings. MuseScore 4 key signatures (`<concertKey>`/`<actualKey>`, instead of
  3.x's `<accidental>`) were read as C major; they are now kept.

- **Octave clefs and C clefs on every line.** `Clef` had only treble, bass, alto, tenor and
  percussion: a tenor voice's treble clef with an 8 below (86 of 205 corpus scores) lost its 8
  and was drawn an octave too high with ledger lines, and soprano, mezzo-soprano and baritone
  clefs became alto. `Clef` gains `Treble8vb`, `Treble8va`, `Bass8vb`, `Bass8va`, `Soprano`,
  `MezzoSoprano` and `Baritone` (with `Clef::from_sign`, `octave_change`, `from_name`), read
  and written as MusicXML `<clef-octave-change>`, MEI `@dis`/`@dis.place` (which Verovio
  renders) and MuseScore `G8vb`/`C1`/`F_B`…, imported from ABC `soprano`/`mezzosoprano`/
  `baritone` and from a Guitar Pro 7 bar's clef `<Ottavia>`. The SVG renderer draws the small 8
  and places notes and key signatures on the new clefs. Pitches stay at sounding pitch.

- **Cautionary, parenthesized and editorial accidentals.** Accidentals the source asks to show
  where the key and bar would not (MusicXML `<accidental cautionary|parentheses|bracket|
  editorial="yes">`, MEI `<accid func="caution|edit">`/`@enclose`, MuseScore `<Accidental>`
  `<role>1`/`<bracket>`) were dropped on import — 1,701 of them in 205 corpus scores. They are now
  kept per chord member in `Note::pitch_accidentals`, drawn (parenthesized ones in parentheses)
  and written back to all three formats (MEI as `<accid>` children, which Verovio renders).
  MuseScore has no editorial role, so an editorial accidental reaches MSCX as a cautionary one.

- **MusicXML bars with rounded durations.** A bar whose durations add up differently from its
  meter only through rounding (17 divisions per quarter, sixteenths written as 4 ticks) became
  an irregular bar of the rounded length, so its correctly written notes overfilled it — 353
  validation errors in one corpus score. A bar whose written values fill the meter exactly now
  keeps the meter. Untyped notes shorter than any note value (a hidden run of 1/17-quarter
  notes) import as sixty-fourths instead of quarters. 203 of 205 corpus scores now validate.

- **Guitar Pro ottavas and bar fermatas.** Beat ottavas (GP7 `<Ottavia>`, GP5 beat flags) were
  reported as lost; consecutive beats with the same mark now become one `8va`/`8vb`/`15ma`/`15mb`
  span in each voice (pitches stay at sounding pitch, as with every ottava in the model). GP7 bar
  fermatas, placed by an offset in the bar, land as a fermata on the first voice's note or rest
  sounding at that offset in every staff. Checked against alphaTab's `ottavia.gp`/`ottavia.gp5`.

## [1.2.15] - 2026-09-28

- **MEI import of Verovio-written files.** Checked against 30 corpus scores converted to MEI by
  Verovio: control events placed a little off the beat (`tstamp="1.9167"`, which Verovio
  derives from MusicXML offsets) found no note and were dropped — now a start takes the nearest
  onset and an end (`tstamp2`) the last note begun by then, within the bar and in the meter in
  force there, so kept dynamics rose from 51 % to 91 % and hairpins from 26 to 266 of 276.
  Notes with only `@dur.ppq` (Verovio's hidden playback notes) and values beyond the model
  (`dur="128"`…`"2048"`, breves) failed the whole file; they now import (the value from
  `@dur.ppq`, or the nearest value the model has; MusicXML `128th`… and `breve` likewise).
  An untyped `<dir>` lands on the staff it names (further ones as expression texts), and
  `<tempo>` converts `@mm.unit`/`@mm.dots` beats to quarter-note tempo and goes on its staff.
- **Realize chord symbols** (`RealizeChordSymbolsCmd`, MuseScore's Tools → Realize chord
  symbols): writes one staff's chord symbols out as chords into a voice of any staff, with
  undo. Every MusicXML chord kind and its added, altered and omitted degrees are voiced in
  close position from the root (octave 4, octave 3 on a bass, tenor or alto clef staff) and
  spelled from it (F#m7 is F#–A–C#–E, Bb7 has Ab), a slash bass goes below, and each chord
  lasts until the next symbol, tied across barlines it holds over.
- MusicXML and ABC import no longer overfill a bar when padding it: a remainder shorter than
  a sixty-fourth (left by playback-only notes of odd lengths) was filled with a sixty-fourth
  rest, which made the bar too long and the score invalid, so no edit could be applied. With
  the range change above, 201 of the 205 corpus scores now import editable (198 before).
- **Out-of-range pitches are warnings (user approved).** Validation reported a pitch outside
  the part's instrument range as an error, and every edit command is refused on a score with
  errors — so a part whose MIDI program names another instrument (a viola marked as violin)
  could not be edited from the moment it was imported (4 of the 205 corpus scores). It is now
  `ValidationWarning::OutOfRange`, as MuseScore flags such notes without blocking;
  `ValidationError::OutOfRange` remains in the enum but is no longer produced.
- **Unroll repeats** (`UnrollRepeatsCmd`, MuseScore's Tools → Unroll repeats): writes repeats,
  voltas and D.C./D.S./coda jumps out as bars in playing order, with undo. Repeat barlines,
  voltas and navigation marks go (a final barline stays on the last bar); a jump that changes
  key, meter or clef restates it, and restatements a jump does not need are dropped; typed
  spanners, chord-symbol ranges, object style overrides and MIDI automation follow their bars
  to every place they are played; linked views' break lists are cleared. On the 198 corpus
  scores that validate, the unrolled score plays exactly the notes of the original.
- **MEI per-part meters.** Like keys before, MEI export wrote the first staff's time signature
  for all staves, so a part in another meter (polymeter in early music, a 6/8 line against
  3/4) came back in the wrong one. Staves whose meters differ now get their own
  `<staffDef meter.count meter.unit>`, and import reads a `<staffDef>` meter (attribute or
  `<meterSig>` child) for that staff only.
- **ABC verses.** Every `w:` line continued from where the previous one stopped, so a second
  verse under a music line was sung over the following notes, and `hel-lo` was one syllable.
  A `w:` line now belongs to the music line above it — the first is verse 1, the next verse 2,
  and so on — `-` splits syllables, `_` holds one, `*` skips a note; export writes each verse as
  its own `w:` line.
- **MIDI import transcription.** Import laid a track's notes end to end — a note that began
  while another still sounded (any piano or chordal track) was pushed after it, a note that did
  not fit the bar was moved whole into the next one, and the whole file took the *last* time
  signature and tempo of the conductor track. Now bars follow the meter changes from tick 0,
  starts and ends snap to a sixteenth grid (thirty-second when the track plays finer), notes
  that overlap go to separate voices (up to four, the top line first), a note crossing a
  barline is split and tied, gaps become rests, and tempo and key changes land on the bars
  they start. A MusicXML → MIDI → import round trip of the corpus now plays back 270519 note
  onsets against 270452 in the source (repeats expanded).
- MIDI export sounds a tied note once instead of striking it again at every tied piece.
## [1.2.14] - 2026-09-27

- **MusicXML opening tempo (user approved).** Export wrote the score's default tempo as a
  visible ♩=120 metronome mark in the first bar of every part, even for a score with no tempo
  marking (a Guitar Pro → MusicXML → import round trip tripled the tempo marks). A marked
  opening tempo is now written once, in the first part; an unmarked one only as a bare
  `<sound tempo>` that sets playback speed and draws nothing, which import reads back into
  the score's tempo.
- **Timing after a meter change.** A bar without its own time signature was timed with the
  score's opening meter instead of the last change before it, so after a change from 4/4 to
  3/4 every later bar gained a silent beat in playback, offline render timing, score duration,
  MIDI export and several editing and SVG checks. `Staff::meter_at` and
  `Staff::measure_beats` give the meter in force, and those paths use them.
- **MIDI export timing and key signatures.** Each voice's notes followed straight on from its
  previous notes, so a voice empty in one bar (a second voice that enters later) played its
  later bars early, and tempo and meter events were placed with the opening bar length. Every
  bar now starts at its own tick. Key signatures are written (at the start and at each change)
  and read back.
- **Playback order no longer loops on unpaired repeat marks.** Two repeat ends that each reset
  the other's pass (a first ending that only ends, a later repeat with its own second ending),
  or a coda placed before its D.C., sent `measure_sequence` round the same bars forever:
  playback, MIDI export and every consumer of the sequence grew it until allocation failed
  (4.5 GB for one Mozart quartet movement in the corpus). Each repeat end and each D.C./D.S.
  now jumps back once, with a length backstop.
- **ABC import and export.** Import put every line after the first into the bar the previous
  line had closed (a tune written a line of bars at a time became one bar), read quoted chord
  symbols (`"Am"`) as notes, rejected named voices (`V:T1`), ignored inline `[K:]`/`[M:]` and
  body `K:`/`M:` fields (a later `K:` changed the key of the whole tune), clefs, dynamics,
  hairpins and broken rhythm (`A>B`). All of these are now read — chord symbols, `"^text"`
  annotations, `!p!`…`!sfz!`, `!<(!`/`!<)!`/`!>(!`/`!>)!`, `>`/`<`/`>>`, voice `name=` and
  `clef=`. Export writes every staff as its own voice with its name and clef, chord symbols,
  dynamics, hairpins, text, inline meter and key changes, and lyrics for every voice. In the
  corpus MusicXML → ABC → import round trip, 6766 of 6961 dynamics, 102 chord symbols and
  almost all key and meter changes that were lost now survive.
- ABC: a line opening with `|:` was taken for a header field and dropped; a short first bar
  is a pickup instead of being filled with rests; lengths such as `7/4` read as double-dotted
  values (and any length as the value that fits); `|]` is a final barline both ways; an empty
  bar is written as `X` instead of vanishing into `||`; a repeat starting mid-tune is written in
  the preceding barline; music before the first `V:` stays the first voice. The corpus ABC
  round trip no longer fails on any file (4 did), and files whose barlines change drop from
  173 to 8.
- ABC tempo changes: body `Q:` lines and inline `[Q:]` set the bar's tempo (a later `Q:` no
  longer replaced the tune's opening tempo), in any beat unit (`3/8=40`) and with quoted text;
  export writes each bar's tempo as `[Q:1/4=…]`.
- **ABC voice overlays.** Voices 2–4 of a bar were not exported and `&` was not read; they are
  now written and read as ABC voice overlays. Files whose notes change in the corpus ABC round
  trip drop from 42 to 2.
- **MusicXML double dots and type-less notes.** Import counted `<dot/>` as a flag, so every
  double-dotted note (202 in the corpus) came in single-dotted and short; it now keeps the
  count. A note without `<type>` (the element is optional) was taken as a quarter; its value
  now comes from its `<duration>` and time modification — exactly when possible, otherwise
  the longest value that fits, so playback-only notes of odd lengths no longer overfill the
  bar.
- A grace note inside a tuplet no longer splits the tuplet's bracket in MusicXML, MEI and MSCX
  export (it closed the bracket early and gave the last note a one-note bracket).
- **Hidden notes and rests.** acorde drew everything a score hides: MusicXML
  `print-object="no"` notes and rests (1464 of them in 34 corpus files, among them written-out
  trills and placeholder rests over other voices), the rests it fills `<forward>` gaps with, MEI
  `<space>` and `@visible="false"`, and MuseScore `<visible>0</visible>`; ABC `x`/`X` invisible
  rests were skipped altogether, shifting the rest of the bar. `Note::hidden` now keeps them:
  they take their time and sound, the SVG keeps their group (class `acorde-hidden`,
  `visibility="hidden"`, so an editor can still show them greyed) and leaves them out of
  beams, and every notation format reads and writes them (MusicXML `print-object`, MEI
  `<space>`/`@visible`, MSCX `<visible>`, ABC `x`). `ScoreEngine::set_hidden` / wasm
  `set_hidden` hide or show a note or rest with undo.
## [1.2.13] - 2026-09-27

- **MusicXML barline styles.** MusicXML import read `<repeat>` but not `<bar-style>`, so final,
  double, dashed, dotted and invisible barlines were lost (every Guitar Pro → MusicXML → import
  round trip of the alphaTab test files lost all 176 of them). They are now read — on every
  staff of the part, as repeats now are — and export writes `dashed`, `dotted` and `none` and a
  `RepeatBoth` bar's repeat signs; export also no longer writes an `<ending type="start">` for a
  volta that only ends at a repeat start.
- **MEI tremolos, arpeggios and tablature marks.** MEI export dropped single-note tremolos
  (now `@stem.mod="Nslash"`) and arpeggios (now `<arpeg order>`), and wrote tablature notes
  without their grace flag, tremolo or lyrics; import reads all of them back. Articulations on
  tablature notes are still reported and not written, because Verovio 6.3 crashes on `@artic`
  inside a `<tabGrp>`. A Guitar Pro → MEI → import round trip of the alphaTab test files now
  keeps its 76 grace notes, 47 arpeggios and 27 tremolos.
- **MSCX export wrote a bar's first voice where MuseScore does not read it.** Chords, rests,
  signatures and barlines of voice 1 sat directly in `<Measure>` and later voices in a
  `<voice>2…` element, but MuseScore 3/4 read a bar's content only inside `<voice>` elements —
  and acorde's own import put the second voice's notes into the first. Every voice is now its
  own `<voice>` (empty ones kept in place before a later voice), with the bar's signatures,
  texts, spanners and barline in the first, as MuseScore writes them.
- **MSCX cross-staff notes.** Chords moved to another staff of their part are written and read
  as `<staffMove>` (73 of them were lost in the corpus round trip), and MSCX import keeps
  hairpins, pedals and ottavas that start on a rest on that rest instead of the next chord.
- **MEI per-part key signatures.** MEI export wrote the first staff's key for every staff, so a
  part in a different key (a transposing part, an early-music voice without the flat) came
  back in the wrong key; staves that disagree now get their own `<staffDef keysig>`, and MEI
  import reads a `<staffDef>` key (attribute or `<keySig>` child) for that staff only. 32 of
  the 205 corpus files lost a part's key this way.
- **Chord symbol labels.** Chord kinds outside a short list were drawn and exported with their
  raw MusicXML kind name ("Daugmented-seventh", "Cdominant-13th"); every MusicXML kind now has
  a compact label (`aug7`, `mMaj7`, `9`/`11`/`13`, `maj9`, `m11`, `add9`, …), shared as
  `CHORD_KIND_SUFFIXES`, and MEI import reads those labels back as the same kind instead of
  keeping them as plain text.
- MSCX export writes invisible barlines (`<visible>0</visible>`), and MSCX import closes a
  volta that runs to the last bar, which MuseScore gives no closing marker.
- **MEI endings.** Voltas were neither written nor read by MEI; they are now `<ending n label>`
  elements around their bars (Verovio draws them), and import gives them to every part.
- MusicXML export wrote the score tempo as a new metronome mark in every bar with an
  `<attributes>` block (a key, time or clef change); it is written once, in the first bar.
- **MSCX export kept only the first part.** Every `<Part>` declared `<Staff id="1">`, so a
  multi-part score (a string quartet, a band score) re-imported, in acorde and in MuseScore,
  with its other parts empty. Staff ids now run across the score.
- **MSCX slurs, hairpins, pedals and ottavas.** MSCX export wrote none of them (it reported
  them); they are now written as MuseScore spanners — slurs inside their first and last
  chords, lines around their notes, with `<next>`/`<prev>` locations — and MSCX import reads
  ottavas. A MusicXML → MSCX → import round trip of the music21 corpus keeps all 15431 slurs
  and 1219 of 1228 hairpins.
- **MSCX tuplets, grace notes, fermatas, ornaments, repeats, voltas and chord symbols.** MSCX
  export wrote tuplets inside the chord (where MuseScore and acorde do not read them), grace
  notes as ordinary chords, repeat ends as an empty `<endRepeat/>` and no repeat starts, voltas
  not at all, chord symbols without a root, and dropped fermatas and ornaments. It now writes
  MuseScore's `<Tuplet>…<endTuplet/>` blocks, `<acciaccatura/>`/`<appoggiatura/>`,
  `<startRepeat/>`/`<endRepeat>2</endRepeat>`, volta spanners, `<Harmony><root>…`, `<Fermata>`
  and ornament articulations; MSCX import reads fermatas, ornaments and a volta's closing
  marker (which it took for a new one-bar volta), and no longer adds each chord symbol a
  second time as measure text. In a MusicXML → MSCX → import round trip of the corpus, 7931
  tuplet notes, 1242 grace notes, 854 fermatas, 442 barlines, 229 voltas and 102 chord symbols
  that were lost now survive.
