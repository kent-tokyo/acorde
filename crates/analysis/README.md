# acorde-analysis

Deterministic, explainable analysis primitives for [`acorde-core`](https://docs.rs/acorde-core).
The crate has no I/O, renderer, or host dependencies.

The capability slice labels chord-shaped pitch collections, records adjacent melodic intervals,
and estimates major/minor keys from diatonic pitch coverage. Every result contains stable
`NoteAddr` evidence and a rule identifier. Key estimation returns all tied best candidates, so
relative-major/minor ambiguity is preserved instead of inventing a single key.

```rust
use acorde_analysis::analyze_chords;
use acorde_core::Score;

let result = analyze_chords(&Score::default());
assert!(result.chords.is_empty());
```
