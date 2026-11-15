# Voice-leading quartet development comparison (2026-11-15)

## Contract

`acorde analyze-voice-leading <score>` emits one located quartet for each
successive simultaneous state of two monophonic written voices, across parts,
staves, nonadjacent voice indices, and physical bar lines. Each of the four
notes carries its source `NoteAddr`, spelled `Pitch` (including cents), absolute
written onset, and duration in quarter-note beats. The score fingerprint and
rule ID make the prediction traceable. A held note can occur in both states;
rests, chords, unpitched notes, and zero-duration notes cannot form quartets.
The WASM binding `analyze_voice_leading_quartets(score_json)` returns the same
JSON shape after score validation.

The flags follow keyless `music21.voiceLeading.VoiceLeadingQuartet` 10.5.2
semantics for `parallelFifth`, `parallelOctave`, `contraryMotion`,
`similarMotion`, and `isProperResolution`. Perfect intervals use written
spelling. Antiparallel fifths/octaves count; unisons are not octaves.
`proper_resolution_without_key` means only the four keyless P4, A4, d5,
and m7 rules implemented by music21. It is not a general counterpoint verdict.

## Reproduction

```sh
cargo build -p acorde-cli --locked
PYTHONPATH=/Users/k_tanabe/.cache/uv/archive-v0/RJI-3mPUEWBi42ME \
  python3 -B scripts/analysis_voice_leading_compare.py \
  tests/fixtures/voice_leading_crossbar.musicxml
PYTHONPATH=/Users/k_tanabe/.cache/uv/archive-v0/RJI-3mPUEWBi42ME \
  python3 -B scripts/analysis_voice_leading_compare.py \
  tests/fixtures/voice_leading_motion.musicxml
```

The script rejects a music21 version other than 10.5.2, constructs the same
four `music21.note.Note` values from Acorde's JSON, sets their written onset
and duration, and compares all five flags. It rejects microtonal notes until
an exact cross-library cents contract is established.

| Fixture | Score fingerprint | Quartets | Mismatches |
| --- | --- | ---: | ---: |
| `voice_leading_crossbar.musicxml` | `fnv1a64-c09ecadac13e621c` | 3 | 0 |
| `voice_leading_motion.musicxml` | `fnv1a64-2f0d9ba18858f133` | 3 | 0 |

The independently checked outer-voice pair in the cross-bar fixture is
G4/C4 → A4/D4: a spelled perfect fifth persists across the bar line between
voice indices 0 and 2. The motion fixture includes G4/C4 → A4/D3, an
antiparallel fifth, and C5/C4 → D5/D4, parallel octaves. Rust tests separately
check that C#4/G4 → D4/A4 is *not* a parallel perfect fifth, that unisons
are not octaves, that a rest breaks continuity, and examples from all four
keyless resolution families.

## Boundary

These six small development quartets and hand-reviewed synthetic cases show
that the new extraction and flags work on the selected examples. They do not
establish precision, recall, or superiority over music21 on unseen scores.
The algorithm currently uses written pitch, does not combine ties into one
attack, excludes chords, and silently skips malformed voice bars. A larger
independently labeled, multi-style corpus and a final untouched set are still
needed before the voice-leading gate in `ROADMAP.md` can close.
