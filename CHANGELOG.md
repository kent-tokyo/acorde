# Changelog

## [Unreleased]

- Preserve open tie starts at system breaks and connect a tie to an explicitly
  tied matching pitch in another voice in measured print, including when the
  original voice continues with a rest or has no further event.

- Keep measured cross-staff lyric continuations on their written staff across
  page breaks and reserve their ink between staves. Split a continuation when
  its notes move to another staff within a system.

- Preserve the explicit MusicXML staff side of each lyric verse in the new
  serde-defaulted `Lyric::placement_above` field, through Score JSON and lyric
  editing. Measured print places above-staff lyrics and their continuations on
  the upper side; support remains partial and other SVG rendering still uses
  the lower side. Import reports that limitation, and other supported exporters
  report loss of the side metadata. Adding the public field requires a semver
  review for Rust struct-literal consumers.

- Reserve measured lyric and annotation spacing for voice displacement so
  syllables can share their baseline. MusicXML import now diagnoses lyric
  placement values that the score model cannot preserve.

- Reserve page-start part names inside the configured left print margin in
  measured engraving. Reserve actual displaced lyric and ordinary beam ink
  between staves, and size grace-note slur clearance to the smaller glyphs.

- Interpret MusicXML print breaks before their containing measure and handle
  both empty and paired print elements. Export reports an unrepresentable
  trailing break; import reports unsupported no-break and page-number settings.

- When page balancing is selected, distribute systems independently between
  explicit page breaks instead of disabling balancing for the whole score.

- Account for voice separation and authored horizontal note offsets when
  measuring print columns, so displaced noteheads have room at barlines.

- Draw measure barlines in connected staff and part groups, keeping distinct
  barline styles and repeat dots on their own visible staves. MusicXML import
  copies each part-level barline to the correct measure of padded staves.

- Import and export one static MusicXML part symbol, including its staff range,
  and apply the MusicXML default brace to multistaff parts. Distinguish square
  group brackets from curved brackets. The new `PartGroupSymbol::Square` variant
  requires semver review. Later part-symbol changes and multiple nested staff
  groups remain lossy and are reported by the corresponding import/export reports.

- Preserve MusicXML part-name and abbreviation visibility and the `none`
  part-group symbol, including its absent default. The new serde-defaulted
  `Part` and `PartLabel` fields and `PartGroupSymbol` variant require semver
  review for Rust consumers. Name-visibility changes use the whole-score patch
  fallback. Measured title-width caching follows the displayed title credit.

- Preserve the first-page MusicXML title and subtitle credit text separately from
  work and movement metadata, and retain work numbers. Score JSON gains
  serde-defaulted `ScoreMetadata::print_title` and `print_subtitle` fields;
  Rust struct-literal consumers need a semver review. Credit styling, positioning,
  additional credits and later-page titles are not yet preserved.

- Preserve MusicXML text enclosure values in `StyledText::enclosure`; older Score JSON defaults to no enclosure. Adding this public field requires a semver release review for Rust struct-literal consumers.

- Add the optional `Measure::timed_dynamics` score-time representation and
  `SetTimedDynamics` command. MusicXML directions between note onsets keep
  their timing through export, playback and measured engraving. Composite
  MusicXML dynamics retain their ordered marks and text; repeat-specific
  playback remains unsupported. The new public fields require version review;
  full print qualification remains in progress.
- Reserve articulation ink between adjacent staves in measured print so lyrics
  clear marks attached to notes on the next staff.
- Reserve top space for every continued volta segment during measured pagination.
- Keep cutaway staves visible when they contain timed dynamics or visible pedal
  directions, including over rests.
- Measured print reserves extra horizontal space when a part name cannot fit
  at 2.7 mm. The resulting system breaks remain under corpus review.
- Import text-bearing MusicXML breath and caesura marks as visible articulations;
  paired tags now follow the same path as self-closing tags.
- Preserve concurrent numbered MusicXML wedges as typed crescendo/decrescendo
  spanners, including starts on notes beamed across staves. MusicXML round trips
  retain their numbers and placements; measured collision reflow follows each
  rendered wedge's side. This extends the serialized
  `NotationSpannerKind` enum; release-version review is required.
- Retain hidden MusicXML articulations as performance data with optional
  `Note::hidden_articulations` indices and `SetHiddenArticulations` editing.
  Hidden notation containers suppress their tuplet digits/brackets and articulation
  ink through export. Measured print reserves final fingering and technique-text bounds.
- Preserve authored dynamic placement in optional, serde-defaulted
  `Note::dynamic_above`, `SetDynamicPlacement`, MusicXML import/export and print
  engraving. Unspecified placement stays unspecified through round trips.
- Preserve MusicXML harmony root/kind spellings, Roman or Nashville numerals,
  alterations, local numeral keys and inversions in optional `ChordSymbol::written`
  JSON. Empty authored root text stays empty. `SetChordSymbol` edits and undo/redo
  retain this data. The added public struct field requires release-version review.
- Measured print uses pinned OFL-licensed Bravura Text 1.482 outlines for inline
  music symbols; ordinary text retains the caller's font. Empty SVG paths are
  accepted as nonpainting PDF geometry.
- Retain authored tuplet bracket, number visibility/ratio, and side in the optional
  serde-defaulted `Note::tuplet_display` field with `SetTupletDisplay` editing and
  MusicXML round trips. Hidden source notes and rests remain invisible in PDF.
- Keep authored beams together when they begin on the lower staff and cross upward;
  synthetic fractional timing rests do not acquire visible tuplet marks on export.
- Measured print clears tuplets, text, articulations, beams and ties using final
  geometry, shares compatible filled unison heads, and measures interleaved rhythmic
  columns per physical staff. Qualification against the retired corpus remains ongoing.
- Add opt-in measured print engraving with caller-supplied text fonts, physical
  minimum spacing, per-system collision reflow, outlined SVG text and shared PDF
  geometry. Existing rendering defaults are unchanged. See the engraving parity
  protocol for development evidence and still-pending qualification gates.
- Preserve compound outline counters in PDF fills, additional-verse lyric
  hyphens/extenders, and the placement of numbered slurs sharing endpoints.
- Separate repeat dots and barline strokes at print size, with larger repeat
  dots and measured trailing clearance before backward repeats.
- Add serde-defaulted `Note::arpeggio_number` and its editing command, preserving
  MusicXML groups that join simultaneous rolled chords across voices or staves.
- Extend `MetronomeMark` JSON with optional, serde-defaulted `written` text
  (`per_minute`, `before`, `after`) to preserve empty, qualified and ranged
  MusicXML metronome values and the order of surrounding words. Playback tempo
  remains independent. Adding the public Rust struct field requires callers
  constructing a literal to specify `written: None`; release versioning must
  account for this API change.

## 1.3.15 - 2026-11-20

- Write new standard-bass preset and layer arrangements on separate treble
  chord and bass staves. Preserve playback and MusicXML round trips, and
  return generated note addresses for both accompaniment staves. Existing
  v1.3.14 resolved configs still replay their two-staff layout; regenerate
  them to adopt the new layout. See the
  [migration notes](docs/migrations.md#v1315-standard-bass-accompaniment-staves).
- Add opt-in source-bound standard-bass screening for harmony/groove candidates
  in Rust and full/viewer WASM. Report exact playback pitches, bass/chord button
  combinations and location-aware failures for the supplied instrument profile.
  The unrestricted arrangement path remains available. See
  [standard-bass screening](docs/harmony-groove-standard-bass.md).

## 1.3.14 - 2026-11-18

- **Migration:** `ChordProposalRequest` and `ResolvedChordProposal` add
  `max_changed_spans` and `contextual_nonchord_tones`; complete Rust literals
  need both fields. Generated chord proposal IDs move to the v2 styles.
  Versioned arrangement replay configs from 1.3.13 must be recreated from
  their source and options before replay under 1.3.14. Rust 1.89 remains the
  minimum. See the [migration notes](docs/migrations.md).

- Add source-bound shared kick/bass rhythm generation and replay in Rust and
  full/viewer WASM. Exact slots retain rests, accents, holding lengths and
  selected synchronization points; approach targets are checked over repeats.
  Ghost notes use an explicit short/quiet approximation or a separately routed
  caller-supplied muted patch. Preserve explicit monophonic MSCX drum maps and
  their MIDI keys during import/export. Fixed 8/16-bar auditions accompany the API.

- Add explicit secondary-dominant and destination-key region targets with
  common-chord or dominant approaches in Rust and full/viewer WASM. Candidates
  retain spelled arrivals, report actual repeat-aware guide-tone resolutions,
  respect locks and change limits, and replay from a source-bound contract.
  Fixed 8/16-bar comparisons cover the original accompaniment and three routes.

- Add score-derived harmony accompaniment and source-bound replay in Rust and
  full/viewer WASM. Spelled voices respect the actual bass and melody intervals,
  including eighth/sixteenth anticipations, cross-bar ties, reattacks and accents.
- Add bounded `suggest_harmony_groove_arrangements` and replay: select distinct
  plain, colored and rhythmic scores from explicit tension and rhythm permissions.
  Results report audible differences, costs, shortages and search limits. Fixed
  8/16-bar inputs and a same-SoundFont listening-pack generator accompany the API.

- Add a bounded, opt-in `plan_harmony_voicings` graph API in Rust and full/viewer
  WASM. It preserves ordered voice identities over repeat transitions and uses
  supplied interval-wide bass/melody context for root omission and register limits.
  The score-level API derives this context before selecting its voices.


- Add opt-in `expand_harmony` in Rust and full/viewer WASM: spelled compound
  degrees, tension roles, slash-bass separation, and checked MusicXML
  add/alter/subtract semantics. Existing arrangement presets remain unchanged.
- Preserve extended chord kinds when importing MSCZ compact labels. Absent
  altered extensions such as G7b9 are represented as added degrees.


- Add opt-in phrase-entry leap bounds to rhythmic ensemble-form development
  in Rust and full/viewer WASM. Adjacent active countermelody phrases retain
  their prior ending pitch for the next chord-tone choice; impossible joins
  fail atomically with a measure diagnostic. Saved contracts replay the bound,
  while the original call and its JSON contract keep their prior behavior.

- Add opt-in `suggest_developed_form_proposals` and replay in Rust and
  full/viewer WASM. Select three to five source-bound sectional scores from
  form profiles and source-rhythm countermelody treatments, deduplicate their
  generated playback, and report failed combinations with form diagnostics.

- Add opt-in `arrange_ensemble_form_development` and source-bound replay in
  Rust and full/viewer WASM. Source-rhythm countermelody transformations now
  follow written section activity and dynamics alongside harmony and bass;
  density and vertical collision checks can reject a developed form atomically.

- Add opt-in source-rhythm countermelody development and replay in Rust and
  full/viewer WASM. Imitation, inversion, augmentation, diminution and answer
  treatments retain motif note/rest durations, stay within chord and melody
  clearance constraints, and return a source-bound resolved contract. Existing
  countermelody calls retain their previous score and JSON behavior.

- Add opt-in `suggest_ensemble_form_proposals` and source-bound replay in Rust
  and full/viewer WASM. Three to five whole-piece proposals vary role entrances,
  section textures, dynamics and attack shape. Candidates are deduplicated by
  generated playback events; collided countermelody sections are omitted with
  their indices recorded in the resolved contract. Invalid or insufficient
  requests return a typed error without a partial proposal set.

- Add authored bass rhythm sections to `arrange_bass_line` and `FormBass`.
  Composable note/rest slots choose chord-relative tones, duration and accents,
  including walking motion, syncopation, ostinato, anticipation and phrase-end
  fills. Bars close at their actual length, mid-bar chord changes split slots,
  and invalid plans return no partial score.

- Add explicit parallel-mode borrowed and short chromatic passing chord targets
  to phrase-harmony search. Passing diminished chords require free bass and
  connect neighboring roots by semitone in one direction. Both targets respect
  locks, change budgets and source-bound resolved chord plans; unsupported or
  conflicting targets return no candidate.

- Add opt-in `arrange_ensemble_form_visits_score` and replay in Rust and
  full/viewer WASM. Repeat visits are unfolded into a separate exportable
  performance score with the selected generated variation. The function
  checks that playback equals the visit-specific event contract and rejects
  source-level references it cannot relocate safely.

- Add explicit root-motion cadence targets to phrase-harmony requests. The
  bounded progression search can require dominant–tonic, subdominant–tonic or
  dominant arrival at caller-selected harmonic positions. Candidates,
  including supplied source plans, must satisfy the target; contradictory
  locks return no candidate. The source score and replay fingerprint remain
  unchanged.

- Add explicit `local_keys` to phrase-harmony requests. Bounded major/minor key
  regions override written keys only during chord search; the original score
  and candidate replay fingerprint remain unchanged. Regions must start and
  end on harmonic bar boundaries and are returned with the result.

- Extend opt-in `suggest_phrase_harmony` with explicit source-entry reanchoring.
  New chord positions can split or merge old regions while preserving mapped
  spelling, bass and source references. Locked original regions retain their
  exact boundaries and chord. Omitted source entries are reported, and the
  changed-region budget is checked against the original plan after reanchoring.

- Add opt-in `suggest_phrase_harmony` to Rust and full/viewer WASM. Callers
  choose chord-change pace by complete phrase ranges, including two changes at
  compound or irregular meter-group boundaries. Existing source-plan chords
  and locks are retained only when their positions match the requested pace;
  otherwise the request returns an atomic diagnostic. This initial API uses
  the existing progression vocabulary and does not change the score model.

- Add opt-in `arrange_ensemble_form_visits` and replay in Rust and full/viewer
  WASM. A second written-measure visit can select another generated texture and
  dynamic while retaining the original melody events and a separate written
  base score. The source-bound contract rejects invalid visit indices,
  unchanged alternatives, incompatible parts and ambiguous event timing.

- Add opt-in `arrange_ensemble_form` and `replay_ensemble_form` in Rust and
  full/viewer WASM. An explicit whole-piece plan coordinates generated role
  entrances, harmony and bass patterns, phrase endings and per-section dynamics.
  Source-bound, versioned replay retains the source parts. Invalid plans and
  downstream realization failures return no partial score or reports. A written
  bar retains its treatment on every repeat visit in this initial contract.

- Add opt-in `suggest_arrangement_proposals` to Rust and full/viewer WASM. It
  combines source-bound chord plans with seven bounded ensemble profiles,
  returns three to five distinct scores or an atomic diagnostic, reports
  written note-event difference axes and replays each resolved proposal.
  `regenerate_arrangement_region` replaces a selected physical-measure range
  while retaining source parts, locked roles/measures and generated measures
  outside the range; it also validates locked source harmonic spans. A selected
  region can add or rest a countermelody, and crossing ties are rejected.
  Deduplication distinguishes tied
  sustain, reattack and articulation, and changed-measure reports ignore note IDs.
  When a countermelody is supplied, three-candidate requests try answer
  textures early enough to expose an orchestration alternative. Proposal
  features include per-measure generated attack counts.

- Add opt-in `arrange_ensemble_roles` to Rust and full/viewer WASM. It composes
  independent chordal-pad, bass and motif-derived countermelody parts around
  unchanged source parts, with separate measure activity for each role, pairwise
  register checks and a
  per-measure generated-attack limit. Invalid sections, collisions and density
  violations return diagnostics without a partial score. The resolved schedule
  replays the same note pitches and timing. Standalone `arrange_countermelody`
  extracts a short source-melody contour, realizes it with chord tones and
  bounded melody clearance, and reports its resolved motif and generated notes.
  `arrange_ensemble_with_melody` creates a separate performance score that can
  rest a selected source melody staff in scheduled measures while the generated
  countermelody takes over; the input score remains unchanged. Optional authored
  countermelody phrase endings reduce attacks and invert the following phrase.

- Extend explicit free-bass harmony realization to suspended second/fourth,
  major/minor added ninth and half-diminished chords in presets, chordal pad
  and independent bass. MusicXML and MSCZ round trips retain these kinds;
  standard-bass presets continue to reject unavailable button qualities.
  `ChordProposalRequest.max_changed_spans` optionally limits reharmonization
  to 0–16 source harmonic regions while retaining locks and unchanged source
  provenance. Opt-in `contextual_nonchord_tones` reduces the proposal weight
  of short weak passing/neighboring notes and resolving suspensions. Both
  selections are recorded in resolved proposals.

- Add opt-in `arrange_bass_line` to Rust and full/viewer WASM. It appends an
  independent GM bass part with root pulses, root/fifth alternation or a
  chromatic approach into the next chord, including changes across barlines.
  It composes with chordal pad parts and preserves the source score; register,
  channel and rhythm errors return no partial score. Results report bass attack
  count and absolute semitone movement in written and repeat-expanded order.

- Add opt-in `arrange_chordal_pad` to Rust and full/viewer WASM. It appends a
  polyphonic GM chord part to a validated score, including a multi-part score,
  without replacing or rewriting existing parts. Explicit harmony can be held,
  reattacked each beat or offbeat, or played as an up-down eighth-note arpeggio.
  Optional, non-overlapping physical-measure sections switch these textures
  within one score while preserving chord changes and the source parts.
  Optional `smooth` voicing chooses close inversions in written chord order
  to reduce movement between adjacent chord changes; the default remains
  root position. `open` voicing spreads an inner chord tone up an octave,
  choosing an in-range inversion or reporting that the requested range cannot
  fit it. Optional `min_melody_gap_semitones` and `melody_part_index` constrain
  every chord region beneath all timed notes in a selected source staff.
  Results report selected voicings and symmetric nearest-tone movement in
  written and repeat-expanded playback order, including backward jumps.
  Opt-in `global-smooth` uses bounded coordinate search to select close
  inversions against the expanded playback graph while respecting the chosen
  register and melody clearance.
  `meter-pulse` follows compound or irregular beat groups, with accented
  attacks and rests. Authored phrase-ending measures hold the final two groups
  while preserving chord changes; resolved groups and start ticks are reported.
  Register and MIDI-channel checks,
  exact note splits and structured diagnostics prevent partial output.

- Add `arrange_sections_with_dynamics` to Rust and full/viewer WASM. Callers
  set sustained accompaniment dynamics over written measure ranges, separately
  from pattern and melodic-voice sections. Staff-local markings change left-hand
  playback velocity and survive repeats without changing the source melody.
  Silent measures have no mark; invalid ranges and accent-only dynamics return
  diagnostics without a partial score.

- Add opt-in `arrange_with_response_fills` to Rust and full/viewer WASM.
  Callers select authored two-beat melody rests; the generator places one
  quarter-note chord tone in the second half of each rest, considering the
  neighboring melody notes and the chord active at its onset. Source melody,
  accompaniment and chord plan are preserved. Invalid or unplayable requests
  return diagnostics without a partial score.

- Add `arrange_sections_with_melodic_voices` to Rust and full/viewer WASM.
  Right-hand harmony can switch independently of accompaniment by physical
  measure, including unison-free rests, thirds, sixths and octaves below the
  original melody. Invalid ranges or incompatible chord tones yield no partial
  score. Existing layer and section APIs keep their output behavior.

- Add opt-in `suggest_chord_plans_at_positions` to Rust and full/viewer WASM.
  Callers can select exact chord-change positions inside bars for melody-only
  proposals; invalid positions return no candidates. The existing one-chord-
  per-bar and source-plan behavior, candidate replay and score schema remain
  unchanged.

- Add optional `min_melody_gap_semitones` to both free-bass voicing APIs.
  Block-chord candidates stay below all overlapping right-hand voices for
  their full written duration; effective ceilings are reported per voicing.
  Impossible constraints return diagnostics without a partial score. Omitted
  or null gaps preserve the existing search and serialized voicing shape.

- Add `suggest_chord_plans_with_locks` to Rust and full/viewer WASM for partial
  reharmonization. Selected source-plan entries become hard progression-search
  constraints, retaining chord spelling, slash bass, boundaries and provenance.
  Invalid locks return no candidates; empty locks preserve existing results.
  Existing request/resolved-plan fields and saved-plan replay remain unchanged.

- Add `arrange_sections_with_voice_leading` to Rust and full/viewer WASM.
  It combines explicit accompaniment sections with the shared free-bass chord
  voicing search, preserving melodic patterns, rests, melody and bass attacks.
  Motion reports follow block-chord attacks across section and repeat edges;
  invalid constraints return no partial score or section/voicing mappings.

- Add `arrange_with_sections` to Rust and full/viewer WASM for explicit
  measure-range accompaniment changes and rests. Generation preserves the
  source melody, pickups, meter changes, harmony carry, repeats and endings.
  Resolved per-measure patterns and notated-swing measures accompany the score;
  invalid or overlapping sections return no partial arrangement. Existing
  single-pattern APIs retain their behavior.

- Add opt-in `arrange_with_voice_leading` to Rust and full/viewer WASM. It
  repositions generated free-bass block-chord tones within a caller range and
  span, preserving melody, harmony, bass pitches and rhythm. The bounded search
  includes repeat-back transitions and reports original/selected voicings and
  motion costs. Existing arrangement APIs keep their output behavior; fixed
  standard-bass buttons and melodic bass patterns are outside this API's scope.

- Chord proposals now evaluate the whole written progression with duration- and
  meter-weighted melody evidence, cadence preferences and a change penalty for
  simplified harmony. Jazz proposals distinguish dominant from major sevenths
  and can use applied dominants that resolve in the following region. Generated
  IDs are `conservative-v2`, `simplified-v2` and `jazz-color-v2`; supplied harmony
  and replay of saved v1 plans are preserved. Standard-bass proposals exclude
  unavailable chord qualities during search. The existing Rust/WASM proposal,
  accompaniment and replay APIs use the new search without a score-schema change.


## 1.3.13 - 2026-11-15

- **Migration:** Analysis result schema and cache keys advance from 13 to 14;
  `analyze-vertical` JSON advances to schema 3. Regenerate saved analysis
  expectations and inspect `timeline_issues` before interpreting missing
  sonorities. Preset and free-layer arrangement markers include 1.3.13, so
  reselect saved inputs before replay. Rust 1.89 remains the minimum. See the
  [migration notes](docs/migrations.md).
- Add a [Simplified Chinese README](README_zh.md) with links from the English
  and Japanese introductions.

- Add bounded `shuffle_accordion_plan` in Rust and full/viewer WASM. Explicit
  section choice sets, a seed and attempt budget select a realizable plan with
  source-checked replay and distinct unsupported, no-candidate and search-limit
  results. See [arrangement plans](docs/arrangement-plans.md).
- Add `arrange_accordion_plan` and `arrange_from_resolved_plan` in Rust and
  full/viewer WASM. Explicit written-measure sections select existing accompaniment
  and added melodic voices in one score, with versioned source-checked replay,
  section/note provenance, bounded input and located diagnostics for invalid
  ranges and unsafe tie/tuplet cuts. See [arrangement plans](docs/arrangement-plans.md).
- Add opt-in located `analyze_voice_leading_quartets` in Rust and WASM plus
  CLI `analyze-voice-leading`. It compares all pairs of monophonic written
  voices across parts and bar lines, retaining four note addresses, pitches,
  onsets, durations, and keyless motion, perfect-interval, and resolution
  flags. A pinned six-quartet development comparison agrees with music21
  10.5.2 on five flags; independent multi-style accuracy is not established.
  The headless browser WASM test remains unverified locally.
- Add opt-in `analyze-local-keys --radius` and a Rust centered-window
  Aarden-Essen local-key estimate for each physical measure. It reports the
  source window and correlation, with the written key signature used only to
  choose between enharmonic spellings. On 24 inspected development works a
  radius selected on those works matches 2,325/3,405 annotated local-key
  positions; this is neither a music21 local-key comparison nor a holdout.
- Add opt-in `analyze-cadences [--infer-key]` and Rust report APIs for located
  dominant-to-tonic outer-voice resolutions. Each candidate retains source
  evidence and lower confidence; the existing `analyze` output is unchanged.
  In 18 inspected Mozart movements this raises correct cadence positions from
  7 to 30 and reduces false positives from 122 to 29, while leaving 303 of
  333 gold cadences undetected. This is a development diagnostic only.
- An opt-in `analyze-vertical --arpeggiated-bars` mode completes exact tertian
  chords from short forward note windows and bar openings. Each added label
  records future note addresses and a lower-confidence rule ID. The default
  vertical-only pass remains unchanged; the CLI JSON contract advances to 3
  and records the mode explicitly.
- After the comparator's triplet timing correction, a frozen second 12-work
  When in Rome harmony evaluation failed at 348 versus 394 correct Roman labels;
  that inspected cohort became development data. The opt-in candidate then
  passed a separately frozen third 12-work task holdout: 559 versus 508 correct
  labels at 2,341 aligned primary annotation positions, with both tools parsing
  all 12 answer-free scores. This is a narrow harmony result, not overall
  music-analysis superiority.
- The BCMH harmony comparator now retains all accepted source-positioned Roman
  labels and both systems' predictions (contract 4). A new adapter verifies the
  pinned stripped inputs and projects the development rerun into the common
  label evaluator: 2,193/4,627 canonical matches for Acorde and 2,073/4,627
  for music21 10.5.2 on 97 equal-timeline works. This is not holdout evidence.
- Analysis passes for vertical sonorities, adjacent voice leading and SATB
  spacing/crossing now share written note onsets within each physical measure.
  Different note durations no longer cause index-matched voices to be compared
  at different times; lone whole rests use the actual measure length. The
  serialized analysis result schema and cache key advance to 14. This does not
  yet align malformed bars across parts or analyze motion across barlines in
  the default SATB-facing result; the separate opt-in quartet pass does.
- Vertical-sonority reports now identify missing or unequal physical measures,
  invalid note durations, and overfull voices; affected measures produce no
  inferred sonority. The preceding CLI schema 2 added `timeline_issues` while
  retaining its `sonorities` and key fields.
- If every staff has a valid, equal total written length, vertical harmony now
  aligns different physical bar partitions by absolute beat. Results use the
  first available staff for measure/beat coordinates and keep each source
  note's own address as evidence. Roman numerals now use the concert key when
  active staves agree across those different bars; a supplied concert key is
  still accepted when they do not.
- A tie continuation with a matching start in the same part at the same
  absolute beat and sounding pitch no longer creates a vertical-harmony attack,
  including ties across voices or staves. Explicit predecessor locations must
  match. New members of a partially tied chord still trigger analysis and
  include held pitches; a dangling tie flag remains a new attack.
- Vertical harmony now applies staff and active instrument transposition before
  combining parts, preserving conventional concert spelling for common
  instrument intervals. On equal physical bars, explicit written keys on
  transposing staves are converted to concert keys; Roman numerals are emitted
  only when contributing staves agree. Missing or conflicting concert keys
  leave chord labels without Roman numerals. Concert pitches outside the score
  model's octave range produce a timeline issue instead of a partial chord
  label.

## 1.3.12 - 2026-11-14

- Add `arrange_standard_bass_with_annotations` in Rust and full/viewer WASM. It
  labels each new simultaneous bass/chord attack with caller-supplied button IDs,
  skips tied continuations, and preserves the source score, chord symbols and
  playback timing. The labels do not certify fingering or physical reach.
- Update `wasm-bindgen-test` to 0.3.80, `rmcp` to 3.9.0 and `zip` to 9.4.0.
  Release WASM builds now use `wasm-opt -Oz` to keep the artifact within the
  existing size budget. Rust 1.89 remains the minimum; Rust 1.100 Clippy
  findings were fixed without changing the affected loops' order.
- **Migration:** Preset and free-layer engine markers include the workspace
  version. Reselect saved inputs to create 1.3.12 markers before replaying
  them. The new annotation API is opt-in; see the
  [migration notes](docs/migrations.md).

## 1.3.11 - 2026-11-07

- **Migration:** Rust 1.89 remains the minimum. Saved preset and free-layer
  configurations carry the workspace version; reselect their inputs to create
  1.3.11 engine markers. The minor-key Roman numeral change below can change
  analysis output even when callers do not opt in to a new API. See the
  [migration notes](docs/migrations.md).
- MusicXML lined pedal starts, releases, resumes, changes, offsets between
  notes, and barline releases keep their written positions through playback
  and export. Direction-level `<other-dynamics>` prose remains visible as text
  in SVG and print PDF; its playback level is still reported as unsupported.
  Humdrum import preserves unambiguous omitted durations and unwritten
  trailing silence. Multi-voice MusicXML cursor handling has focused tests.
- Print pages gain opt-in notation-density layout for lyric-free scores with
  five or more staves and a system cap above four. Staff sizing and lyric
  clearance improve on the pinned Verovio 6.4.1 development corpus. The
  default four-measure cap and explicit three-systems-per-page behavior stay
  as before; print parity still needs blind review and an untouched holdout.
- Playback fixes whole-measure-rest fermata timing. The independent alphaTab
  1.9.0 comparison now audits written note onsets and releases, chord-member
  articulations, and pedal intervals more precisely. These comparator changes
  do not themselves change Acorde playback output.
- Analysis adds opt-in vertical sonorities, key-profile ranking, and
  source-beat-aligned BCMH comparison with music21 10.5.2. Minor-key diminished
  chords on the raised seventh now receive leading-tone Roman numerals,
  including supported inversions. The BCMH gains are development-corpus
  evidence, not a general or frozen-holdout parity claim.
- Opt-in contextual passing/neighbor-note insertion accepts chord plans,
  local keys, phrase endings, explicit selections, and a density cap. It
  returns scored candidates and preserves supported typed references when
  splitting source notes. The existing insertion API retains its signature;
  tied, tuplet, and other unsafe anchors remain conservatively skipped.

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
