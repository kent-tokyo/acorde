# acorde

> Platform-agnostic music score library for Rust and WebAssembly.

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.77%2B-orange)
![Status](https://img.shields.io/badge/Status-Stable-green)

---

## Overview

**acorde** is a pure-Rust music score library covering the full notation pipeline:
score model · command engine · MusicXML/MIDI/ABC I/O · logical layout · WebAssembly bindings · CLI.

It has zero UI dependencies and no filesystem access in the core crates — renderers and host
applications (desktop, web, server) consume the library and handle I/O at the boundary.

<!-- Drop a screenshot of your score editor (e.g. MusicLav) here. -->
<!-- Replace the placeholder below with the real image path once you add the file. -->
<!-- Example: ![Score editor](docs/assets/screenshot.png) -->

---

## Architecture

### Pipeline

```mermaid
flowchart LR
  subgraph Input["Input formats"]
    MXL[".musicxml / .mxl"]
    MID[".mid"]
    ABC[".abc"]
    MSZ[".mscz / .mscx"]
  end

  subgraph IO["acorde-io"]
    direction TB
    pm["parse_musicxml\nparse_mxl"]
    pmid["parse_midi"]
    pabc["parse_abc"]
    pmscz["parse_mscz\nparse_mscx"]
  end

  subgraph Core["acorde-core"]
    direction TB
    Score(["Score"])
    SE["ScoreEngine\n(apply / undo / redo)"]
    PE["to_playback_events"]
    TX["transpose / validate / diff\nScale::best_fit · roman_numeral"]
  end

  subgraph Layout["acorde-layout"]
    CL["compute_layout"]
    LR(["LayoutResult\n(vis_slots · rows · spans\nbeam_groups · tuplet_groups)"])
  end

  subgraph Output["Output formats"]
    OXL["serialize_musicxml"]
    OMD["serialize_midi"]
    OAB["serialize_abc"]
  end

  MXL --> pm --> Score
  MID --> pmid --> Score
  ABC --> pabc --> Score
  MSZ --> pmscz --> Score

  Score <--> SE
  Score --> PE
  Score --> TX
  Score --> CL --> LR

  Score --> OXL
  Score --> OMD
  Score --> OAB
```

### Score data model

```
Score
├── metadata  { title, composer, lyricist, copyright, … }
├── settings  { tempo_bpm, time_signature, key_signature }
├── part_groups  Vec<PartGroup>
└── parts     Vec<Part>
    ├── midi_channel / midi_program
    └── staves  Vec<Staff>
        ├── clef
        ├── transpose_semitones  (transposing instruments)
        └── measures  Vec<Measure>
            ├── time_sig / key_sig / clef / tempo
            ├── barline_left / barline_right
            ├── volta / rehearsal / navigation / expression_text
            └── voices  [Vec<Note>; 4]
                └── Note
                    ├── pitches       Vec<Pitch>  (chord = multiple pitches)
                    ├── duration / dot_count / tuplet
                    ├── is_rest / is_grace / is_cue
                    ├── tie_start / slur_start / hairpin_start / ottava_start
                    ├── dynamic / articulations / lyric / chord_symbol
                    ├── stem_up / note_head / fingering / technique_text
                    └── guitar_technique / arpeggiate / trill_line_start
```

### Command flow (undo / redo)

```mermaid
sequenceDiagram
  participant App
  participant ScoreEngine
  participant CommandStack
  participant Score

  App->>ScoreEngine: apply(Command::AddNote(…))
  ScoreEngine->>CommandStack: execute(cmd, &mut score)
  CommandStack->>Score: mutate
  CommandStack-->>ScoreEngine: ChangeHint { scope, layout_dirty, playback_dirty }
  ScoreEngine-->>App: ChangeHint

  App->>ScoreEngine: undo()
  ScoreEngine->>CommandStack: undo(&mut score)
  CommandStack->>Score: revert mutation
  CommandStack-->>App: ChangeHint
```

---

## Why acorde?

### Edit history is a first-class citizen
Every mutation is a serializable `Command` enum stored in a `CommandStack`.
This means undo/redo works out of the box, but also that edit history can be persisted to disk,
replayed deterministically, or streamed over a network — without any extra infrastructure.

### The same code runs natively and in the browser
`acorde-core` and `acorde-io` compile to WebAssembly without modification.
A server that parses MusicXML and a browser-based editor can share identical business logic.

### Only import what you actually use
`acorde-io` features are independent flags — `musicxml`, `midi`, `abc`, `mscz`.
`acorde-core` pulls in no I/O crates at all, keeping it embeddable in constrained environments.

---

## Compared to other tools

The music notation ecosystem has several mature players, each optimised for a different use-case.
The table below maps the most commonly reached-for alternatives.

| | acorde | [music21] | [VexFlow] | [OSMD] | [jFugue] |
|---|---|---|---|---|---|
| **Primary language** | Rust | Python | JavaScript | TypeScript | Java |
| **Runs in browser** | ✓ (WASM) | ✗ | ✓ | ✓ | ✗ |
| **Score data model** | ✓ | ✓ | partial¹ | ✗ | ✓ |
| **MusicXML parse + emit** | ✓ | ✓ | ✗ | parse only | ✗ |
| **MIDI parse + emit** | ✓ | ✓ | ✗ | ✗ | ✓ |
| **ABC Notation** | ✓ | ✓ | ✗ | ✗ | ✗ |
| **Undo / redo built-in** | ✓ | ✗ | ✗ | ✗ | ✗ |
| **Serializable edit history** | ✓ | ✗ | ✗ | ✗ | ✗ |
| **Playback event generation** | ✓ | ✗ | ✗ | ✗ | ✓ |
| **Renderer-agnostic layout** | ✓ | ✗ | ✗ | ✗ | ✗ |
| **Music-theory analysis** | ✓ | ✓✓✓ | ✗ | ✗ | basic |
| **No garbage collector** | ✓ | ✗ | ✗ | ✗ | ✗ |
| **Embeddable / no runtime** | ✓ | ✗ | ✗ | ✗ | ✗ |

[music21]: https://web.mit.edu/music21/
[VexFlow]: https://www.vexflow.com/
[OSMD]: https://opensheetmusicdisplay.org/
[jFugue]: http://www.jfugue.org/

¹ VexFlow's object model (`StaveNote`, `Beam`, etc.) is tightly coupled to its SVG/Canvas renderer.
  It is not designed as a standalone data layer that can be serialized or mutated independently.

### When to choose acorde

**acorde is the right fit when you need:**

- A **Rust or WebAssembly** target — native desktop, server-side processing, or a browser editor using the same binary.
- A **full pipeline in one library** — parse MusicXML, run commands, export MIDI, compute layout hints, generate playback events — without gluing together separate packages.
- **Built-in undo/redo and crash recovery** — every mutation is a serializable `Command`. History can be persisted and replayed deterministically; no separate state-management framework required.
- **AI or batch mutation** — `batch_apply_labeled()` applies an arbitrary sequence of commands as a single undoable step, making AI-assisted score editing straightforward.
- **Renderer independence** — `LayoutResult` returns logical coordinates (row/column slot indices, resolved span endpoints) rather than pixels, so you can drive VexFlow, a canvas renderer, or a native UI from the same data.

### When to choose something else

- **Deep music-theory analysis** (Roman numerals, voice-leading, corpus research): reach for **music21**. Its analysis toolkit is unmatched and Python's ecosystem is ideal for research notebooks.
- **Rendering only, no mutation**: if you just need to display a static MusicXML file in a browser, **OSMD** has a polished out-of-the-box experience.
- **JavaScript without a build step**: **VexFlow** or **abc.js** can be dropped into a `<script>` tag; acorde requires a WASM build pipeline.
- **JVM ecosystem**: **jFugue** is the natural choice for Java/Kotlin projects.

---

## Workspace

```
acorde/
  Cargo.toml              # workspace
  crates/
    core/                 # Score model + ScoreEngine (no I/O, no layout)
    io/                   # MusicXML / MIDI / ABC parsers & serializers
    layout/               # Logical layout engine
    wasm/                 # wasm-bindgen bindings
    cli/                  # Format-conversion CLI
  tests/
    fixtures/             # Sample .musicxml / .mid / .abc files
```

---

## Crates

### `acorde-core`

Score data model and command engine. Zero I/O, zero layout.

```rust
use acorde_core::{
    Score, ScoreEngine, Command,
    SetTempoCmd, SetMidiInstrumentCmd, SetTransposeCmd, SetTempoAtMeasureCmd,
    PasteVoiceCmd, transpose, to_playback_events, measure_sequence, program_name, drum_name,
};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))?;
engine.undo()?;
engine.redo()?;

// Transpose the score up a perfect fifth (7 semitones)
let transposed = transpose(engine.score(), 7);

// Mark staff as Bb instrument (written C4 → concert Bb3)
engine.apply(Command::SetTranspose(SetTransposeCmd {
    part_index: 0, staff_index: 0, semitones: -2,
}))?;

// Change MIDI channel and program for a part
engine.apply(Command::SetMidiInstrument(SetMidiInstrumentCmd {
    part_index: 0, midi_channel: 1, midi_program: 40, // Violin
}))?;

// Set a tempo change starting at measure 4
engine.apply(Command::SetTempoAtMeasure(SetTempoAtMeasureCmd {
    measure_index: 3, bpm: Some(160),
}))?;

// Merge two scores (append parts, pad shorter one with empty measures)
let combined = score_a.merge(&score_b);

// Compute playback events for audio engines (Tone.js, Web Audio API, etc.)
use acorde_core::PlaybackOptions;
let events = to_playback_events(engine.score(), &PlaybackOptions {
    bpm_override: None,
    muted_parts: vec![],
});
// Each PlaybackEvent: time_beats, time_secs, pitch_midi, velocity, duration_beats, duration_secs, part_index
// pitch_midi includes Staff.transpose_semitones; time_secs is correct across tempo changes

// Copy and paste a voice (clipboard lives in ScoreEngine; paste is undo-able)
engine.copy_voice(0, 0, 0, 0)?; // copy part 0 / staff 0 / measure 0 / voice 0
engine.paste_voice(0, 0, 1, 0)?; // paste to measure 1

// General MIDI program and drum name lookup
assert_eq!(program_name(40), "Violin");
assert_eq!(program_name(0),  "Acoustic Grand Piano");
assert_eq!(drum_name(38),    "Acoustic Snare");

// ChangeHint — skip redundant recomputes
let hint = engine.apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))?;
// hint.scope == ChangeScope::Global
// hint.layout_dirty == false  (tempo doesn't affect measure layout)
// hint.playback_dirty == true
```

**Public types:** `Score` · `Part` · `Staff` · `Measure` · `Note` · `Pitch` · `Step` ·
`Duration` · `Clef` · `KeySignature` · `TimeSignature` · `Dynamic` · `Articulation` ·
`Barline` · `HairpinKind` · `OttavaKind` · `Lyric` · `ChordSymbol` · `NoteHead` · `GuitarTechnique` ·
`PartGroup` · `PartGroupSymbol` · `ScoreTemplate` ·
`ScoreEngine` · `Command` · `CommandStack` · `ScoreStats` · `PlaybackEvent` ·
`Interval` · `IntervalQuality` · `Scale` · `ScaleKind` ·
`ValidationError` · `ValidationWarning` · `ValidationReport`

**Commands (53):** `AddNote` · `AddPitch` · `DeleteNote` · `AddMeasure` · `DeleteMeasure` ·
`SetTempo` · `NewScore` · `AddHairpin` · `ToggleTie` · `SetDynamic` · `ToggleArticulation` ·
`SetKeySignature` · `SetTimeSignature` · `SetBarline` · `AddPart` · `DeletePart` ·
`SetMetadata` · `SetRehearsalMark` · `SetNavigationMark` · `SetChordSymbol` · `SetGrace` ·
`SetOttava` · `SetLyric` · `SetMultiRest` · `AddPedal` · `SetVolta` · `SetClef` ·
`SetPartName` · `SetMidiInstrument` · `SetTranspose` · `SetTempoAtMeasure` · `PasteVoice` ·
`PasteRange` · `SetSystemBreak` · `SetPageBreak` · `ToggleSlur` · `AddStaff` · `DeleteStaff` ·
`SetTuplet` · `RespellScore` · `RespellScoreToKey` · `SetStem` · `SetArpeggio` ·
`SetTechniqueText` · `SetFingering` · `SetStringNumber` · `SetNoteHead` · `SetCue` ·
`SetGuitarTechnique` · `SetExpressionText` · `ToggleTrillLine` · `SetPartGroup` · `Batch`

**Functions:** `transpose(score, semitones)` · `to_playback_events(score, options)` ·
`measure_sequence(score)` · `validate(score)` · `Score::statistics()` ·
`Score::extract_part(n)` · `Score::merge(other)` · `Score::diff(a, b)` ·
`program_name(n)` · `drum_name(n)` · `interval_between(p1, p2)` ·
`detect_chord(pitches)` · `roman_numeral(chord, key)` · `Scale::best_fit(pitches)`

**ChangeHint types:** `ChangeHint` · `ChangeScope` (`Global` / `Part(usize)` / `Measures{…}`)

### `acorde-io`

Feature-gated parsers and serializers. Never touches the filesystem.

| Feature | Default | Content |
|---------|---------|---------|
| `musicxml` | ✓ | MusicXML + MXL parser, MusicXML serializer |
| `midi` | ✓ | MIDI parser + serializer |
| `abc` | — | ABC Notation parser + serializer |
| `mscz` | — | MuseScore .mscz/.mscx parser |

```rust
use acorde_io::{parse_musicxml, serialize_musicxml, parse_midi, serialize_midi};
use acorde_io::{parse_abc, serialize_abc}; // requires feature = "abc"
use acorde_io::{parse_mscz, parse_mscx};  // requires feature = "mscz"

let score = parse_musicxml(xml_str)?;
let xml   = serialize_musicxml(&score)?;
// <midi-instrument> channel/program and <transpose><chromatic> survive round-trips

let score = parse_midi(midi_bytes)?;
let midi  = serialize_midi(&score)?;
// Part.midi_channel / Part.midi_program restored from ProgramChange events on import
// → Vec<u8> (SMF Type 1, PPQ = 480)
// Staff.transpose_semitones is applied to all MIDI note pitches
// Per-measure Tempo meta events are emitted when Measure.tempo is set

let score = parse_mscz(mscz_bytes)?;   // .mscz (compressed archive)
let score = parse_mscx(mscx_str)?;     // .mscx (raw XML)
// Imports: pitches (TPC), durations, rests, ties, slurs, dynamics, lyrics,
//          repeat barlines, volta brackets, MuseScore 3.x and 4.x formats
```

### `acorde-layout`

Logical layout computation — no pixel values, no CSS.

```rust
use acorde_layout::{LayoutConfig, compute_layout};

let config = LayoutConfig {
    measures_per_row: 4,
    concert_pitch: false,
    first_row_measures: None, // override first row count if needed
};
let result = compute_layout(&score, &config);
// result.vis_slots           — visual column → physical measure index (multi-rest aware)
// result.rows                — measures per row/system
// result.spans               — hairpin / pedal / ottava start+end indices resolved
// result.concert_key_overrides — per-staff key sig for transposing instruments in concert pitch
// result.beam_groups         — note index groups for beam rendering
// result.tuplet_groups       — note index groups with actual/normal note counts for tuplets
// result.courtesy_accidentals — accidentals that must be shown as a courtesy to the player
```

### `acorde-wasm`

wasm-bindgen bindings. Build with `wasm-pack build`.

```bash
wasm-pack build crates/wasm --target bundler
```

Exposes: `parse_musicxml` · `parse_mxl` · `serialize_musicxml` · `parse_midi` · `serialize_midi` ·
`serialize_midi_region(score_json, start, end)` · `parse_mscz` · `parse_mscx` ·
`parse_abc` · `serialize_abc` ·
`to_playback_events(score_json, bpm, muted_parts_json)` · `to_playback_events_ex(score_json, options_json)` ·
`compute_playback_position(score_json, time_secs)` ·
`compute_layout(score_json, measures_per_row, concert_pitch)` · `compute_layout_ex(score_json, config_json)` ·
`gm_program_name(n)` · `gm_drum_name(n)` ·
`validate_score` · `transpose_score` · `extract_part` · `merge_scores` · `diff_scores` ·
`score_statistics` · `score_duration_secs` · `score_duration_secs_region` ·
`respell_score(score_json, prefer_flat)` · `respell_score_to_key` ·
`measure_beats_remaining` · `pitch_from_midi(midi, prefer_flat)` · `pitch_from_str` ·
`interval_between(pitch1_json, pitch2_json)` ·
`key_alter_for_step` · `key_contains_pitch` · `key_display_name` ·
`clef_middle_line_midi` · `suggested_stem_up(pitches_json, clef_json)` ·
`compute_beams(notes_json, time_sig_json)` · `command_key_from_json` ·
`detect_chord(pitches_json)` · `roman_numeral(chord_json, key_json)` · `best_fit_scale(pitches_json)` ·
`ScoreEngine` JS class:
  `apply(cmd_json)` → `ChangeHint` JSON · `undo()` / `redo()` → `ChangeHint` JSON ·
  `apply_batch(cmds_json)` · `apply_batch_labeled(cmds_json, label)` ·
  `copy_voice` / `paste_voice` · `copy_range` / `paste_range` ·
  `get_undo_label()` / `get_redo_label()` · `get_undo_key()` / `get_redo_key()` ·
  `export_history()` / `restore_history(json)` · `replace_score` · `get_score` · `get_version`

### `acorde-cli`

```bash
cargo install acorde-cli
```

```bash
acorde convert  input.mid output.musicxml
acorde convert  input.musicxml output.mid
acorde convert  input.mscz output.musicxml    # requires --features mscz build
acorde info     input.musicxml          # title, parts, measures, notes, duration
acorde validate input.musicxml          # structural validation, exits 1 on error
acorde extract  --part 0 input.musicxml violin.musicxml
```

---

## Getting Started

Add `acorde-core` to your `Cargo.toml`:

```toml
[dependencies]
acorde-core = "0.1"
```

For I/O support:

```toml
acorde-io = "0.1"

# ABC Notation support (opt-in)
acorde-io = { version = "0.1", features = ["abc"] }

# MuseScore .mscz/.mscx support (opt-in)
acorde-io = { version = "0.1", features = ["mscz"] }
```

---

## Building

**Prerequisites:** Rust 1.77+

```bash
git clone https://github.com/kent-tokyo/acorde.git
cd acorde
cargo build --all
cargo test --all
cargo clippy --all -- -D warnings
```

For WebAssembly:

```bash
cargo install wasm-pack
wasm-pack build crates/wasm --target bundler
wasm-pack test crates/wasm --headless --chrome
```

---

## Design Constraints

| Rule | Applies to |
|------|-----------|
| No async runtime (`tokio`) | core · io · layout |
| No `std::fs` | core · io · layout |
| No pixel values, CSS, or renderer-specific types | all crates |
| `core` must not depend on `io` or `layout` | core |
| No `panic!` / `unwrap` in public paths | all crates |

---

## Testing

```bash
cargo test --all                           # unit + integration tests (467 tests)
cargo test -p acorde-io --features abc   # ABC parser + serializer tests
cargo test -p acorde-io --features mscz  # MSCZ parser tests (69 unit + 28 roundtrip)
```

Every parser has a round-trip integration test in `crates/io/tests/roundtrip.rs`.
Passing 0-byte or garbage data to any parser returns `Err`, never panics.

---

## License

acorde is dual-licensed under **MIT** or **Apache-2.0**, at your option — see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
