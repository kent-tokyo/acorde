# Shared kick and bass rhythm

`arrange_rhythm_section` appends a bass part and a kick part to a source score. `replay_rhythm_section` checks the source fingerprint, engine/contract versions and saved bass choices before returning the same musical result. Both are exposed by Rust and the full/viewer WASM builds. The playback-only profile does not expose arrangement APIs.

## Request and behavior

A `RhythmSectionRequest` supplies an explicit `ChordPlan`, editable measure range, locks, bass register, MIDI program, leap bound, ghost-sound capability and `RhythmSectionBar` entries. Each bar contains consecutive slots in 1/32-quarter-note ticks. Slots sum to the actual bar length, including pickups and meter changes. A slot separately specifies optional kick/bass attacks, their holding lengths and accents; absent attacks are rests. Holds must fit inside their slot. Cross-slot/cross-bar holds are outside this first contract.

`synchronize: true` requires both attacks at the slot start. Other slots can contain either part alone. Output playback is checked against every performed bar visit and the exact slot offsets, so matching written positions alone do not count as verified synchronization. Generated notes use explicit dynamics and tenuto; held sounds split by notation are tied, and unused slot time remains a rest.

Bass tones can use the spelled root, slash bass, third or fifth of the active expanded harmony. An approach supplies its target attack, direction and distance (one or two semitones). The target must be a normal, non-approach bass attack. Target register selection reserves room for its incoming approaches. Every performance of an approach must be followed by the specified target in the actual repeat/navigation order, and the contract saves the spelled target pitch and resolution count. Ambiguous repeat destinations, impossible registers and excessive performed leaps return typed errors without a partial score. Register selection is deterministic and greedy; a leap error does not prove that no alternate global voicing exists.

Source parts, note IDs and authored harmony stay intact. Omitted bars are filled with rests in the added parts. Supplying a locked or out-of-range bar is an error even when its slots contain only rests. Planned bars must occur in playback. Existing use of percussion channel 9 is rejected instead of colliding with an authored drum part or controller stream. Melodic channels are allocated using the existing channel-occupancy rules.

## Ghost sound

Ghost notes have parenthesized noteheads, explicit `pp` velocity and at most eight ticks (one sixteenth note in 4/4). Accented ghosts are rejected. Ghost pitches do not steer the subsequent normal bass register. Normal attacks explicitly restore `mf`; a ghost cannot reduce the following normal note's level.

- `ShortQuiet` retains the bass program and reports `ShortQuietGhostApproximation`. This is a short, quiet pitched approximation.
- `MutedProgram` carries a caller-supplied program and sound identity. Ghosts use a separate part/channel so MusicXML and MSCZ preserve their routing. The report says `CallerSuppliedMutedSound`: the caller must know that the renderer has the requested muted patch. The core does not inspect an audio device or SoundFont.

The kick uses GM percussion key 36 on channel 9. MSCX import/export now preserves explicit monophonic Drum definitions and their playback key, name, staff position, notehead and voice. Instrument IDs are reconstructed; named techniques and mixed-drum chords are not a lossless notation contract. Export diagnostics retain the identity/technique limitation. The declaration format was checked against [MuseScore's Drumset implementation](https://github.com/musescore/MuseScore/blob/v3.6.2/libmscore/drumset.cpp).

## Bounds and replay

The API accepts at most 32 measures, 29 source parts, four staves per part, 8,192 source notes, 64 chord entries, 64 slots per selected bar and 2,048 generated attacks. Source staves must agree on measure lengths. Keys/programs remain in MIDI range. Ghost sound identities are limited to 256 bytes. WASM requests are limited to 2 MiB and resolved contracts to 8 MiB in addition to the existing score JSON limit. Invalid fractions, missing thirds/fifths and unsupported chord expansion are typed failures.

The resolved contract contains the exact request, chosen bass pitches and approach targets. Replay recomputes these deterministic choices and compares them to the saved values. Fingerprints detect stale or mismatched input; they are not authorization signatures.

## Reproducible audition

```
cargo run -p acorde-cli --example rhythm_section_listening_pack -- /new/output-directory
```

The fixed 8/16-bar corpus compares root pulse, coordinated attacks with approaches, short/quiet ghosts, and a dedicated muted patch. The source harmony and melody stay constant. The generator checks actual playback, replay, MusicXML/MSCZ round trips, distinct rendered WAVs and absence of silent/clipped renders. Its manifest records input/settings/artifact hashes, source commit and dirty status, sound identity and pending listening review.

`rhythm-audition.sf2` contains original synthetic keys, bass, muted bass and kick samples, authored by `scripts/build_rhythm_audition_font.py` under the repository license. They are audition sounds, not acoustic instrument models. All comparisons use the same font, tempo, gain and 22,050 Hz mono renderer, with no individual normalization. Run `python3 scripts/build_rhythm_audition_font.py tests/fixtures/arrangement/rhythm-audition.sf2` to rebuild the fixture.

The user accepted all presented stage-5 eight/sixteen-bar comparisons on 2026-11-17 with "問題なし". Together with the structural, replay, I/O, native/full/viewer and hosted checks, this completes stage-5 development acceptance. The pinned listening manifest and `harmony-targets-rhythm-progress.json` record the scope. No per-criterion numeric ratings or held-out multi-listener result is inferred; the draft PR remains unmerged.

## Generation measurements

[`arrangement-stage45-performance.json`](arrangement-stage45-performance.json)
records 60 measured native release calls per variant for the fixed eight/sixteen-bar
inputs, after warmup. The largest observed p95 was 0.804 ms (sixteen-bar muted
ghosts). Each call preserved original parts and reproduced the same playback and
slot/attack counts. Rhythm generation uses deterministic construction, so a
candidate-search count does not apply. Preparing the harmonized source, playback
verification, JSON, rendering and result destruction are outside the timing.
This is a one-host development measurement, not a browser or maximum-input
latency guarantee. The matched legacy baseline and reproduction command are
described in [`harmony-targets.md`](harmony-targets.md).

The eight rhythm variants in `stage45-native-parity.json` retain native release
playback events, resolved bass pitches, approach targets, synchronized addresses
and ghost diagnostics. The native and Chrome full/viewer parity tests compare
both fresh generation and replay with these fixed values, including the original
source parts. The expected events are produced by the native harness, rather
than another core call compiled to WASM.
