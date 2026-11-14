# Three-style global-key holdout: score-only preflight

The [selection](analysis-global-key-holdout-selection-v1.md) was committed as
`57ac5bb7` before any selected key-label value was inspected. The
[score-source hashes](analysis-global-key-holdout-source-hashes-v1.json) verify
all twelve pinned Git blobs. The [input record](analysis-global-key-holdout-inputs-v1.json)
retains each exported and cleaned MusicXML SHA-256. Four chorale MXL files
contained one MusicXML score each. MuseScore 4.7.5 exported the eight DCML
MSCX scores. It returned status `-6` after writing each complete, parseable
MusicXML file; every output was checked and hashed. The input preparer removed
805 `<harmony>`, 490 `<direction>`, and 56 `<credit>` elements. It made no
syntax repairs. Both analyzers receive the identical cleaned bytes per score.

The [parser preflight](analysis-global-key-holdout-preflight-v1.json) used
music21 10.5.2 and the pinned Acorde CLI binary. Both parsed **12/12** inputs.
Their pitched onset, duration, and sounding-MIDI multisets agreed in every
one of **496 physical measures**. The Acorde import diagnostics concern only
unsupported placement/render attributes on the exported keyboard and quartet
scores; the diagnostic codes and counts are retained per score. This check
does not establish voice placement, rests, spelling, ties, or global-key
accuracy.

The selected values in the DCML `annotated_key` columns and the Bach key-index
rows have not been inspected. The chorale-reference reconciliation rule,
label parser, comparator implementation, exclusions, and success thresholds
must be frozen before reading those selected labels.
