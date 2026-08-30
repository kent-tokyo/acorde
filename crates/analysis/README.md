# acorde-analysis

Deterministic, explainable analysis primitives for [`acorde-core`](https://docs.rs/acorde-core).
The crate has no I/O, renderer, or host dependencies.

The first capability slice labels chord-shaped pitch collections in each score voice. Every label
contains stable `NoteAddr` evidence, a rule identifier, confidence, and an optional Roman numeral
in the active key. A missing label is intentional: the analyzer does not invent a chord when the
existing templates do not provide an unambiguous match.

```rust
use acorde_analysis::analyze_chords;
use acorde_core::Score;

let result = analyze_chords(&Score::default());
assert!(result.chords.is_empty());
```
