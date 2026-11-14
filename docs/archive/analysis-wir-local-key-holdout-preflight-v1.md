# When in Rome local-key label holdout: preflight and frozen protocol

The [selection](analysis-wir-local-key-holdout-selection-v1.json) pins 12
score/annotation path pairs from the complete When in Rome Git tree at
`1c61fe41b8c2910296d7d2bcbf6476c7c1f2fe35`. The selector reads only
the tree index and three earlier work lists. It chooses three works each from
Beethoven Op.59 quartets, Bach WTC I numbers 10–12, Mozart K309–311 sonatas,
and three distinct Louise Reichardt song collections. None overlaps the 36
works in the previous three When in Rome cohorts. Some Mozart scores were
used for a separate global-key task, so this is a **local-key label holdout**,
not an unseen-score claim.

The [score-only source record](analysis-wir-local-key-holdout-score-hashes-v1.json)
verifies all 12 MXL files against the selection's Git blob hashes. The
[prepared-input record](analysis-wir-local-key-holdout-inputs-v1.json) removes
2,359 embedded `<harmony>` answer elements. It records source and prepared
SHA-256 values. The score-only manifests contain no annotation SHA-256, and
the local source directory contains no `analysis.txt` files. The selected
annotation Git blob IDs are metadata from the tree, not annotation content.

The [input-only preflight](analysis-wir-local-key-holdout-preflight-v1.json)
verifies prepared hashes and absence of `<harmony>`, then parses every score
with Acorde and music21 10.5.2. Both parse **12/12**. Pitched onset,
duration, and MIDI-pitch multisets agree in all **1,462 physical measures**;
there are no predeclared unequal-timeline exclusions. It does not read
annotations or evaluate either local-key prediction.

The [protocol](analysis-wir-local-key-holdout-protocol-v1.json) freezes the
Acorde binary and radius four, music21 10.5.2 implementation and window two,
score inputs, preflight, evaluator source and its helper scripts before any
held-out label is read. The evaluator was smoke-tested on previously inspected
development labels from a keyboard work and a quartet. The primary score is
exact tonic spelling and major/minor mode at aligned RomanText annotation
positions; unavailable predictions count as wrong. It reports annotation
exclusions, prediction coverage, and per-work counts. The preregistered gate
requires all 12 works, no more than 5% excluded annotations, no lower Acorde
coverage, more Acorde exact labels on paired positions, and at least seven
Acorde work wins. A failed gate remains a failed holdout; tuning afterward
requires a new disjoint cohort.

At this checkpoint the 12 reserved `analysis.txt` files have **not been
retrieved or opened**. The [scorer](../../scripts/score_wir_local_key_holdout.py)
is ready, but its result is not part of this preflight.
