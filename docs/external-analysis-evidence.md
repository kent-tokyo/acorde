# External analysis evidence

This document records a bounded music21 comparison protocol. It is not a claim that Acorde and
music21 have equivalent analytical algorithms, engraving, playback, or file-format coverage.

## Projection contract

`scripts/music21_projection.py` parses one fixture with a locally installed music21 and emits
version, fixture SHA-256, options, parsed counts, and music21's inferred key. It does not run in
the normal test suite because music21 is an external optional dependency.

Run it together with Acorde's structural and analysis output:

```bash
python3 scripts/music21_projection.py tests/fixtures/simple.musicxml
cargo run --locked -p acorde-cli -- info tests/fixtures/simple.musicxml
cargo run --locked -p acorde-cli -- analyze tests/fixtures/simple.musicxml
```

Validate a saved report, or stream the observation directly into the generic evidence-contract
validator:

```bash
python3 scripts/music21_projection.py tests/fixtures/simple.musicxml \
  | python3 scripts/validate_external_evidence.py -
```

`validate_external_evidence.py` is intentionally tool-agnostic. MuseScore, Verovio, and alphaTab
measurements must use the same version, fixture hash, options, and projection envelope before they
can be compared or reviewed; validation alone is not a parity result.

The shared projection is deliberately narrow: part count, physical measure count, pitched-note
count, rest count, chord-element count, and one explicitly named key-estimation result. Compare
each field independently. Acorde's key estimates are coverage-based candidates and may be tied;
music21's `score.analyze('key')` selects one result, so a key disagreement is evidence to review,
not a failed semantic round trip.

## Recorded local observation

On 2026-09-22, music21 9.9.1 parsed `tests/fixtures/simple.musicxml` with SHA-256
`55c2d8d2546100437460474b8bc685b173493c3696303cbad2eb85254acbfd7d` as one part, two measures,
five pitched notes, one rest, zero chord elements, and inferred `C major`. Acorde CLI `info`
reported one part, two measures, five notes, and one rest. Its coverage-based key candidates
include C major.

This is one checksum-pinned smoke observation only. It supplies neither a population accuracy
measurement nor a music21 compatibility score. Add more permission-cleared fixtures, pinned
environment details, and explicit expected agreements/disagreements before treating it as a gate.

## Independent global-key corpus (2026-11-07)

The [BCMH Bach chorale baseline](archive/analysis-bcmh-key-baseline.md) compares
Acorde v1.3.10 with music21 10.5.2 against independently annotated keys.
On the 82 mutually parsed scores with matching note/chord event counts and
explicit key labels, music21's single estimate matched 76 labels and Acorde's
first candidate matched 46. Acorde included the label among tied candidates in 78,
which is not single-key accuracy. The result is limited to this corpus and
does not cover harmony or cadence analysis.
The unordered pitch/duration event bag matches on 88 of the 90 paired parses;
this does not check event order, placement, rests or ties.

An [opt-in tonal-profile ranking](archive/analysis-bcmh-key-profile.md) orders
the same tied candidates. Its first choice matched 76 of the same 82 labels,
equal to music21 on this development corpus. This is not an independent
holdout result; parser coverage and other analysis categories remain separate
gates.

The [Mozart sonata holdout](archive/analysis-mozart-key-holdout.md) fixes 53
previously unseen annotated movements before scoring. MuseScore 4.7.5 produced
52 valid common MusicXML inputs; one movement failed to export. After
analyst-provided harmony labels were removed, music21 selected 51 annotated
keys and Acorde's opt-in ranking selected 50, both over the original 53-score
denominator. The 52 paired parses had equal pitched-event counts and unordered
pitch/duration bags. Exporter instability and the narrow global-key task keep
this a provisional, task-specific result rather than overall analysis parity.

The [integrated BCMH rerun](archive/analysis-bcmh-integrated-profile.md) combines
the Humdrum field recovery with opt-in tonal-profile ranking. Both tools parse
98/100 scores; all 98 paired parses have matching note/chord counts and
unordered pitch/duration event bags. On the 89 scores with explicit key labels,
Acorde and music21 each select 83 labels. This remains the ranking rule's
development corpus and does not change the Mozart holdout result.
