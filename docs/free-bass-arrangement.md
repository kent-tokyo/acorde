# Lossless single-staff free-bass candidates

`split_for_free_bass(score, FreeBassOptions)` (Rust and WASM) assigns an existing
polyphonic staff, including chord members, to right- and left-hand staves. It
preserves pitches, note lengths, rests, tuplets and per-pitch ties. A pitch-level
source map identifies every retained pitch. It does not invent accompaniment,
transpose, remove notes, or claim that a standard-bass instrument can play the result.

The default screening profile is right hand MIDI 48–96, left hand 36–84, up to
12 semitones of simultaneous span, and at most 5/4 distinct pitches. These are
configurable **screening limits**, not a specification of every accordion.
Instrument range, C/B button layout or piano keys, fingering, repeated-note speed,
hand jumps, registers and bellows still require a player's review. Successful
results always say `required_bass_mode: "free-bass"` and
`requires_performer_review: true`.

Each measure tries pitch boundaries nearest the previous boundary (initially
MIDI 60). Sustained notes in other voices count towards simultaneous span and
pitch count. This is a conservative, incomplete search: `no-supported-assignment`
means this algorithm did not supply a candidate under the requested limits; it
does not mean the music is impossible to arrange. A standard-bass adaptation may
need a separately authorized harmonic reduction, changed voicings or omitted
inner voices. This API never performs that reduction implicitly.

An explicit tie check and a final sounding-event comparison reject a split that changes a cross-bar tie
or repeat playback. The search does not backtrack to repair cross-bar ties.
Unsupported cross-hand notation, pedal, grace notes, tablature, instrument
routing and score-address annotations are rejected explicitly. Source-instrument
fingerings are removed: they are not valid accordion fingering instructions.
Lyrics remain on the upper staff. Neither notation identity nor engraving is
claimed; the output is a new two-staff arrangement for review.

The CLI example accepts score JSON and emits the complete result:

```sh
cargo run -p acorde-core --example free_bass_json < source.json > result.json
```

A `null` score is never a partial arrangement. Hosts must show diagnostics and
must not interpret an unavailable candidate as proof of instrumental impossibility.

Inputs are limited to 4096 measures and 512 pitch/rest entries per measure to
bound the overlap screening work. Larger inputs return `input-limit`.

## WASM size evidence

Linux CI's full web artifact measured 6,467,136 bytes at baseline
`fa05ace027cb1a5f5369121e17b2b44eac446fbd` (run 41390407443,
job 125100318558) and 6,514,678 bytes for the implementation at
`4ea0fd439e162f52ad423aea0e0fc17f6d5a20a5` (run 41392194497,
job 125105806769). The observed increase is 47,542 bytes (0.74%).
The full-WASM regression cap is 6,521,000 bytes, preserving approximately
6 KB of headroom. This is an explicit budget change for the new exported API;
it is not evidence of reduced transfer size or faster startup.
