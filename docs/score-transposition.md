# Whole-score transposition

Use `transpose_checked(&score, semitones)` in Rust or
`transpose_score(score_json, semitones)` in viewer/full WASM. The validated
operation returns a new score. Hosts should render, play and export that same
returned score, and derive every requested offset from the immutable original
(or selected arrangement), rather than repeatedly transposing the previous result.

Whole-score transposition updates global/local key signatures, pitched notes,
structured chord roots and slash bass notes. Chord quality, degrees, placement,
analysis fields, rhythm, repeats, ties and note identities are retained. The
spelled interval follows the source/destination key signatures; octave shifts
preserve the original spelling and key signature. Unpitched percussion is unchanged.

The checked boundary rejects MIDI pitch overflow and unsupported chord-root
spellings. It also rejects tablature/fret or harp-pedal rewriting that needs a
separate instrument-aware transformation. It does not silently clamp a checked
result or certify physical playability for a particular instrument. The legacy
unchecked `transpose` and staff-region operation are not substitutes for this
validated whole-score path.

The UI must validate its numeric offset before the WASM i8 boundary. For example,
a score-page control can allow integer offsets from -12 to +12 semitones. To
restore the original key and spelling exactly, restore the retained source score.
