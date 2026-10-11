# OMR proposal contract

`acorde-core` does not read images, PDFs, the network, or an AI provider. Optical music
recognition (OMR) belongs to a host such as Acorde Composer. The host supplies an untrusted,
provider-neutral `OmrProposal`; acorde turns only complete and high-confidence measures into
ordinary undoable score commands.

## Boundary

```text
image or PDF → host preprocessing / OMR provider → OmrProposal
             → acorde-core validation and Command plan → reviewed ScoreEngine batch
             → MSCZ or another existing exporter
```

Image bytes, page rasterization, model credentials, network requests, and filesystem access must
not cross into `acorde-core`. `OmrPage.source_id` is an opaque host identifier, never a path or a
URL that acorde resolves.

## Version 1 scope

The initial contract represents pages, normalized source rectangles, confidence values, complete
voice measures, note annotations, and unresolved source items. It can propose pitched notes,
chords, rests, TAB string/fret locations, and unpitched percussion display events with
power-of-two durations and dots. Its typed note annotations are dynamics, lyrics, and chord
symbols; raw OCR text remains unresolved until a host can map it exactly. The built-in adapter
currently exercises a narrower pixel subset:
PNG, successive one-staff rows or successive aligned two-staff rows, a host-specified
power-of-two meter (4/4 by default), and whole or quarter notes. It can propose two voices only
when all noteheads have clearly opposed stems; ambiguous
voice geometry is withheld for review. It intentionally does not claim to recognize handwriting,
lyrics, articulations, ties, beams, tablature, PDF, JPEG, three or more staves, or general
multiple-voice notation.

Each proposed measure replaces one existing `part/staff/measure/voice` lane through
`Command::PasteVoice`. This makes the accepted result undoable as one labeled batch and avoids
adding recognition results to a pre-existing rest-filled measure.

An annotation can target only a note event in a complete proposed measure. After the voice has
been replaced, acorde emits the normal `SetDynamic`, `SetLyric`, or `SetChordSymbol` command, so
the whole recognition remains one undoable batch. Duplicate annotation kinds, invalid lyric/chord
values, low confidence, or annotations outside a recognized measure stop the entire plan.

## Safety and review rules

- A proposal contract version is required and must equal `OMR_PROPOSAL_CONTRACT_VERSION`.
- Pages, measure/event counts, source identifiers, confidence values, and normalized rectangles
  are bounded and validated.
- Only a complete measure whose aggregate and every event confidence meet the host-selected
  threshold can become a command.
- The proposed duration must exactly fill the effective target meter (including a declared pickup
  or irregular measure); a host may not rely on the general score validator to infer missing rests.
- Duplicate targets, unknown pages, unresolved notation, and invalid score content produce typed
  `OmrDiagnostic` values.
- **Any diagnostic clears the entire command list.** A host must correct or explicitly remove the
  offending proposal items, then build a new plan. It must not apply a partial recognition result.
- The command plan is replayed against a clone and passed through the ordinary score validator
  before it is returned. `ScoreEngine::batch_apply_labeled(..., "ApplyOMR")` remains the final
  transactional/undo boundary.

## Host responsibilities

A host owns crop/deskew/binarization, page and staff detection, OMR-provider selection, source
image retention, consent, and the review overlay. It should show every `OmrDiagnostic` at its
source rectangle and treat generated notation as an editable draft, not an asserted transcription.

For a non-interactive host hand-off, the CLI can revalidate an untrusted JSON report before
writing MSCZ:

```bash
acorde omr-apply-report provider-result.json draft.mscz --report application.json
```

The input is a bounded JSON serialization of `acorde_omr::RecognitionReport`; it contains both
the draft score and its provider-neutral proposal. The CLI repeats `command_plan` rather than
trusting a provider's ready flag, and writes no MSCZ unless the plan is valid and diagnostic-free.

The current local PNG-only adapter and its dependency boundary are recorded in the
[OMR license matrix](omr-license-matrix.md). No external OMR or AI provider is bundled.

## Next increments

1. Fixture and measurement corpus with permission and checksum metadata.
2. Single-staff printed notation adapter: clef, key, time, barlines, notes, rests, dots.
3. Multi-staff/voice grouping plus ties, beams, and basic directions.
4. Lyrics, tablature, percussion, handwriting, and difficult photography only after the preceding
   gates have measured correction rates and MuseScore re-open compatibility.
