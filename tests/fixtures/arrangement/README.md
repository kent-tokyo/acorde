# Arrangement corpus v1

`corpus-v1.json` contains eight short, original melodies. Four cases form the
development split and four form the holdout split. Meter, authored note pitches,
explicit chords, repeat, pickup and multi-part flags are fixed. The matching core test
constructs scores, adds a chord part and an independent bass part, and checks
validation plus unchanged source parts and playback.

`baseline-v1.json` freezes the results of five explicit pad-and-bass profiles
for every case: held root-position chords with root bass, offbeat chords with
root/fifth bass, arpeggios with approaching bass, and held open chords with root
bass, plus a grouped pulse with an authored final-measure ending. For each
result it records the source
fingerprint, chord and pattern choices, note-onset and duration distributions,
pitch classes, range, a deterministic fingerprint of playback events, and pad
and bass motion in written and expanded playback order. The
core test compares every result with this file. To inspect a candidate baseline
without changing the fixture, run:

```sh
ACORDE_PRINT_ARRANGEMENT_BASELINE=1 cargo test -p acorde-core --test arrangement_corpus --locked -- --nocapture
```

The holdout split is reserved for final comparison and must not be adjusted to
make an algorithm look better. Its structural invariant and snapshot tests run
in CI; these are not listening review or musical-quality scores. The structural
comparison is recorded in `docs/arrangement-corpus-v1-review.md`.

## Long-form development corpus v2

`development-v2.json` contains twelve distinct original source melodies of
16–32 written measures. It covers major/minor, 3/4, 4/4, 5/8, 6/8, 7/8,
a pickup, a local key-signature passage, a written repeat and a two-part source.
Each case has contiguous, measure-addressed form sections and explicit GM
programs in the pinned manifest.
The shared `development_v2.rs` builder gives every score, part and note a
stable ID. Core tests check complete measures, coverage and playback; I/O tests
check MusicXML playback round trips for every case. The authored notes are
development material, not reviewed examples of good music or final holdouts.

To reproduce the source score, MusicXML and matched WAV files, use a new output
directory:

```sh
cargo run -p acorde-cli --example arrangement_v2_source_pack --locked -- <new-output-dir>
```

The example checks the generated SHA-256 manifest against
`development-v2-manifest.json`. It writes the original instrument programs to
the source scores and exports. The bundled small SoundFont has only a piano
preset, so each reference WAV is rendered from an explicitly saved copy with
all programs set to piano 0. The manifest records both JSON identities, the
soundfont hash, renderer settings and WAV hashes. These are matched reference
audio files for pitch/rhythm comparison; they do not evaluate instrument
specific playability. Twelve new source scores are ready for development
review. Six independent final works and listening judgments remain to be
prepared under the v2 evaluation protocol.

The companion proposal pack uses `development_v2_request.rs` to fix one
explicit three-candidate request per work. The cello-led `low-tide` case places
harmony above its low melody; the other cases use a lower pad and bass with an
optional cello countermelody. Run it against a verified source pack:

```sh
cargo run -p acorde-cli --example arrangement_v2_proposal_pack --locked -- <source-pack-dir> <new-output-dir>
```

The generated `index.html` presents the original and three complete proposals
per work at matched tempo and piano timbre. `development-v2-proposals-manifest.json`
pins the requests, candidate identities, MusicXML/WAV SHA-256 and resolved
contracts. Rebuilding twice produced the same manifest. A core regression
checks all 36 proposals for unchanged source parts and playback, distinct
generated events, score validity and replay. The three current proposal
profiles change harmony, bass and density. At least one candidate has a
countermelody in ten of the twelve works; `five-step` and `low-tide` do not.
The pack does not yet establish diverse whole-piece form or instrumentation,
nor musical preference.
