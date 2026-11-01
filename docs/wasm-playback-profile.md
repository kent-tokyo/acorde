# Playback-only WASM build

Original-score-image pages can load a small Acorde module when the visitor asks
to play. Build it separately from the default `full` and opt-in `viewer` modules:

```sh
wasm-pack build crates/wasm --target web --release \
  --out-dir ../../target/wasm-profiles/playback-only \
  --no-default-features --features playback-only
```

The profile exports `parse_mscz` and `to_playback_events_ex`. Its generated JS
also exports the ordinary WASM initializer. `parse_mscz` returns canonical score
JSON; `to_playback_events_ex` accepts the same `PlaybackOptions` JSON and returns
the same bounded events as the other profiles. The full and viewer profiles keep
their existing bindings, including the viewer's SVG, beam, chord, ABC, and MSCZ
export functions. When `playback-only` is combined with `full` or `viewer`, the
larger profile takes precedence; use `--no-default-features` for the small build.

The playback-only module has no SVG/layout, ABC, analysis, score editing,
SoundFont, or format-export bindings. It supplies scheduling data, not audio
synthesis. Hosts that display Acorde's engraving or edit a score should use the
viewer or full profile.
The Cargo package still compiles its existing layout, SVG, and analysis
dependencies; unused code is omitted from the optimized playback-only WASM
binary. The size figures below measure the deployable `.wasm` file, not Rust
build time or disk usage.

## Reproduce the comparison

Build all three profiles from the same source and toolchain:

```sh
wasm-pack build crates/wasm --target web --release \
  --out-dir ../../target/wasm-profiles/full
wasm-pack build crates/wasm --target web --release \
  --out-dir ../../target/wasm-profiles/viewer \
  --no-default-features --features viewer
wasm-pack build crates/wasm --target web --release \
  --out-dir ../../target/wasm-profiles/playback-only \
  --no-default-features --features playback-only
node scripts/check-wasm-playback-profile.mjs > target/wasm-playback-profile-report.json
```

The checker reads the generated TypeScript declarations, measures raw WASM,
gzip level 9, and Brotli quality 11 bytes, and compares event timing, MIDI
pitch, duration, part index, and score address for the same MSCZ bytes in all
three modules. It fails on a missing binding or an event difference. The browser
test also checks rejection of malformed inputs in the playback-only build.

Measured on 2026-10-30 from source commit
`2a2356177cd082b12a5fbde799f7a3d9837f32bd` (after PR #153), on macOS
arm64 with Rust 1.97.0, `wasm-pack` 0.15.0, `wasm-opt` 130, and Node 24.5.0.
All rows use the same release options and compression settings:

| Profile | Raw bytes | gzip bytes | Brotli bytes | Application bindings |
| --- | ---: | ---: | ---: | ---: |
| Full | 5,556,377 | 1,788,297 | 1,216,208 | 103 |
| Viewer | 4,535,027 | 1,424,799 | 961,910 | 81 |
| Playback-only | 904,828 | 343,017 | 268,752 | 2 |

The playback-only Brotli file is 693,158 bytes (72.1%) smaller than the viewer
file in this build. This is a transfer-size comparison, not a cold-start timing
measurement or a claim about audio rendering quality.

The checker used a two-bar tune and a generated 4-part, 8-staff, 32-bar score.
The tiny archive was 854 bytes (SHA-256
`0d9b498aa5df4281feb7b073aea7630ff8afb7c126960be1d9789b86cf54fa0d`)
and produced 16 events. The dense archive was 3,260 bytes (SHA-256
`da4dcce9f1e476b1e9e135c2caa555dbdd3c81d43b9033825676de60f86ab0a9`)
and produced 2,048 events. All compared event fields matched exactly across
the three profiles for both inputs.

## Shared dynamics across staves

Dynamics and hairpins are staff-local by default. To share a particular written mark with
all staves of its own part, provide its zero-based note address in `shared_dynamics` or,
for the first note of a hairpin, in `shared_hairpins`:

```json
{
  "shared_dynamics": [{"part": 0, "staff": 0, "measure": 0, "voice": 0, "note": 0}],
  "shared_hairpins": [{"part": 0, "staff": 0, "measure": 0, "voice": 0, "note": 0}]
}
```

Pass this object as `options_json` to `to_playback_events_ex`. The two lists are independent:
a dynamic and hairpin on the same note must each be selected if both should be shared.
Unselected markings remain local. At the same time, a staff-local dynamic takes precedence
over a shared mark on that staff; a local hairpin likewise takes precedence over an overlapping
shared hairpin. A note's own explicit dynamic retains its attack. Repeated measures use the
same physical addresses. Addresses that do not point to the relevant
mark have no effect; hosts should update them after score edits.

This choice belongs to playback options, not the score or MusicXML/MuseScore import: those
formats do not automatically imply that every marking between staves is shared. The host must
persist its choice separately. Rendering does not duplicate or modify the mark. The offline
render manifest uses the same playback options; the current MIDI serializer has no option
parameter and continues to export staff-local dynamics.
