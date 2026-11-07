# Beethoven sonata global-key holdout against music21 10.5.2 (2026-11-07)

This evaluates the opt-in Aarden-Essen key ranking on [DCMLab's Beethoven piano
sonatas v2.3](https://github.com/DCMLab/beethoven_piano_sonatas/releases/tag/v2.3),
tagged at commit `a10cb43e229db57e3c5e3649d325bee08ffe4d0f`. The dataset is
licensed CC BY-NC-SA 4.0. Cite Hentschel et al. (2024), *An annotated corpus
of tonal piano music from the long 19th century*,
[doi:10.18061/emr.v18i1.8903](https://doi.org/10.18061/emr.v18i1.8903).
No corpus scores or converted MusicXML are committed here.

The selection was fixed before looking at any result: all 64 of the archive's
91 movements whose `metadata.tsv` has an `annotated_key`. No movement was
excluded after selection. The archive SHA-256 is
`078e6af64e0fe56075e0220d522a5cf38778f2fbb15c93851f5ff070eb9861a1`.
MuseScore 4.7.5 exported the MS3 scores to MusicXML; 21,924 analyst-provided
`<harmony>` elements were removed before either analyzer read the files.
Both received the same resulting XML for each movement. The final run reused
64 XML files generated in an earlier pass after validating their XML and
recording each file's SHA-256. The Acorde CLI SHA-256 was
`7e4506e7b68c24b1a052887a6b4a94646d0e47048449857d7e43ece07ce7ff14`.

| Result | Acorde | music21 |
| --- | ---: | ---: |
| Common MusicXML files exported | 64/64 | 64/64 |
| Successfully parsed and analyzed | 64/64 | 64/64 |
| Global keys matching the corpus label | 57/64 | 56/64 |

The two tools chose the same key on 63/64 inputs. On `23-2`, the common pitch
class is spelled D-flat major by Acorde and C-sharp major by music21; the
corpus label is D-flat major. Unordered sounding-pitch-and-written-duration
event bags agree on 64/64 paired parses. This check does not establish equal
event order, measure positions, voices, rests, ties, or source-to-MusicXML
fidelity. The [per-movement report](analysis-beethoven-key-holdout.json) records
input hashes, parse results, import diagnostics and the answer for each tool.

The import work required to reach 64/64 included 18 small source-voice cursor
rounding adjustments and one full-measure rest moved to a separate voice in
the converted XML. The report contains 82,511 Acorde diagnostics, dominated by
76,062 unsupported placement attributes and 6,179 unsupported render
attributes. These are retained in the evidence rather than counted as key
errors. Their effect on complete notation semantics has not been adjudicated.
The Mozart sonata set previously used to identify a missing key candidate is
now development evidence; it is no longer an untouched holdout.

This reaches music21's result for **global key selection on this corpus**. It
does not show parity in harmony, cadence, voice leading, or overall analysis.
The Aarden-Essen option follows music21's default profile and is not an
independent new key-detection method.

## Reproduction

Build `acorde-cli` with this change, download the pinned v2.3 ZIP, verify its
SHA-256, and run:

```sh
uv run --no-project --with music21==10.5.2 python3 -B \
  scripts/compare_beethoven_keys.py beethoven_piano_sonatas-2.3.zip \
  '/Applications/MuseScore 4.app/Contents/MacOS/mscore' \
  /path/to/acorde /tmp/acorde-beethoven-work /tmp/beethoven-keys.json
```

The script checks the archive hash, corpus size and tool versions. Its
`--reuse-valid-musicxml` option reuses previously exported XML in the same work
directory, as in the saved final report. Re-evaluate conversion failures in
the full denominator rather than silently dropping them.
