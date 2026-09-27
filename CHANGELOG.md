# Changelog

All notable released changes are summarized here. The repository tags and commit history retain
the full implementation record. Score JSON additions are backward-compatible unless marked
**[breaking]**; consumers should accept unknown additive fields and retain `#[serde(default)]`
compatibility.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

- **MuseScore rehearsal marks, repeat navigation and breath marks.** MSCX import ignored
  `<RehearsalMark>`, `<Marker>` (segno, coda, fine, to coda), `<Jump>` (D.C./D.S. al Fine/al
  Coda) and `<Breath>` (breath marks, caesuras), and export reported rehearsal marks and
  navigation as not written and dropped breath marks silently. All four now read and write,
  so a D.S. al Coda plays back after a MuseScore round trip. The staff-transposition loss
  report is gone too (MSCX has written the part's transposition since the last release).
- **MuseScore trill lines, expression and system text.** `<Spanner type="Trill">` (the "tr"
  with its wavy line) was dropped on import and reported as lost on export; it now maps to the
  trill line and trill mark both ways. MuseScore 4's `<Expression>` (dolce, espressivo…) and
  `<SystemText>` were ignored; they import as expression and generic text.

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
## [1.2.12] - 2026-09-27

- **SVG stem directions.** A single voice drew every stem up unless the score fixed it. Stems
  left open now follow the engraving rule: notes (or a beamed group, taken together) reaching
  farther below the middle line stem up, the rest (on or above it) stem down. Authored stems,
  grace, unpitched and cross-staff notes, and multi-voice staves are unchanged.
- MusicXML `<forward>`/`<backup>` of zero duration are no longer reported as invalid values.
- **[schema] Dashed text lines.** `NotationSpannerKind::Dashes` holds MusicXML `<dashes>`
  ("cresc. - - -"): imported (58 lines in 4 corpus files were dropped), exported, and drawn as a
  dashed line under (or over) the notes. MEI and MSCX export report them as unsupported.
- **SVG volta brackets, repeat starts, bar numbers and arpeggios.** Voltas (1st/2nd endings)
  were not drawn at all; they now appear over the top staff with their number, a closing hook
  at the ending's last bar, and an unlabelled continuation into the next system. A forward
  repeat inside a system was drawn only at a system's start; it is drawn wherever it falls,
  with room for its dots, and an end-start repeat barline has dots on both sides. Every system
  after the first shows its first bar number above the clef. Rolled chords (`arpeggiate`)
  get a wavy line left of the chord, with an arrowhead for a downward roll, and space for it.
- MusicXML `<ending>` starts written with their text (`<ending …>1.</ending>`, as MuseScore and
  Finale write them) were ignored, so most first and second endings lost their start (22 of 165
  imported in the music21 corpus; now all 165), and a one-bar ending became only an end.
- **Ottava lines.** MusicXML `octave-shift` directions were read the wrong way round: `down`
  (notes displayed an octave below their pitch, an 8va) became an 8vb, and export wrote the
  mirror image, so interchange with MuseScore, Verovio and other tools flipped every octave
  line. Note pitches stay the sounding pitch, as in MusicXML, and SVG now draws the notes under
  an 8va/15ma (8vb/15mb) line one or two octaves lower (higher) instead of on ledger lines at
  their sounding height. `OttavaKind::display_shift_steps` gives the shift.
- **MusicXML hairpins on the right staff and voice.** A wedge stop always ended on the first
  staff's first voice, and a start went to whatever note came next in the file, so left-hand
  and second-voice hairpins lost their end or paired with the wrong notes. Wedges now follow
  their direction's `<staff>` (written on export) and end in the voice they began in; layout
  also closes a hairpin before one starting on the same note. Paired hairpins in the music21
  corpus: 1165 → 1228. Pedal and octave-shift stops set their note flag on the note the typed
  spanner ends on (it was always the first staff's first voice, unpairing left-hand pedals).
- **Typed spanners are no longer dropped.** A slur, glissando, trill line, pedal or ottava held
  only in `Score::spanners` (as `AddSpanner`/`UpdateSpanner` leave it: they clear the note
  flags) was not drawn in SVG, played (pedal), or exported to MEI, MSCX or ABC, and was not
  reported. `Score::with_legacy_spanner_flags` marks such spans on their endpoint notes, and
  layout, SVG, playback, those exporters and their loss reports use it.
- MusicXML export wrote a typed pedal's or ottava's stop before its last note, so each round
  trip ended the span one note earlier; the stop now follows the note (and its chord members).
- MusicXML lyric elisions (`<text>`, `<elision>`, `<text>`) kept only the last syllable; both
  are kept, joined by ‿, and written back as an elision.
- **Percussion plays its kit sounds.** Unpitched notes played their display pitch (a snare on
  C5 sounded MIDI 72). Playback events and MIDI export now sound the kit instrument's
  `midi_unpitched` key (`Part::percussion_key`), found by the note's instrument id or, for chord
  members, by display position (`PercussionInstrument::staff_position`, now filled by MusicXML
  import). `PercussionInstrument::midi_unpitched` is the 0-based General MIDI key: MusicXML's
  1-based `<midi-unpitched>` is converted on import and export. Unpitched notes are no longer
  reported as unsupported; `musicxml.unpitched-without-instrument` flags a part whose unpitched
  notes name no playable instrument.
- **[schema] Compound dynamics.** `Dynamic` gains `Fp`, `Sfp`, `Sfpp`, `Pf`, `Sffz`, `Sfzp` and
  `N` (niente), with `Dynamic::ALL`, `Dynamic::from_musicxml_str` and
  `Dynamic::sustained_level`. MusicXML, MEI and MSCX read and write them (478 fp/sfp marks in
  the music21 corpus were dropped; `sffz` was folded into `sfz`). `<other-dynamics>` is now
  reported instead of silently ignored.
- **Playback dynamics hold until the next marking.** `PlaybackEvent::velocity` and MIDI export
  use the staff's dynamic in force (`DynamicTimeline`) instead of giving unmarked notes velocity
  64: p/f levels persist across notes, voices, bars and repeats; sf/sfz/fz/rfz/sffz accent only
  their moment; fp/sfp/pf and the like attack at the first level and continue at the second.
  Hairpins ramp the velocity across their notes towards the marking that follows them, or two
  dynamic steps up or down when none follows, and the level stays there.
- **[schema] Lyric extenders.** `Lyric::extend` (omitted from JSON when false) marks a melisma
  line after the syllable. MusicXML `<extend>`, MEI `con="u"` and MSCX lyric `ticks`/`ticks_f`
  import and export (379 extenders in 45 corpus files were dropped); SVG draws the line on the
  lyric baseline to the melisma's last notehead.
## [1.2.11] - 2026-09-27

- **[schema] Mid-bar clef changes.** `Measure.mid_clefs` (additive, omitted from JSON when
  empty) holds clef changes inside a bar at their offset. MusicXML, MEI and MSCX import keep
  them where they occur (MusicXML and MEI moved them to the next barline; MSCX applied them to
  the whole bar) and export them before the note they precede; SVG draws a small clef there,
  leaves the same room on every staff, and reads later notes in the new clef. Commands, diff,
  patches, validation, split and join handle them. 78 changes in 22 music21 corpus files now
  import in place and survive a MusicXML round trip.
- MusicXML export wrote no bar clef changes for a part's second and later staves; they are now
  written with their staff number. MusicXML import no longer marks every bar with an
  `<attributes>` block as restating the first staff's clef.
- MSCX export names alto and tenor clefs `C3`/`C4` (it wrote `C`, which MuseScore does not
  define), and import reads `C4` as tenor.
- **MusicXML dynamics import.** `<dynamics>` marks were not imported at all (exported ones did
  not survive a round trip). A direction's dynamic now attaches to the next sounding note of its
  staff and a `<notations>` dynamic to its own note; fp, sfp, pf, n and other-dynamics, which the
  model does not hold, are reported as `musicxml.unsupported-dynamic`. 7322 dynamics now import
  from 205 music21 corpus files (none did before). Export writes the direction's `<staff>`.
- SVG dynamics are drawn as bold slanted letter shapes (p, m, f, n, r, s, z; plain vector
  paths), falling back to italic text for other letters.
- SVG unbeamed stems run 3.5 spaces past the chord's stem-side note, 0.75 more per flag beyond
  two, and reach the middle line from notes far outside the staff.
- SVG accidentals are engraved filled shapes: sharps with thick rising crossbars and an offset
  left stem, a flat whose bowl swells and tapers into its stem, a natural with the left stem
  up and the right one down (they were drawn the other way round), and a double sharp with
  square arm ends. Event spacing reserves an accidental's width wholly before its own note, so
  a double flat no longer runs into the previous stem.
- SVG key signatures use the engraved staff positions per clef (F♯ on the treble top line,
  the tenor clef's low-starting sharps); they were placed at the octave nearest the middle
  line, which put F♯ in the bottom space.
- SVG beam groups written across two staves are kneed: upper-staff notes stem down and
  lower-staff notes up to one horizontal beam between the staves (authored stem directions
  are kept), with stems extended to secondary beams on their far side.

- SVG cross-staff notes are drawn on the staff they are written across to (they were drawn on
  their own staff with ledger lines), stems, ledger lines and beams included.
- SVG time signatures, and tuplet numbers, use bold engraved-style numerals filling half the
  staff (they were thin seven-segment digits), centred when the numerator and denominator
  differ in width; treble, bass and C clefs are redrawn after the engraved shapes. Still plain
  vector paths with no font. Header widths, spacing and golden SVGs change.
- SVG ties leave a single note on the side away from its stem, run from just past one notehead
  to just before the next, and are shallow crescents (they bowed on the stem side, from notehead
  centre to centre, almost two spaces high).
- SVG quarter rests are the engraved zigzag with a curled foot, and eighth and shorter rests a
  slanted stem with one ball-ended hook per flag.

## [1.2.10] - 2026-09-27

- SVG beams now join the stems (they ran between notehead centres), and sixteenth and shorter
  beams stack toward the noteheads instead of outward.
- SVG lyrics, dynamics and chord symbols sit on lines measured from the staff, so verse 1
  reads level across the bar; dynamics move above a staff that carries lyrics. Staccato,
  tenuto and accent sit at the notehead opposite the stem; fermatas, ornaments, marcato and
  string marks go outside the staff. Fermatas, mordents, turns and shakes are drawn as glyphs
  instead of words, and a tremolo is drawn as strokes across the stem.
- MEI `<tie>` control events from or to one note of a chord tie that pitch only.
- SVG lyric hyphens are short dashes centred between syllables on the lyric line (they were long
  lines drawn about a line above the text), and flags have an engraved S-shape on the right of
  the stem, spaced for sixteenths and shorter.
- **Clef changes.** SVG rendering applied a clef only at the start of each system, so a clef
  change inside a system left the rest of it drawn in the old clef; the change is now drawn
  (small, at the start of its bar) and the notes follow it. MusicXML clef changes on a later
  staff (a piano left hand moving to treble clef) were lost; they now land on that staff's bar,
  or on the next bar when written after the staff's notes, and a C clef on line 4 imports as a
  tenor clef.
- **MusicXML staff placement.** A voice written wholly on another staff in a bar (Finale and
  others number the left hand 3 or 2) now belongs to that staff instead of being imported as
  cross-staff notes drawn on the upper staff (2997 → 73 cross-staff notes across 205 music21
  files). Time signature changes now reach every staff of a part.

## [1.2.9] - 2026-09-27

- **Guitar Pro 3/4/5 import.** `acorde_io::parse_gp` now also reads `.gp3`, `.gp4` and `.gp5`
  binary files (detected from their bytes; the CLI accepts the extensions). They share the GPIF
  path's mapping and `gp.*` diagnostics: tuning and capo, string/fret, bends, hammer/pull,
  slides, dead notes, harmonics, grace notes, palm mute/let ring, tremolo picking, strokes,
  lyrics, markers, repeats and endings, tempo changes, and directions. All 90 GP3/4/5 files in
  alphaTab's test data import and export to schema-valid MusicXML and to MEI that Verovio loads.
  Accent and ghost velocities that GP5 stores as note dynamics are not read as dynamic marks.
- Guitar Pro coda/segno/fine directions import as navigation marks (GPIF and GP5).
- **Per-pitch chord ties.** `Note::pitch_tie_starts`/`pitch_tie_ends` (additive, omitted from
  JSON when empty) record a tie on only some notes of a chord. MusicXML, MEI, MSCX and Guitar
  Pro import keep them, MusicXML, MEI and MSCX export write each note's own tie, playback sustains only
  the tied pitches, and the SVG renderer draws one tie per tied notehead (upper notes bow up,
  lower notes down). The 1.2.8 `*.partial-chord-tie` diagnostics are no longer needed.
- **MusicXML note positions.** A note's `default-x`/`default-y` (absolute positions in the
  source engraver's layout) are no longer imported into `Note::offset_x`/`offset_y`, which the
  renderer applies as nudges. Scores exported by MuseScore or Finale rendered with notes outside
  their bars (164 of 274 music21 corpus files) and, where `default-y` was present, drawn above
  their staves (80 files). `relative-x`/`relative-y` still import, and `offset_*` now export as
  part of `relative-x`/`relative-y`.
- **SVG noteheads at standard size.** Noteheads were about half size (0.62 × 0.48 spaces); they
  now follow SMuFL/Bravura proportions (about 1.15 × 0.85 spaces, tilted), with stems, flags,
  dots, ledger lines and other head shapes following. A second in a chord puts its head across
  the stem (right with the stem up, left with it down), accidentals sit clear of the head, and a
  bar whose first notes carry accidentals starts its content clear of the clef and time
  signature. SVG output and golden fixtures change accordingly.

## [1.2.8] - 2026-09-27

- **Guitar Pro import.** New `gp` feature (`acorde_io::parse_gp`, `parse_gp_with_report`; enabled
  in the CLI and WASM builds, umbrella feature `acorde/gp`). It reads Guitar Pro 7/8 `.gp` and
  Guitar Pro 6 `.gpx` files: tracks become parts, staves keep tuning and capo, and notes keep
  string/fret positions, rhythm, ties, dynamics, lyrics, repeats, endings, sections, and tempo,
  plus bends (with curves), hammer-ons/pull-offs, slides, dead notes, palm mute/let ring,
  harmonics, accents, left-hand fingering, tremolo picking, pick strokes, and brushes (as
  arpeggios). Other content is reported as `gp.*` diagnostics
  with counts. All 133 GP6/7/8 files in alphaTab's test data import, validate, and export to
  schema-valid MusicXML and to MEI that Verovio loads. GP3/4/5 binary files are not supported.
- SVG tablature staves now draw a TAB clef and no key signature. Fret numbers mask the string
  line behind them, and dead notes are written `X`.
  Tab staves no longer draw beams placed by pitch, and a technique text such as "let ring"
  that continues from note to note is written once per run within a bar.
- SVG whole and half rests now use SMuFL proportions (about 1.1 × 0.5 spaces), the whole rest
  hangs from the fourth line instead of the top line, and a measure rest is centred in its bar.
  Rests on tablature staves sit in the middle space. Guitar Pro bars holding only a placeholder
  rest import as measure rests.
- Rendering no longer fails with "minimum measure widths exceed the available system width"
  because of f32 rounding, or when a very short bar sits next to long ones.
- MusicXML cue notes now keep their written time at the I/O boundary. Import advances the cursor
  past them, and export writes their duration followed by a `<backup>`. All 268 music21 corpus
  exports validate against the MusicXML XSD.
- MusicXML `<slide>` is written in `<notations>` and read from either place.
- A tie on only some notes of a chord is now reported (`musicxml.partial-chord-tie`,
  `gp.partial-chord-tie`). The model ties whole chords, so such a tie is still applied to every
  note of the chord; 26 of 205 music21 corpus files contain one.
- MEI tablature course numbers now follow acorde's string order (string 1 = lowest), so tab
  exported to Verovio is no longer upside down.
- GM instrument ranges widened to professional ranges (string quartet high positions, drop-D and
  7-string guitar, 5-string bass, trombone pedal tones), so standard repertoire no longer fails
  validation with `OutOfRange`.

## [1.2.7] - 2026-09-27

- MEI export now writes every part. It previously wrote only the first part and dropped the
  rest without a diagnostic. Parts become labelled `<staffDef>`s or labelled `<staffGrp>`s
  (the form Verovio and MuseScore use), part groups become enclosing `<staffGrp>`s, and
  `<instrDef>` carries the MIDI channel and program. MEI import splits parts when the first
  `<scoreDef>` names instruments; unlabelled layouts still import as one part.
- MEI chords are exported as `<chord>`; before, only the first pitch was written. `<chord>` is
  imported too; it used to be rejected. Lyrics are written as standard `<verse n><syl wordpos>`
  note content for every verse. Articulations are written as `@artic`. Fermatas, trills,
  mordents, turns, breaths, caesuras, dynamics, hairpins, slurs, ties, pedals, and ottavas are
  measure-level control events with `@startid`/`@endid`, and their spans may cross barlines.
  Hairpins were previously dropped without a diagnostic.
- MEI now covers mid-piece meter, key, and clef changes, system and page breaks, beams, grouped
  tuplets, cross-staff notes (`@staff`), barlines on `measure@left`/`@right`, measure rests in
  layers, and typed measure texts (`<dir type="acorde-…">`). Accidentals are written visibly only
  where the key signature and earlier accidentals in the measure do not already imply them
  (`@accid` vs `@accid.ges`), and the key uses MEI 5 `@keysig`. With no explicit beams, the
  default beat beaming is written.
- MEI import reads what Verovio writes: `@accid.ges`, `<accid>` children, `<clef>`/`<keySig>`/
  `<meterSig>` inside `scoreDef`/`staffDef`, `<tie>`, `<space>`, `<mRest>` (as a lone whole rest),
  `@tstamp`-anchored control events, and tuplets recovered from `@dur.ppq`. Two bugs that dropped
  text are fixed: an empty `<title/>` made the importer swallow every later lyric and dynamic,
  and entity references such as `&amp;` were removed from all text.
- The OpenScore fixtures now load in Verovio 6.3 with an empty toolkit log, and Verovio's own
  re-encoding imports back with the same notes, chords, verses, marks, and spans.
- MEI tablature: `<tuning>`/`<course>` become the staff's `TablatureConfig`, `<tabGrp>` notes
  with only `@tab.course`/`@tab.fret` take their pitch from the tuning, and tablature staves
  export as `notationtype="tab.guitar"` with `<tabGrp>`. Tablature MEI previously failed to
  import. A single named part keeps its name as a `<label>`.
- MusicXML export now validates against the official MusicXML 4.0 XSD; before, every exported
  fixture was invalid, and the Lieder fixture had 713 errors. `<note>` children are written in
  schema order. Chord members repeat the chord's dots, ties, and tuplet ratio, and their tab
  positions go inside `<notations>`. Beams are written, either explicit or the default beat
  grouping, and tuplets get `<tuplet>` bracket notations. Part ids that are not XML names are
  written as `P-<id>`, and text positions move from `<direction>` to `<words>`/`<rehearsal>`.
- MusicXML import reads `<beam>` groups into `BeamState`. Before, explicit beaming was replaced
  by default beat grouping.
- MusicXML import now handles real-world files that previously failed. On a 268-file sample of
  the music21 corpus, imports went from 153 to 268. The fixes: DOCTYPEs with any external
  identifier are accepted (internal subsets and entities are still refused). UTF-16 and
  BOM-prefixed files are decoded through the new `acorde_io::decode_xml_text`, which the CLI
  and the MXL reader use. Orphan spanner stops (`musicxml.orphan-spanner-stop`), backups past
  the measure start (`musicxml.backup-underflow`), overfull bars (imported as an irregular
  `actual_length`), and chord members on another staff (`musicxml.chord-staff-mismatch`) are
  reported instead of aborting the import. A chord member whose `<voice>` differs from its
  chord joins that chord. `<midi-unpitched>` is read from `<midi-instrument>`.
- MusicXML export fixes found with the XSD on that corpus: `<ending>` comes before `<repeat>`,
  `<wavy-line>` is inside `<ornaments>`, breath marks and caesuras are inside `<articulations>`,
  `<degree>` includes all its required children, harmony kinds are mapped onto the schema
  enumeration, and `<score-instrument>`/`<midi-instrument>` are well formed. 266 of the 268
  exports now validate; the remaining two contain cue notes. MEI export gives unique ids when
  measure numbers repeat, adds the layer Verovio needs for cross-staff notes, and writes the
  percussion clef as `perc`. All 268 MEI exports load in Verovio, and no warning traces to the
  exporter apart from ties the source itself leaves unterminated.
- MuseScore import now sets each staff's starting clef from `Part/Staff/defaultClef` and
  `Instrument/clef`. Before, bass-clef instruments and piano left hands imported in treble. A
  clef change in a later measure also no longer replaces the starting clef.

## [1.2.6] - 2026-09-27

- MuseScore (MSCX/MSCZ) import now keeps voice-level hairpins, pedal lines, and 3.x slurs, slur
  ends, `<LayoutBreak>` system/page/section breaks, and `<BarLine>` double/final/dashed/dotted
  subtypes, which were previously dropped without a diagnostic. Hidden (`visible=0`) hairpins,
  pedals, and dynamics import as visible marks and report `mscx.unsupported-visibility`; unknown
  barline subtypes report `mscx.unsupported-barline`. MSCX export now writes layout breaks and
  those barline subtypes.
- A voice holding one plain whole rest is a measure rest that fills its bar in any time signature
  (`voice_duration_beats`, `Note::is_plain_whole_rest`). MusicXML `<rest measure="yes"/>` and
  MuseScore `durationType=measure` rests in 3/4, 6/8, and other meters no longer fail validation,
  and MSCX export writes them as `measure` rests of the bar's length.
- Added `Articulation::{UpBow, DownBow, Harmonic, OpenString, Stopped, SnapPizzicato}` with
  MusicXML `<technical>`, MSCX, MEI, ABC, and SVG support (ABC reports the harmonic as a loss).
- **[schema]** Added optional `Measure.measure_repeat` for one-measure repeats. The measure stores a
  playable copy of the repeated measure, so playback and validation need no special handling; SVG
  draws the repeat sign instead of the copy. MusicXML `<measure-repeat>` start/stop and MuseScore
  `RepeatMeasure`/`MeasureRepeat`/`measureRepeatCount` import and export. Validation reports
  `InvalidMeasureRepeat` and warns with `MeasureRepeatContentDiffers` when the copy no longer
  matches its source. Multi-measure repeats import as written notes and are diagnosed.
- MusicXML beat repeats and slash styles, lyric extend/elision, dashes, brackets, non-arpeggiate,
  and harmony frames are now source-diagnosed instead of silently dropped.

## [1.2.5] - 2026-09-27

- **[schema]** Added optional `Measure.actual_length` (`MeasureLength`, a fraction of a whole note)
  for pickups, incomplete final bars, and other irregular measures. Validation, playback timing,
  metronome clicks, editing capacity, time-signature changes, logical layout, and SVG spacing use
  it instead of the time signature. MusicXML import keeps a measure whose content ends before the
  time signature at its authored length instead of padding it with rests; export writes the
  shorter content and marks a shortened first measure `implicit="yes"`. MSCX/MSCZ import and
  export map it to MuseScore's `<Measure len="n/d">`. Invalid lengths are reported as
  `ValidationError::InvalidMeasureLength`.
- **[schema]** Added optional `Note.additional_lyrics` (`VerseLyric`) for lyric verses 2–32;
  verse 1 remains `Note.lyric`. MusicXML `<lyric number>` and MuseScore `<Lyrics><no>` import and
  export every verse instead of letting a later verse overwrite verse 1; unnumbered or duplicate
  numbers take the lowest free verse. `SetLyric` gains an optional `verse` (legacy JSON still
  targets verse 1), SVG draws each verse on its own line, and ABC, MEI, and MIDI export report
  unwritten verses as `*.export-unsupported-lyric-verse` losses. Invalid or duplicate verse
  numbers are reported as `ValidationError::InvalidLyricVerse`.

## [1.2.4] - 2026-09-27

- MusicXML `<staves>` now materializes every declared staff, including staves without a numbered
  clef or notes, and export writes it back, so staff count and note staff ownership survive a
  round-trip (#81). A note that continues its voice on another declared staff imports as a
  cross-staff placement owned by the voice's staff (voices 1–4 on staff 1, 5–8 on staff 2, …);
  export numbers voices on additional staves the same way when no source number is retained.
  Note staff references beyond a `<staves>` declaration are source-diagnosed as
  `musicxml.undeclared-staff-reference`.
- MusicXML `<staff-details><capo>` now imports and exports for tablature staves and time-local
  tablature changes; the former `musicxml.export-unsupported-capo` loss is removed.
- Added undoable `RespellStaffRegion` (Respell Pitches for a selection) with flat, sharp, and
  local-key policies. Tie chains crossing the range boundary keep one spelling, and unpitched notes
  keep their staff position. WASM `ScoreEngine.respell_staff_region` exposes it.
- Added undoable `CycleEnharmonicSpelling` (MuseScore's "Change enharmonic spelling") for a note
  or one chord member, cycling natural, sharp, flat, double sharp, and double flat spellings of
  the same sounding pitch while keeping microtones.
- Added undoable `ResequenceRehearsalMarks` (letters, numbers, or measure numbers continuing the
  first mark), `SetSystemBreakInterval` (a line break every N measures, or none), and
  `RemoveTrailingEmptyMeasures` (keeps rests with marks and moves a final barline).

## [1.2.3] - 2026-09-23

- Consolidated English/Japanese entry points, print guidance, release notes, and evidence links.
- Added Phase 18 external-observation and Phase 19 logical print-corpus documentation. These are
  evidence and contract improvements, not a new compatibility or publication-quality claim.

## [1.2.2] - 2026-09-22

- Accepted the conventional, non-loading MusicXML public DTD declaration emitted by MuseScore;
  internal subsets, `ENTITY`, and `SYSTEM` remain rejected.
- Corrected MusicXML tablature string-number conversion at the parser/serializer boundary.
- Added bounded checksum-pinned interoperability observations and separated annotation, span,
  tablature, and command-remapping internals without changing the public command API.
- Updated workspace versions, scorecard, examples, browser metadata pin, and the 5 MiB WASM gate.

## [1.2.1] - 2026-09-22

- Unified deterministic collision lanes for legacy spans, tablature connections, and
  lyrics/dynamics/chords/articulations; measure text treats resolved annotations as obstacles.

## [1.2.0] - 2026-09-21

- Added channel-aware SoundFont materialization and SF3 logical-stream decoding.
- **[breaking]** `SoundFontPresetZone` must be built with `new` or `with_channel_layout`.
  Materialized-zone JSON gained additive `channel_layout` and `decode_channels` fields.

## [1.1.7] - 2026-09-20

- Added backward-compatible typed `Score.spanners` for numbered slur, glissando, trill, pedal,
  and ottava ranges, with editing, layout, and SVG identity support.
- Preserved MusicXML source voice numbers and cursor gaps; added safe MSCX numeric-fallback
  diagnostics and bounded SoundFont zone inheritance.

## Earlier releases

Versions 0.1.0–1.1.6 established the core score model, commands, MusicXML/MIDI/ABC/MEI/MSCX
boundaries, deterministic SVG and browser contracts, analysis, playback, security hardening,
print metadata, and release infrastructure. Use the matching tag, [migration notes](docs/migrations.md),
and focused contract documents when upgrading from those versions.
