# Three-style global-key holdout: source selection

The [selection](analysis-global-key-holdout-selection-v1.json) fixes twelve
scores before reading their key labels or either analyzer's predictions:
four Bach chorales (Riemenschneider 200–203), four C. P. E. Bach keyboard
pieces, and four movements from two Pleyel quartets. The chorales are outside
the 001–101 BCMH key-development set. The selection script reads only source
file IDs and score paths from the pinned Git tree and metadata tables; it does
not inspect `annotated_key` values or rows of the Bach key index. Each score
has a recorded Git blob at a pinned repository commit.

The proposed chorale reference is the Riemenschneider-numbered
[Bach chorale key index](https://bach-chorales.com/ChoraleKeyIndex.htm), whose
downloaded HTML has SHA-256 `d8849022d85f59ade61d84e911fd439fe8a42ad920c9a7748129c08863487d5e`.
The corresponding [When in Rome](https://github.com/MarkGotham/When-in-Rome)
human RomanText analysis is a second source for checking the opening tonic.
The key index describes original manuscript keys, which may differ from a
chorale collection's transposition. Before the selected labels are read, the
scoring protocol must specify how to handle a transposition, an ambiguous
tonality, or disagreement between the two references. A score whose key
reference cannot be matched to its actual pitches cannot count as an ordinary
wrong analyzer prediction.

For keyboard and chamber music, the reference is `annotated_key` in the pinned
[C. P. E. Bach](https://github.com/DCMLab/cpe_bach_keyboard) and
[Pleyel](https://github.com/DCMLab/pleyel_quartets) metadata. Their MuseScore
3 scores still require a deterministic MusicXML export and removal of any
embedded analyst harmony labels. Both programs must receive the same cleaned
MusicXML bytes. The score export, two-program parse and pitched-event checks,
label projection, exclusions, and success thresholds remain to be frozen.

This is a source selection, not a global-key accuracy result or a completed
holdout. The selection must not be used to choose an algorithm. If labels from
these twelve scores inform tuning, a new disjoint set is required for the
final gate.
