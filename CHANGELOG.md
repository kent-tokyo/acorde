# Changelog

## [Unreleased]

- MusicXML direction-level `<other-dynamics>` prose stays visible as expression
  text in SVG and print PDF. Its playback level is still unsupported and
  reported; MusicXML export writes the text as `<words>`.
- Opt-in notation-density print layout with a system cap above four can pack
  lyric-free scores of five or more staves into fewer, larger systems. The
  four-measure default and explicit three-systems-per-page requests retain
  their previous behavior.

## 1.3.10 - 2026-11-06

- **Rust source compatibility:** New public fields on `Measure`, `Note`, `ScoreFragmentMeasureAttributes`, `PrintConfig`, `StandardBassCandidate`, and the standard-bass arrangement options/results require updates to complete struct literals. `ScoreChange`, `ScorePatch`, `SvgGlyphSource`, and `PdfError` add variants that require updates to exhaustive matches. Serde defaults help read older score JSON; they do not preserve Rust source compatibility. Saved preset/layer engine markers must be reselected for 1.3.10. See the migration notes.

### Fixed
- MusicXML wedges pair by their `number` and in time order per staff, so a crescendo that ends where a numbered diminuendo begins no longer draws a zero-length tick and a wedge running to the system edge. A wedge that would end on its own start note is drawn to the next note or the measure end (#391).
- Two-staff parts named "Pianoforte", "Klavier", "Πιάνο", "ピアノ" and other localized piano names, or with a piano instrument id, get the implicit piano brace (#391).
- Print lyrics (1.6 staff spaces) and directions, chord symbols and other annotation text (1.0 staff space) are larger and measured with matching glyph widths. Print lyric collision boxes use the lyric size, so neighbouring syllables no longer merge (#391).
- Print measure text (directions, tempo, `Fine`, …) moves past a slur or tie on its side of the staff instead of crossing it. Span drawing and text placement share one slur-geometry helper (#391).
- Print measures with lyrics keep room for their syllables: a measure is at least as wide as its widest lyric line, its first syllable clears the barline, and every staff of a bar with lyrics shares one beat-to-x map that gives each onset gap the lyric clearance it needs, taking room only from gaps that have more than they need, so syllables stay on their verse line and simultaneous notes stay aligned. A bar still too dense spreads its notes as far as fits instead of keeping colliding rhythmic positions (screen SVG included) (#391).
- Print widens the staff gap where a staff's lyrics would meet noteheads of the staff below in the same measure (e.g. high piano chords under a vocal line) (#391).
- MusicXML import joins several `<words>` in one direction, often split around a `<metronome>`, instead of keeping only the last (Webern's opening tempo printed as `ca 112)`) (#391).
- Physical print-page SVGs, and the PDF pages built from them, now contain only the notation that reaches each page. Previously every page carried the whole score hidden outside its viewBox (#183).
- A chord member tied into another voice's chord at the same onset now draws its tie. Previously only same-voice ties were drawn (#183).
- Print part labels split at their own line breaks, or at the middle space of a long name. They are sized to keep 3 mm of paper clear instead of reaching the page edge (#183).
- MusicXML import reports a wedge replaced by another start (`musicxml.unsupported-overlapping-hairpin`) and a wedge with no stop (`musicxml.unterminated-hairpin`), instead of dropping them silently. Open hairpin state no longer carries into the next part (#183).
- MusicXML import applies a mid-part `<sound><midi-instrument><midi-program>` as a part program change at its position, instead of ignoring it (#203).

### Added
- Opt-in SMuFL music fonts in `acorde-render-svg`. `SvgRenderConfig::with_smufl_font(family)`, or `SvgGlyphSource::Smufl`, draws clefs, noteheads, rests, accidentals, flags, time signatures, dynamics, common articulations and ornaments as SMuFL code points in a host-supplied font such as Bravura, Leipzig or Petaluma. Unmapped symbols stay vector and spacing is unchanged; default output is byte-identical. `glyph_coverage_for` reports `smufl:<family>`, and `render_print_page_svgs_with_glyphs` renders print pages with the setting. WASM render options accept `"smufl_font"`. No font is bundled (#393).
- `acorde-pdf` returns `PdfError::MusicFontText` for SVG containing SMuFL/Private Use Area text, because Harumi draws all SVG text with one font and cannot embed CFF music fonts (#393).
- Humdrum `**kern` and Plaine & Easie (PAE) import behind the new opt-in `acorde-io` features `humdrum` and `pae` (mirrored in the `acorde` umbrella crate, enabled in the CLI): `parse_kern`/`parse_kern_with_report` and `parse_pae`/`parse_pae_with_report`. Kern imports `**kern` spines as parts (lowest spine last), `*^`/`*v` sub-spines as voices, reciprocal durations including tuplets (3, 6, 12, 5, …), dots, chords, ties, slurs, beams, grace notes, common articulations, clef/key/meter/tempo interpretations and barlines/repeats. PAE imports the compact and `@field:` forms: clef/key/time codes, octave marks, carried durations and rhythmic patterns, accidentals, rests, measure rests, chords, grace notes, beams, ties, fermatas, tuplets, barlines/repeats and `!…!f`/`i` repeats. Anything outside the subset is a located `humdrum.*`/`pae.*` import diagnostic; empty, malformed and oversized input returns `Err`. Malformed structure is reported as `Error::Xml("Humdrum: …")` / `Error::Xml("PAE: …")` so the public `Error` enum is unchanged. `acorde convert`/`info`/`report` accept `.krn` and `.pae`. Self-authored CC0 fixtures (`chorale_excerpt.krn`, `incipit_compact.pae`, `incipit_fields.pae`) round-trip through MusicXML (#392).
- `PrintConfig.system_spacing` (`Justified`, the default, or `Packed`). Justified spreads each page's systems over the usable height, up to 1.6 times their reserved height, so pages are not left half empty and the staff space can grow. Spacers and the fixed-height policy keep their positions (#391).
- `scripts/compare_print_corpus.py` measures Acorde print pages and Verovio 6.3.0 on the same MusicXML in headless Chrome: page and system counts, elements past the page edge or margins, text-over-notation and text-over-text candidates, staff space, font sizes, mark counts against the source, and import diagnostics. The ten-score record is in `docs/archive/print-corpus-verovio.md` (#183).
- `scripts/compare_audio_corpus.py` measures Acorde and alphaTab PCM against the same playback schedule (onsets, paired onset difference, release into rests, buffer end, level, import diagnostics). The eight-score record is in `docs/archive/playback-audio-corpus.md` (#203).
- Standard-bass candidates carry a readable `label` (e.g. `C bass + Em chord`) and a `realization` (`direct` or `combined`), and arrangement results report `edition_kinds` (`direct-buttons`, `combined-buttons`, `simplified`, `no-candidate`) and the `harmony_source` (`supplied` or `editorial-proposal`, an option echoed from the input). All new fields default for existing JSON (#358, #359).
- `standard_bass_double_chord_candidates` (Rust and WASM) searches one bass plus two chord buttons, opt-in, capped at 256 results, each button required to contribute a requested pitch class (#358).
- `arrange_standard_bass_proposed_harmony` proposes editorial harmony for the selected melody with `suggest_chord_plans` and marks the result `editorial-proposal` (#359).
- `arrange_standard_bass_with_phrases` lets the melody follow explicit measure-range voice choices; source addresses refer to the unmodified score and playback is re-checked against it (#359).
- A synthetic polyphonic regression test checks the loss report (every source pitch kept or omitted exactly once) and fail-closed ambiguity and omission diagnostics (#359). A Bach Sarabande fixture was not added: the site file is CC BY-SA 3.0.

- Add `acorde mcp --root <dir> [--allow-write]`, a local MCP server over stdio (rmcp 3.8). It
  exposes `score_info`, `validate_score`, `import_report`, `compatibility_report` and
  `render_svg_preview` with the same results and diagnostics as the corresponding CLI paths, plus
  reviewed editing: `open_session`, `preview_edit` (semantic diff and validation, no change),
  `apply_edit` (only the current, valid, previewed proposal), `undo_edit`/`redo_edit`,
  `session_diff`, `write_score` and `close_session`. Ten `Command` types are exposed. Writing
  requires `--allow-write` and an explicit destination inside a root, never replaces the source,
  replaces another existing file only with `overwrite: true`, refuses scores with validation
  errors, and returns the export loss report. Paths must resolve, after symlinks, inside a
  configured root; inputs over 64 MiB and results over 8 MiB are refused. No HTTP transport.
  Tests drive the server both with raw JSON-RPC and with the rmcp client SDK.
- MusicXML export now reports `musicxml.export-shadowed-default-tempo` when a non-default score
  tempo is dropped because the first measure has its own tempo mark (it re-imports as 120 BPM).
- MSCX/MSCZ export writes the score tempo in the first bar when that bar has no tempo of its own, so `settings.tempo_bpm` and playback timing survive a roundtrip (#379).
- Add `arrange_profiled_accompaniment` and `profiled_accompaniment_capabilities` (Rust and full/viewer WASM): `bass-pulse`, `alternating-bass`, `offbeat-chords`, `waltz-bass` and `waltz-alternating-bass` with an explicit standard-bass button profile. Each attack reports the one button actually pressed, its sounding MIDI and per-attack omissions; alternate basses come only from supplied buttons; expanded playback attacks are checked against actions and can be screened with a caller transition policy. `arrange_profiled_bass_pulse` is unchanged (#378).
- Add `arrange_standard_bass_transposed` and `evaluate_standard_bass_transpositions` (Rust and full/viewer WASM): transpose the selected melody and explicit harmony (respelled for the key at each chord, including slash basses and local key changes), then choose buttons again from the unchanged profile, with optional transition screening on the expanded playback order. The bounded search (at most 25 semitone values) classifies each value as arranged, out of range, no button combination, transition rejected or unsupported (#377).

## 1.3.9 - 2026-11-02

- Add explicit standard-bass button profiles, located candidate search, melody-pitch selection and bounded repeat-aware transition/alternative search in Rust and WASM. Source-pitch omissions are reported and results require performer review. A fixed 16-measure Eventide fixture checks selected-melody playback and MSCZ roundtrip with synthetic harmony and a mock profile; it does not establish physical reach or a publishable edition.
- Preserve same-onset voice alignment at supported system openings, and improve diagnostics for unsupported accompaniment rate and range.
- **Compatibility:** `Note::hairpin_above` is a new serde-defaulted public field and new `AccordionPattern` variants affect exhaustive Rust matches. Preset and layer engine markers change with the release; reselect saved inputs. Standard-bass default chord capabilities also change. See the migration notes.

- Correct standard-bass preset and chord-plan capabilities: the documented 12-root default model accepts dominant sevenths and rejects augmented/diminished triads; callers using the latter must choose another arrangement. Add a profiled bass-pulse preset with actual button pitches, per-attack actions and explicit omission reports.

- Whole-score transposition updates chord roots/slash bass and key-aware pitch spelling; checked transposition rejects out-of-range pitches and unsupported fret/pedal rewriting instead of silently clipping.

- Add a lossless single-staff polyphony splitter for free-bass accordion in Rust and WASM, with declared hand limits, pitch provenance and explicit review requirements.

- Add a conservative, explicitly keyed passing/neighbor-note insertion API for Rust and viewer WASM, with source mapping and diagnostics.

- Add driving and lighter 4/4 bass/chord accompaniment options, with explicit
  accent, articulation and standard-bass rate policies. New enum variants require
  updates to exhaustive Rust matches; see the migration notes.

- Add fine bass/chord and habanera 2/4 accompaniment presets. New `AccordionPattern`
  variants require updates to exhaustive Rust matches; see the migration notes.

- Preserve explicit above/below hairpin placement in MusicXML, MSCX, and MEI. SVG and print
  layout reserve additional space for tested adjacent-staff collisions; complex cases remain open.

- Improve beam clearance for chords and refine two-note beam slopes using
  Verovio comparisons. Two-staff pairs account for secondary beams, mixed
  durations, stem attachment, and available clearance in SVG and print.

- MSCX/MSCZ import accepts partial forward moves between directions after a
  backward voice location, without inserting rests or moving later notes.
  Notes and rests at an unresolved backward position still fail explicitly;
  the existing direction-position loss diagnostic remains.

## 1.3.8 - 2026-11-02

- **Compatibility.** Rust 1.89 is now the workspace minimum. Explicit accidentals,
  hidden time signatures, and shared dynamics add public model fields or variants;
  see the migration notes before updating exhaustive Rust matches or struct literals.
- SVG engraving improves slur clearance around stems and beams, simultaneous-voice
  alignment, and grace-note placement. Some dense or multi-part collisions remain open.
- MusicXML import and playback preserve timed pedal changes and dynamics; measured
  single-note tremolo and shared fermata timing are supported. MuseScore instrument
  channels and supported tie locations are retained.
- Update png to 0.18.1, zip to 9.1.3, harumi to 1.23.8, uuid to 1.29.0, and
  rustysynth to 1.3.8.

- Offline render manifests preserve pedal controls attached to rests, including
  swing timing. Hosts can explicitly coalesce identical shared unisons while
  retaining each source address; ordinary playback remains independent.
  MusicXML import reports noncontiguous ties without guessing a sustain.
- MusicXML accepts exactly representable triplet voice-entry gaps and resolves
  numbered slurs by score time across staves. MSCX/MSCZ import MuseScore 4
  chord ornaments and report unsupported cross-staff slur export. Accidental
  reminders no longer repeat after the first occurrence or override the key.
- Single-voice slur side selection now considers mixed stem directions within
  a phrase. Chord stems use inner notes to resolve symmetric outer pitches;
  beams avoid crossing interior noteheads and flatten for beam-side interior
  extrema. Screen and print regressions cover the Verovio-guided cases.
- Same-system slurs now consider the horizontal positions of intermediate
  notes when determining their quadratic bow, improving clearance for peaks
  away from the middle of a phrase.
- Same-system slurs now account for the actual quadratic bow when clearing a
  central high or low note in a phrase.
- Physical print SVG/PDF now keeps a short final system at a notation-demand
  width, including a one-system score. On a same-input Verovio 6.3.0 A4 probe,
  Acorde's one-bar staff shrank from 176.6 to 24.5 mm versus Verovio's 22.0 mm;
  a dense final bar still receives a full line.
- A partially authored beam now carries its explicit stem direction to the
  unmarked notes in the group. Same-input Verovio 6.3.0 probes with upper and
  lower two-note slurs match the resulting up/down stem and opposite-side slur
  placement in both screen and print SVG.
- MusicXML pedal stop followed by start at one score cursor now puts both
  playback actions on the following note while preserving the printed spanner
  endpoint. Full-SF2 rendering releases the old pedal before pressing the new
  one, so the new note keeps its intended sustain. The pinned Webern MXL and a
  synthetic WAV regression cover the retake.
- MusicXML slurs without an explicit `placement` now use the starting Bezier
  control's vertical sign as a bounded above/below hint. The pinned Quilter
  opening phrase bows above its notes, as in Verovio 6.3.0, instead of below
  them. SVG and print slurs use a filled, tapered curve rather than a hairline.

## 1.3.7 - 2026-10-31

- **[breaking for exhaustive Rust source]** `Measure` now has serde-defaulted
  `time_symbol` and `metronome_mark` fields; `StyledText` has a serde-defaulted
  beat position. Score diff/patch enums gained corresponding variants.
  Complete struct literals and exhaustive matches need updating; stored score
  JSON remains readable.
- MusicXML now preserves written common-time and cut-time symbols separately
  from the 4/4 and 2/2 rhythmic values. Screen SVG and print PDF draw vector
  meter glyphs, including a change inside a system. The pinned Quilter piano
  score now prints C on both staves as in the Verovio reference.
- Native and full-SF2 WAV rendering now accepts written `tenuto` when the
  playback schedule has already retained the note's full duration. The pinned
  CC0 Webern score previously failed with `unrealized note expression`; it now
  renders 132 events at both 16 and 48 kHz with the alphaTab-bundled SF2.
- MusicXML now retains explicit metronome equations, including beat unit, dots,
  parentheses, and rate, separately from playback tempo. The Quilter Prelude's
  written quarter-note `=69` appears beside its opening tempo words in screen
  SVG and print PDF. MusicXML roundtrip and score patches preserve the mark;
  a mark without `<sound>` does not invent a playback tempo.
- `audio-render --realize-ornaments` now uses the opt-in deterministic ornament
  schedule for WAV output. Realized attacks no longer carry the ornament mark
  as an unsupported, unrendered instruction; other articulations remain.
  The pinned Chopin Prelude now exports a complete full-SF2 WAV with 603 events.
- Playback now sounds written grace notes before the following main note, borrowing
  a bounded slice from the preceding rhythmic slot (or the main note at the
  beginning of playback). MusicXML-to-MSCZ round trips preserve those attacks.
  On the pinned Chopin Prelude case, the two missing grace attacks now sound
  without moving the following written main-note onsets.
- Physical print pages now move a below-staff direction a short distance right
  when its text box still touches a chord after vertical clearance. The move is
  capped at three staff spaces so longer directions keep their measure anchor.
- Print layouts with opening title and movement/composer credits now reserve
  16 mm before the first music system when the page has room. The physical
  page SVG and vector PDF use the same shifted coordinates, keeping credits
  clear of the opening staff and repaginating if the first page fills.
- Offline PCM16 WAV export now accepts an explicit peak target through
  `audio-render --normalize-peak-dbfs`. The bounded, deterministic pass preserves
  timing and reports input rail samples, so earlier clipping is not hidden.
  The comparison script also reports per-channel peaks and out-of-range
  float samples instead of relying only on a stereo downmix.
- MIDI export now assigns overlapping same-pitch notes to separate channels and
  mirrors their program and controller events. Reimport recognizes mirrored
  control streams without adding duplicate score automation. On the pinned *Just for Today*
  score, all 579 note onsets and durations match Acorde's playback schedule;
  previously eight note durations were ambiguous on one channel.
- The `bass-pulse`, `alternating-bass`, and `offbeat-chords` accordion presets
  now fill 9/8 with three dotted-quarter pulses. Pickup and shortened bars
  retain their measure phase; eighth-aligned chord changes, playback, MSCZ
  export, and bass-staff SVG addresses are covered by regressions.
- MusicXML now retains a direction word's quarter-note position in `StyledText` and
  round-trips it with `<offset>`. Print SVG anchors above-staff words to the notes'
  reflowed beat positions; the two `L.H.` marks in the pinned Quilter Prelude are
  placed at their authored beats rather than spread by a repeated-word heuristic.
  Below-staff words keep their established grand-staff clearance placement.
- MusicXML/MXL now imports each part's initial MIDI volume and front-hemisphere pan into
  CC7/CC10. These settings reach MIDI export and full-SF2 stereo audio and survive MusicXML
  roundtrip at seven-bit controller precision. Rear-hemisphere pan is reported as an import
  loss because MIDI pan cannot encode it. The pinned *Aloha Oe* render now has distinct left
  and right channels; this is a fixed-score correction, not a general audio-quality ranking.
- MusicXML import now keeps later and repeated direction words within one measure, including
  both `L.H.` marks and `poco cresc.` in the pinned Quilter piano score. Print rendering reserves
  a text lane between those grand staves and keeps below-staff words clear of noteheads.
- Full-SF2 audio rendering now applies authored MIDI program changes at their scheduled frames
  and uses the selected preset for later attacks on the source part and channel. Missing presets
  still fail explicitly. This closes one instrument-change gap; same-input alphaTab sound-quality
  superiority is unproven.

## 1.3.6 - 2026-10-31

- Preserved ABC closing bars and corrected marcato orientation. MSCZ export now keeps the
  initial staff clefs, and SVG reserves space for inline meter changes and improves print
  direction legibility. These focused fixes do not establish overall engraving parity.
- Recorded a same-score PCM timing comparison with alphaTab for the pinned fixture. The
  measurement does not establish general playback or audio-quality superiority.
- Kept accordion preset phase through fractional pickups and added versioned free-layer
  combinations and shuffle. New 3/4 waltz patterns provide bass–chord–chord accompaniment;
  an explicit one-note drone sustains through each authored bar independently of chord changes.
  The Rust and viewer WASM APIs preserve source melody, short bars, rests, and repeat playback
  within their documented ranges and rhythm limits.

## 1.3.5 - 2026-10-31

- Fixed ABC alternate-ending closure so repeated sections and playback retain their intended
  span when the ending marker is implicit.
- Added source-positioned chord-plan extraction with exact beat fractions, provenance,
  coverage diagnostics, and an opt-in written-accompaniment analysis. Bounded melody-aware
  chord proposals and replay are available through Rust and viewer WASM. Generated harmony is
  a suggestion and is not presented as historical source evidence.
- Kept accompaniment patterns on the authored measure phase when a chord changes; repeating
  the same chord no longer restarts alternating bass or arpeggios. Preset configurations now
  use a versioned content fingerprint to replay on an equivalent re-parsed score while
  retaining the new input's note IDs. Legacy exact-ID configurations remain accepted with
  their original score and engine version.
- Kept the v1.3.4 Rust struct-literal shape of `AccordionPresetConfig` and
  `AccordionPresetCapabilities`; the fingerprint version is encoded in the existing string.

## 1.3.4 - 2026-10-31

- Added physical page SVG and vector PDF export from validated print layouts, including WASM page
  SVG access, piano and staff-group braces, span continuation, and bounded publication text.
  Density-aware breaks, notation-weighted measure widths, lyric clearance, and larger print text
  improve the pinned piano and vocal fixtures. Broader collision and print-quality review remains
  open; this release does not establish parity with Verovio.
- Added source-backed chord annotations, musical-position comparison, optional display beams,
  MusicXML publication credits, and safer MuseScore nested-excerpt handling.
- Added bounded SF2-to-WAV rendering, note release and score-pedal behavior, plus an opt-in full-SF2
  engine for wider GM preset coverage. The full-SF2 path applies authored pitch bend and MIDI
  CC 1, 7, 10, 11, and 64 on the frame schedule, with isolated channels for overlapping voices.
  Same-input alphaTab comparisons are recorded for fixed fixtures; audio-quality equivalence or
  overall superiority has not been established.

## 1.3.3 - 2026-10-30

- Added bounded print-page validation and PDF assembly from page SVGs with page order and media
  boxes taken from a validated print layout.
- Preserved pickup and shortened-bar timing in MSCZ and ABC, corrected tied staccato playback,
  and kept adjacent authored tuplet groups distinct through layout, SVG, and format export.
- Added source-preserving accordion reduction, explicit-chord and layered accompaniment, and
  versioned seeded presets through Rust and WASM. These APIs return source mappings and diagnostics;
  their documented ranges and host integration limits still apply.
- Raised the all-feature Rust line coverage reported by Codecov on `main` to 92.04%. A regression
  test also found and fixed a panic when tablature optimization encounters an empty voice.
- **[breaking]** `Note` now carries `tuplet_boundary` for authored group edges. Stored score JSON remains readable
  through a default, but Rust code using complete `Note` struct literals must add the field.

## 1.3.2 - 2026-10-30

- Preserved more authored MusicXML semantics during import: part labels, rest notation, beam
  grouping, tempo words, text placement, and voice slots across measures.
- Corrected playback timing for swing pairs, global tempo, irregular measures, cross-voice ties,
  positioned rests, and MIDI export; cue notes no longer produce sounding attacks. Independent
  alternate endings in MSCZ now unfold in the intended order.
- Aligned tuplet beam starts with beat boundaries in SVG output.
- Added an opt-in `playback-only` WASM profile for MSCZ playback event scheduling. The default
  browser API remains available through the full profile.
- Added Codecov reporting and README badges. Workspace line coverage reached 91.02% under the
  all-feature `cargo llvm-cov` command used by CI.

## 1.3.1 - 2026-10-30

- Fixed MSCZ alternate-ending markers that have an orphan closing marker or an open final
  beginning marker. Round-tripping now preserves repeat expansion and playback events.
- Fixed MSCZ parsing of self-closing `<voice/>` elements so later voice slots and tied-note
  playback remain aligned.
- Fixed MuseScore 4.7 MSCZ imports whose `<Part><Staff>` declarations omit IDs. Initial
  `defaultClef`, instrument-relative clefs, and transposition now follow the actual score staves;
  an explicit first-measure clef still takes precedence.
- Added an opt-in `viewer` WASM profile for ABC/MSCZ viewing while retaining the full API as the
  default. The viewer build, browser contract, size, and cold-start evidence are documented in
  `docs/wasm-viewer-profile.md`.
- Fixed WASM SVG `options_json` handling of `staff_gap` across direct, layout, row, and metadata
  rendering. Invalid gaps now return an error instead of being ignored.

## 1.3.0 - 2026-10-28

- Restored `SvgRenderOptions` to its original four public fields. Versions 1.2.19 and 1.2.20
  added `staff_gap`, which broke downstream Rust struct literals despite being patch releases.
- Added `SvgRenderConfig::new(options).with_staff_gap(...)` plus configuration-aware SVG and
  metadata entry points. Future renderer settings belong in this builder type.
- Added an external-crate-style compile regression test for the legacy options literal.
- Renderer and analysis benchmark commands now record fixed-input repetition metadata and host
  information for reproducible local timing evidence; the analysis timing mode requires an explicit
  CPU label.
- Simplified the scorecard and interchange evidence, and documented the published
  `SvgRenderOptions` source-compatibility exception for Rust struct literals.

Recent release notes are listed here; older notes are linked below. Repository tags and commit
history retain the implementation record. Score JSON additions are backward-compatible unless marked
**[breaking]**; consumers should accept unknown additive fields and retain `#[serde(default)]`
compatibility.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Older release notes are preserved in the archive. Each heading below keeps its existing link.

## [1.2.20] - 2026-10-28

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1220---2026-10-28).

## [1.2.19] - 2026-10-28

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1219---2026-10-28).

## [1.2.18] - 2026-10-25

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1218---2026-10-25).

## [1.2.17] - 2026-09-28

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1217---2026-09-28).

## [1.2.16] - 2026-09-28

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1216---2026-09-28).

## [1.2.15] - 2026-09-28

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1215---2026-09-28).

## [1.2.14] - 2026-09-27

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1214---2026-09-27).

## [1.2.13] - 2026-09-27

[Full release notes](docs/archive/changelog-1.2.13-1.2.20.md#1213---2026-09-27).

## [1.2.12] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#1212---2026-09-27).

## [1.2.11] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#1211---2026-09-27).

## [1.2.10] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#1210---2026-09-27).

## [1.2.9] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#129---2026-09-27).

## [1.2.8] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#128---2026-09-27).

## [1.2.7] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#127---2026-09-27).

## [1.2.6] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#126---2026-09-27).

## [1.2.5] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#125---2026-09-27).

## [1.2.4] - 2026-09-27

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#124---2026-09-27).

## [1.2.3] - 2026-09-23

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#123---2026-09-23).

## [1.2.2] - 2026-09-22

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#122---2026-09-22).

## [1.2.1] - 2026-09-22

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#121---2026-09-22).

## [1.2.0] - 2026-09-21

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#120---2026-09-21).

## [1.1.7] - 2026-09-20

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#117---2026-09-20).

## Earlier releases

[Full release notes](docs/archive/changelog-1.1.7-1.2.12.md#earlier-releases).
