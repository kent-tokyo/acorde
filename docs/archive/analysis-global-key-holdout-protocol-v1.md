# Three-style global-key holdout: frozen scoring protocol

The [protocol](analysis-global-key-holdout-protocol-v1.json) pins the
[selection](analysis-global-key-holdout-selection-v1.json),
[common inputs](analysis-global-key-holdout-inputs-v1.json),
[input-only preflight](analysis-global-key-holdout-preflight-v1.json), Acorde
binary, scorer and its support scripts, music21 10.5.2 key-analysis source,
and lxml 6.0.2. The selected key-label values were not inspected when this
protocol was written.

One score receives one exact tonic-spelling and major/minor-mode vote. Acorde
uses the first candidate from `analyze --aarden-essen-keys`; music21 uses
`score.analyze('key')`. Prediction failures count as wrong and reduce coverage.
The scorer also reports pitch-class/mode agreement and per-style results.

For Bach chorales, the reference is the pinned Riemenschneider key-index row.
The pinned When in Rome human RomanText opening key must agree. A missing or
duplicate row, an asterisk indicating a possible collection transposition,
ambiguous markup, unsupported key, missing opening key, or disagreement
excludes that score with a recorded reason. The other two styles use the
`annotated_key` field from their pinned DCML metadata, where an uppercase
tonic denotes major and lowercase denotes minor. Missing or unsupported
metadata keys are recorded exclusions.

The gate requires all twelve scores in the source selection, at most one
reference exclusion, at least three eligible scores in each style, no lower
Acorde prediction coverage, more Acorde exact matches on scores where both
tools return a key, and no worse Acorde exact count in any style. These
thresholds and the scorer are fixed before the selected values are read.
A failure stays in the record; any change informed by these labels needs a
new disjoint holdout.
