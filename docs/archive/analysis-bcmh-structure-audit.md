# BCMH part and measure audit (2026-11-07)

This extends the [integrated global-key rerun](analysis-bcmh-integrated-profile.md)
with a structural comparison of the same cleaned `**kern` inputs. The pinned
BCMH archive has SHA-256
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`.
Acorde was built from `9a0f8078` with Rust 1.89.0; its CLI SHA-256 is
`e621c057ae7ca80e3a0690a5d519cd65cfe4bca492a8def514f9239a0d6176ca`.
The comparison uses music21 10.5.2. The [per-score JSON](analysis-bcmh-structure-audit.json)
records both tools' parse outcomes and the new structural flags.

Both parsers read each score after the leading harmony annotation spine and
tonic/mode interpretations are removed. The script compares part and measure
ordinal positions. Within each measure, it compares multisets of pitches and
durations, then additionally compares their onset offsets. Visible and hidden
rests are counted separately. The music21 Humdrum part order was checked
against the corresponding Acorde part order on a source score before this run.

| Check on paired successful parses | Agreement |
| --- | ---: |
| Successful parses | 98/100 in each tool |
| Equal global pitch/duration bags | 98/98 |
| Equal part and measure counts | 97/98 |
| Equal pitched events within each part and measure | 97/98 |
| Equal pitched onsets within each part and measure | 97/98 |
| Equal visible-rest onsets within each part and measure | 97/98 |
| Equal all-rest onsets, including hidden rests | 81/98 |

`chor011_rev.krn` is the sole part/measure-count and pitched-onset mismatch.
Its cleaned source line 35 has the `=4` barline in three of four `**kern`
fields. music21 combines the soprano events across that missing field into
one measure; Acorde reports separate measures. This is a malformed-source
disagreement, not evidence by itself that either parser recovered the intended
barline correctly. In 16 other scores, visible-rest positions agree while
hidden-rest positions differ. For example, Acorde inserts a hidden eighth
rest at the end of bass measure 6 in `chor003_rev.krn`, where music21 leaves
that part's measure underfull. Those hidden rests need source-level review
before either representation is called a loss.

The per-measure onset check is stronger than an unordered global event bag,
but it does not establish matching absolute positions across measures, voice
identity, pitch spelling, ties, or analytical accuracy. The 83/89 explicit-key
development result is unchanged. A frozen, independently labeled holdout is
still required for a broader analysis claim.

Reproduce with the pinned archive and a CLI built from `9a0f8078`:

```sh
uv run --no-project --with music21==10.5.2 python3 -B \
  scripts/compare_bcmh_keys.py /path/to/BCMH_dataset.zip \
  /path/to/acorde /tmp/bcmh-structure-audit.json --rank-tied-keys
```
