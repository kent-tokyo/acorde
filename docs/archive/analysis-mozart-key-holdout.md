# Mozart sonata global-key holdout against music21 10.5.2 (2026-11-07)

This was an untouched holdout for PR #403. Its K331-2 result was later
inspected to design the full-candidate Aarden-Essen route, so this corpus is
now development evidence for that route. The figures below remain the
historical PR #403 measurements, not a fresh holdout result.

This is an **untuned holdout observation** of the opt-in tonal-profile ranking in
[PR #403](https://github.com/kent-tokyo/acorde/pull/403). The ranking was developed
on Bach chorales; it was not changed after inspecting these Mozart results.
The corpus is [DCMLab's Annotated Mozart Sonatas v2.3](https://github.com/DCMLab/mozart_piano_sonatas/tree/5337257a5318711e6302cfe85c3f1a6ade3c6271),
licensed [CC BY-NC-SA 4.0](https://github.com/DCMLab/mozart_piano_sonatas/blob/5337257a5318711e6302cfe85c3f1a6ade3c6271/LICENSE).
Its authors request citation of Hentschel, Neuwirth and Rohrmeier (2021),
*The Annotated Mozart Sonatas: Score, harmony, and cadence*,
[doi:10.5334/tismir.63](https://doi.org/10.5334/tismir.63). No corpus scores or converted MusicXML files are committed here.

## Protocol

The archive SHA-256 is
`32c9698aecc398a7a155c4b25c6da848e88d34ed6f0ade4cead3f6a2a0477101`.
The [frozen comparator](../../scripts/compare_mozart_keys.py) uses all 54
movements with an `annotated_key` in `metadata.tsv`. K279-1 was used to check the
export command before this protocol was fixed, so the other **53** form the
primary set. Parse and export failures remain in that denominator.

MuseScore 4.7.5 converted each MuseScore 3 score to MusicXML. The comparator
removed all MusicXML `<harmony>` elements containing analyst-provided chord
labels, while retaining sounding notes and key signatures. It passed the same
resulting file to music21 10.5.2 and to the Acorde binary with SHA-256
`08ce60f2e56c92c50582eb7dd96739e10f514b84b51b04c3aea9e5894ba26387`.
music21 used `score.analyze('key')`; Acorde was measured both with its default
first candidate and `--rank-tied-keys`. A zero-pitch music21 `ChordSymbol` is
excluded from pitched-event counts. The [per-score JSON](analysis-mozart-key-holdout.json)
records source, exported and analysis-input hashes, tool output and errors.

## Result

| Gate | Result |
| --- | ---: |
| Primary movements | 53 |
| Valid common MusicXML input | 52 of 53 |
| Parsed by both tools | 52 of 53 |
| Same pitched note/chord event count | 52 of 52 |
| Same unordered pitch-and-duration event bag | 52 of 52 |
| music21 selected annotated key | 51 of 53 (51 of 52 parsed) |
| Acorde default first candidate selected annotated key | 45 of 53 (45 of 52) |
| Acorde ranked first candidate selected annotated key | 50 of 53 (50 of 52) |

The Acorde ranking is **one movement below music21** on this holdout. In
K331-2, annotated A major, music21 selected A major while Acorde selected D
major. Acorde's candidate list there is D major / B minor: the annotated A
major is absent, so reordering tied candidates cannot correct this case.
In K331-3, annotated A minor, both selected A major. These are
comparisons with corpus labels, not adjudications of every local modulation.
The input projection also matched on unordered notehead pitch-and-duration
bags for all 52 paired parses. It does not test event order, voices, measure
placement, rests, spelling, ties or the fidelity of the MuseScore export to
its original MSCX score.

MuseScore's command-line exporter sometimes terminated abnormally after
writing valid XML. The final run reused 52 previously generated primary
files only after XML parsing and hash recording. K533-3 produced no MusicXML
in repeated single-score and batch attempts (exit status -11), and is counted
as an export failure. The comparator removes 14,598 harmony elements across
the 52 primary inputs. A clean rerun may have different export coverage on
this host; compare per-file hashes and treat this as provisional until an
independent, stable export path reproduces all 53 inputs.

This measures global-key selection on Mozart piano sonatas only. It neither
establishes overall music-analysis parity nor tests harmony, cadence, voice
leading or calibrated confidence. The remaining key error and the export
failure are concrete next investigation targets, without tuning on this
holdout.

## Reproduction

Download the pinned corpus archive above and verify its SHA-256. Build the
CLI from the PR #403 head, install music21 10.5.2, and use MuseScore 4.7.5:

```sh
uv run --no-project --with music21==10.5.2 python3 \
  scripts/compare_mozart_keys.py mozart_piano_sonatas-v2.3.zip \
  /path/to/mscore /path/to/acorde /tmp/acorde-mozart-work out.json
```

The script verifies the corpus hash, metadata count, tool versions and
MusicXML structure. If a converter run writes valid XML but crashes before
returning, the script retains the complete XML and records its nonzero exit
status. `--reuse-valid-musicxml` reuses validated exports from previous
attempts in the same work directory, as done for the recorded report.
