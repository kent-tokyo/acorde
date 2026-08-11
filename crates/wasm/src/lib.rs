use wasm_bindgen::prelude::*;
use acorde_core::{Command, Score};

// ── helpers ───────────────────────────────────────────────────────────────────

fn js_err(msg: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&msg.to_string())
}

fn score_from_json(json: &str) -> Result<Score, JsValue> {
    serde_json::from_str(json).map_err(|e| js_err(format!("invalid score JSON: {e}")))
}

fn score_to_json(score: &Score) -> Result<String, JsValue> {
    serde_json::to_string(score).map_err(|e| js_err(format!("score serialization failed: {e}")))
}

// ── MusicXML ──────────────────────────────────────────────────────────────────

/// Parse a MusicXML string and return the score as a JSON string.
#[wasm_bindgen]
pub fn parse_musicxml(xml: &str) -> Result<String, JsValue> {
    let score = acorde_io::parse_musicxml(xml).map_err(js_err)?;
    score_to_json(&score)
}

/// Parse a compressed MXL file (byte array) and return the score as a JSON string.
#[wasm_bindgen]
pub fn parse_mxl(data: &[u8]) -> Result<String, JsValue> {
    let score = acorde_io::parse_mxl(data).map_err(js_err)?;
    score_to_json(&score)
}

/// Serialize a score (JSON string) to MusicXML.
#[wasm_bindgen]
pub fn serialize_musicxml(score_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    acorde_io::serialize_musicxml(&score).map_err(js_err)
}

// ── MIDI ──────────────────────────────────────────────────────────────────────

/// Parse a MuseScore .mscz ZIP file (byte array) and return the score as a JSON string.
#[wasm_bindgen]
pub fn parse_mscz(data: &[u8]) -> Result<String, JsValue> {
    let score = acorde_io::parse_mscz(data).map_err(js_err)?;
    score_to_json(&score)
}

/// Parse a MuseScore .mscx XML string and return the score as a JSON string.
#[wasm_bindgen]
pub fn parse_mscx(xml: &str) -> Result<String, JsValue> {
    let score = acorde_io::parse_mscx(xml).map_err(js_err)?;
    score_to_json(&score)
}

/// Parse a MIDI file (byte array) and return the score as a JSON string.
#[wasm_bindgen]
pub fn parse_midi(data: &[u8]) -> Result<String, JsValue> {
    let score = acorde_io::parse_midi(data).map_err(js_err)?;
    score_to_json(&score)
}

/// Serialize a score (JSON string) to MIDI bytes (`Uint8Array` in JS).
#[wasm_bindgen]
pub fn serialize_midi(score_json: &str) -> Result<Vec<u8>, JsValue> {
    let score = score_from_json(score_json)?;
    acorde_io::serialize_midi(&score).map_err(js_err)
}

/// Serialize a region of a score to MIDI bytes.
///
/// Only measures with physical index in `[start_measure, end_measure]` are exported.
/// The MIDI file starts at tick 0 regardless of the region offset.
#[wasm_bindgen]
pub fn serialize_midi_region(
    score_json: &str, start_measure: usize, end_measure: usize,
) -> Result<Vec<u8>, JsValue> {
    let score = score_from_json(score_json)?;
    acorde_io::serialize_midi_region(&score, (start_measure, end_measure)).map_err(js_err)
}

// ── Playback ──────────────────────────────────────────────────────────────────

/// Compute playback events for a score (JSON string).
///
/// Returns a JSON array of `PlaybackEvent` objects, sorted by `time_beats`.
/// Each event includes: `time_beats`, `time_secs`, `pitch_midi`, `velocity`,
/// `duration_beats`, `duration_secs`, `pedal`, `part_index`.
///
/// `bpm`: override tempo in BPM. Pass `0` to use the score's own tempo.
/// `muted_parts_json`: JSON array of part indices to silence, e.g. `"[0,2]"` or `"[]"`.
#[deprecated(
    since = "0.2.0",
    note = "Use `to_playback_events_ex(score_json, options_json)` instead."
)]
#[allow(deprecated)]
#[wasm_bindgen]
pub fn to_playback_events(score_json: &str, bpm: u16, muted_parts_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let muted_parts: Vec<usize> = serde_json::from_str(muted_parts_json)
        .map_err(|e| js_err(format!("invalid muted_parts JSON: {e}")))?;
    let options = acorde_core::PlaybackOptions {
        bpm_override: if bpm == 0 { None } else { Some(bpm) },
        muted_parts,
        ..Default::default()
    };
    let events = acorde_core::to_playback_events(&score, &options);
    serde_json::to_string(&events)
        .map_err(|e| js_err(format!("playback serialization failed: {e}")))
}

/// Extended playback event generator accepting a full `PlaybackOptions` JSON.
///
/// Use this instead of [`to_playback_events`] when you need `loop_region` or other
/// options that cannot be expressed via positional parameters.
///
/// `options_json` example: `{"bpm_override":null,"muted_parts":[],"loop_region":[2,5]}`
#[wasm_bindgen]
pub fn to_playback_events_ex(score_json: &str, options_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let options: acorde_core::PlaybackOptions = serde_json::from_str(options_json)
        .map_err(|e| js_err(format!("invalid options JSON: {e}")))?;
    let events = acorde_core::to_playback_events(&score, &options);
    serde_json::to_string(&events)
        .map_err(|e| js_err(format!("playback serialization failed: {e}")))
}

/// Map `elapsed_secs` to a `PlaybackPosition` JSON object (`{measure_index, beat}`).
///
/// Returns JSON `null` when `elapsed_secs` is negative or past the end of the score.
/// Pass the same `options_json` used for `to_playback_events_ex` for consistency.
#[wasm_bindgen]
pub fn compute_playback_position(
    score_json: &str,
    options_json: &str,
    elapsed_secs: f64,
) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let options: acorde_core::PlaybackOptions = serde_json::from_str(options_json)
        .map_err(|e| js_err(format!("invalid options JSON: {e}")))?;
    let pos = acorde_core::compute_playback_position(&score, &options, elapsed_secs);
    serde_json::to_string(&pos)
        .map_err(|e| js_err(format!("position serialization failed: {e}")))
}

// ── Layout ────────────────────────────────────────────────────────────────────

/// Compute layout for a score (JSON string) and return the result as a JSON string.
///
/// `measures_per_row` defaults to 4 when 0 is passed.
/// `concert_pitch` when true populates `concert_key_overrides` in the result.
#[deprecated(since = "0.2.0", note = "use compute_layout_ex instead")]
#[allow(deprecated)]
#[wasm_bindgen]
pub fn compute_layout(score_json: &str, measures_per_row: usize, concert_pitch: bool) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let per_row = if measures_per_row == 0 { 4 } else { measures_per_row };
    let config = acorde_layout::LayoutConfig { measures_per_row: per_row, concert_pitch, first_row_measures: None };
    let result = acorde_layout::compute_layout(&score, &config);
    serde_json::to_string(&result).map_err(|e| js_err(format!("layout serialization failed: {e}")))
}

/// Compute layout with a full JSON [`LayoutConfig`].
///
/// Config example: `{"measures_per_row":4,"concert_pitch":false,"first_row_measures":3}`
/// Use this instead of `compute_layout` when you need `first_row_measures`.
#[wasm_bindgen]
pub fn compute_layout_ex(score_json: &str, config_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let config: acorde_layout::LayoutConfig = serde_json::from_str(config_json)
        .map_err(|e| js_err(format!("invalid config JSON: {e}")))?;
    let result = acorde_layout::compute_layout(&score, &config);
    serde_json::to_string(&result).map_err(|e| js_err(format!("layout serialization failed: {e}")))
}

// ── SVG rendering ────────────────────────────────────────────────────────────

/// Render a score (JSON string) to an SVG string.
///
/// `options_json` is a (partial) [`acorde_render_svg::SvgRenderOptions`] JSON object, e.g.
/// `{"width":900,"staff_size":24,"measures_per_system":4,"interactive":true}` — every field
/// has a default, so `"{}"` renders with defaults. This calls the exact same
/// `acorde_render_svg::render_svg` used natively — no browser/DOM-specific code path.
#[wasm_bindgen]
pub fn render_score_svg(score_json: &str, options_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let options: acorde_render_svg::SvgRenderOptions = serde_json::from_str(options_json)
        .map_err(|e| js_err(format!("invalid options JSON: {e}")))?;
    acorde_render_svg::render_svg(&score, &options).map_err(js_err)
}

// ── GM lookup ─────────────────────────────────────────────────────────────────

/// Return the General MIDI Level 1 program name for a 0-based program number.
/// Returns `"Unknown"` for values >= 128.
#[wasm_bindgen]
pub fn gm_program_name(program: u8) -> String {
    acorde_core::program_name(program).to_string()
}

/// Return the General MIDI percussion instrument name for a MIDI note number (35–81).
/// Returns `"Unknown Drum"` outside the standard range.
#[wasm_bindgen]
pub fn gm_drum_name(note: u8) -> String {
    acorde_core::drum_name(note).to_string()
}

// ── ABC ───────────────────────────────────────────────────────────────────────

/// Parse an ABC Notation string and return the score as JSON.
#[wasm_bindgen]
pub fn parse_abc(text: &str) -> Result<String, JsValue> {
    let score = acorde_io::parse_abc(text).map_err(js_err)?;
    score_to_json(&score)
}

/// Serialize a score (JSON string) to ABC Notation.
#[wasm_bindgen]
pub fn serialize_abc(score_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    acorde_io::serialize_abc(&score).map_err(js_err)
}

// ── Score operations ──────────────────────────────────────────────────────────

/// Validate a score. Returns a JSON `ValidationReport` with `errors` and `warnings` arrays.
#[wasm_bindgen]
pub fn validate_score(score_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let report = acorde_core::validate(&score);
    serde_json::to_string(&report)
        .map_err(|e| js_err(format!("validation serialization failed: {e}")))
}

/// Transpose all notes in a score by `semitones` (positive = up).
#[wasm_bindgen]
pub fn transpose_score(score_json: &str, semitones: i8) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    score_to_json(&acorde_core::transpose(&score, semitones))
}

/// Extract a single part (0-based) from a score.
#[wasm_bindgen]
pub fn extract_part(score_json: &str, part_index: usize) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let extracted = score.extract_part(part_index)
        .ok_or_else(|| js_err(format!("part index {part_index} out of range")))?;
    score_to_json(&extracted)
}

/// Merge two scores: append parts of `score_b` into `score_a`.
#[wasm_bindgen]
pub fn merge_scores(score_a_json: &str, score_b_json: &str) -> Result<String, JsValue> {
    let a = score_from_json(score_a_json)?;
    let b = score_from_json(score_b_json)?;
    score_to_json(&a.merge(&b))
}

/// Compute the diff between two scores. Returns a JSON array of `ScoreChange` objects.
#[wasm_bindgen]
pub fn diff_scores(score_a_json: &str, score_b_json: &str) -> Result<String, JsValue> {
    let a = score_from_json(score_a_json)?;
    let b = score_from_json(score_b_json)?;
    let changes = acorde_core::diff(&a, &b);
    serde_json::to_string(&changes)
        .map_err(|e| js_err(format!("diff serialization failed: {e}")))
}

/// Compute statistics for a score (JSON string).
///
/// Returns a `ScoreStats` JSON object with fields:
/// `measure_count`, `note_count`, `rest_count`, `part_count`, `estimated_duration_secs`.
#[wasm_bindgen]
pub fn score_statistics(score_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let stats = score.statistics();
    serde_json::to_string(&stats)
        .map_err(|e| js_err(format!("statistics serialization failed: {e}")))
}

// ── Accordion arrangement ─────────────────────────────────────────────────────

/// Rank a score's non-percussion parts by mean pitch for accordion arrangement.
/// Returns JSON `{ candidates: [{part_index, name, mean_pitch}], ambiguous }`.
/// `ambiguous` is true when the top two candidates' mean pitch is within 3
/// semitones — the caller should offer a right-hand-part picker.
#[wasm_bindgen]
pub fn analyze_for_accordion(score_json: &str) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let analysis = acorde_core::analyze_for_accordion(&score);
    serde_json::to_string(&analysis)
        .map_err(|e| js_err(format!("analysis serialization failed: {e}")))
}

/// Arrange a score onto a 2-staff accordion part (treble = right hand,
/// bass = left hand), reassigned to the Accordion GM program and
/// octave-fit to its practical range. Returns JSON `{ score, notes }`
/// where `notes` are human-readable messages about choices made.
///
/// `right_hand_part_index`: pass a non-negative index to force that part
/// onto the treble staff, or `-1` for the automatic mean-pitch ranking
/// from [`analyze_for_accordion`].
#[wasm_bindgen]
pub fn arrange_for_accordion(score_json: &str, right_hand_part_index: i32) -> Result<String, JsValue> {
    let score = score_from_json(score_json)?;
    let override_index = if right_hand_part_index < 0 { None } else { Some(right_hand_part_index as usize) };
    let result = acorde_core::arrange_for_accordion(&score, override_index)
        .map_err(|e| js_err(format!("arrangement failed: {e}")))?;
    serde_json::to_string(&result)
        .map_err(|e| js_err(format!("arrange result serialization failed: {e}")))
}

/// Return the total duration of a score in seconds (uses score tempo and repeat structure).
#[wasm_bindgen]
pub fn score_duration_secs(score_json: &str) -> Result<f64, JsValue> {
    let score = score_from_json(score_json)?;
    Ok(acorde_core::score_duration_secs(&score))
}

/// Duration in seconds for measures `start_measure..=end_measure` (0-based, inclusive).
///
/// Useful for computing progress-bar lengths when `loop_region` is active.
#[wasm_bindgen]
pub fn score_duration_secs_region(
    score_json: &str,
    start_measure: usize,
    end_measure: usize,
) -> Result<f64, JsValue> {
    let score = score_from_json(score_json)?;
    Ok(acorde_core::score_duration_secs_region(&score, (start_measure, end_measure)))
}

/// Respell all pitches in a score (no undo). Returns the modified score as a JSON string.
///
/// `prefer_flat`: when `true` uses flat spellings (e.g. Db instead of C#).
#[wasm_bindgen]
pub fn respell_score(score_json: &str, prefer_flat: bool) -> Result<String, JsValue> {
    let mut score = score_from_json(score_json)?;
    acorde_core::respell_score(&mut score, prefer_flat);
    score_to_json(&score)
}

/// Respell all pitches to match the score's key signature (no undo).
///
/// Flat-key signatures use flat spellings; sharp-key and C major use sharps.
#[wasm_bindgen]
pub fn respell_score_to_key(score_json: &str) -> Result<String, JsValue> {
    let mut score = score_from_json(score_json)?;
    acorde_core::respell_score_to_key(&mut score);
    score_to_json(&score)
}

/// Return the number of beats available in a voice before the measure is full (≥ 0.0).
///
/// Correctly accounts for tuplet scaling via `Note::beats()`.
/// Returns an error if any index is out of range.
#[wasm_bindgen]
pub fn measure_beats_remaining(
    score_json: &str,
    part_index: usize,
    staff_index: usize,
    measure_index: usize,
    voice_index: usize,
) -> Result<f64, JsValue> {
    let score = score_from_json(score_json)?;
    acorde_core::measure_beats_remaining(
        &score, part_index, staff_index, measure_index, voice_index,
    ).map_err(js_err)
}

/// Convert a MIDI note number (0–127) to a `Pitch` JSON object.
///
/// `prefer_flat`: `true` = Db/Eb/Gb/Ab/Bb spelling; `false` = C#/D#/F#/G#/A#.
#[wasm_bindgen]
pub fn pitch_from_midi(midi: u8, prefer_flat: bool) -> Result<String, JsValue> {
    let pitch = acorde_core::Pitch::from_midi(midi, prefer_flat);
    serde_json::to_string(&pitch)
        .map_err(|e| js_err(format!("pitch serialization failed: {e}")))
}

/// Parse scientific pitch notation (`"C4"`, `"F#5"`, `"Bb3"`) into a `Pitch` JSON object.
///
/// Returns an error if the string cannot be parsed.
#[wasm_bindgen]
pub fn pitch_from_str(s: &str) -> Result<String, JsValue> {
    let pitch: acorde_core::Pitch = s.parse()
        .map_err(|_| js_err(format!("invalid pitch string: {s}")))?;
    serde_json::to_string(&pitch)
        .map_err(|e| js_err(format!("pitch serialization failed: {e}")))
}

// ── Theory ────────────────────────────────────────────────────────────────────

/// Compute the signed chromatic interval from `pitch1` to `pitch2`.
///
/// Returns an `Interval` JSON object with fields `semitones` (signed i16) and a
/// `display` string such as `"P5"`, `"M3"`, `"m7"`.
#[wasm_bindgen]
pub fn interval_between(pitch1_json: &str, pitch2_json: &str) -> Result<String, JsValue> {
    let a: acorde_core::Pitch = serde_json::from_str(pitch1_json)
        .map_err(|e| js_err(format!("invalid pitch1 JSON: {e}")))?;
    let b: acorde_core::Pitch = serde_json::from_str(pitch2_json)
        .map_err(|e| js_err(format!("invalid pitch2 JSON: {e}")))?;
    let iv = acorde_core::Interval::between(&a, &b);
    serde_json::to_string(&iv)
        .map_err(|e| js_err(format!("interval serialization failed: {e}")))
}

/// Return the accidental alter (`-1`, `0`, or `+1`) for a diatonic step in a key signature.
///
/// `step_char`: single letter `"C"`, `"D"`, `"E"`, `"F"`, `"G"`, `"A"`, or `"B"` (case-insensitive).
#[wasm_bindgen]
pub fn key_alter_for_step(key_json: &str, step_char: &str) -> Result<i8, JsValue> {
    let key: acorde_core::KeySignature = serde_json::from_str(key_json)
        .map_err(|e| js_err(format!("invalid key JSON: {e}")))?;
    let step = acorde_core::Step::from_char(
        step_char.chars().next().ok_or_else(|| js_err("empty step_char"))?
    ).ok_or_else(|| js_err(format!("invalid step character: {step_char}")))?;
    Ok(key.alter_for_step(&step))
}

/// True if `pitch` is diatonic to the given key signature (octave-independent).
#[wasm_bindgen]
pub fn key_contains_pitch(key_json: &str, pitch_json: &str) -> Result<bool, JsValue> {
    let key: acorde_core::KeySignature = serde_json::from_str(key_json)
        .map_err(|e| js_err(format!("invalid key JSON: {e}")))?;
    let pitch: acorde_core::Pitch = serde_json::from_str(pitch_json)
        .map_err(|e| js_err(format!("invalid pitch JSON: {e}")))?;
    Ok(key.contains_pitch(&pitch))
}

/// Human-readable key name: `"C major"`, `"G major"`, `"F# minor"`, `"Bb major"` etc.
#[wasm_bindgen]
pub fn key_display_name(key_json: &str) -> Result<String, JsValue> {
    let key: acorde_core::KeySignature = serde_json::from_str(key_json)
        .map_err(|e| js_err(format!("invalid key JSON: {e}")))?;
    Ok(key.display_name())
}

/// Detect the chord name from a JSON array of `Pitch` objects.
///
/// Returns a `ChordSymbol` JSON object, or JSON `null` if fewer than 2 pitches are
/// provided or no template matches.
/// Inversions are detected automatically; slash-chord bass is set when applicable.
#[wasm_bindgen]
pub fn detect_chord(pitches_json: &str) -> Result<String, JsValue> {
    let pitches: Vec<acorde_core::Pitch> = serde_json::from_str(pitches_json)
        .map_err(|e| js_err(format!("invalid pitches JSON: {e}")))?;
    let result = acorde_core::detect_chord(&pitches);
    serde_json::to_string(&result)
        .map_err(|e| js_err(format!("chord serialization failed: {e}")))
}

/// Return the Roman numeral analysis of a chord in the context of a key.
///
/// `chord_json`: JSON-encoded `ChordSymbol` (e.g. `{"root":"G","kind":"dominant","bass":null}`).
/// `key_json`: JSON-encoded `KeySignature` (e.g. `{"fifths":0,"mode":"major"}`).
/// Returns a JSON string such as `"V7"`, `"ii"`, `"viio"`, or JSON `null` if the chord root
/// falls outside the key's scale.
#[wasm_bindgen]
pub fn roman_numeral(chord_json: &str, key_json: &str) -> Result<String, JsValue> {
    let chord: acorde_core::ChordSymbol = serde_json::from_str(chord_json)
        .map_err(|e| js_err(format!("invalid chord JSON: {e}")))?;
    let key: acorde_core::KeySignature = serde_json::from_str(key_json)
        .map_err(|e| js_err(format!("invalid key JSON: {e}")))?;
    let result = acorde_core::roman_numeral(&chord, &key);
    serde_json::to_string(&result)
        .map_err(|e| js_err(format!("serialization failed: {e}")))
}

/// Find the scale that best fits a set of pitches.
///
/// `pitches_json`: JSON array of `Pitch` objects.
/// Returns a JSON `Scale` object (`{"root":{…},"kind":"Major"}`) or JSON `null` if the array
/// is empty.
#[wasm_bindgen]
pub fn best_fit_scale(pitches_json: &str) -> Result<String, JsValue> {
    let pitches: Vec<acorde_core::Pitch> = serde_json::from_str(pitches_json)
        .map_err(|e| js_err(format!("invalid pitches JSON: {e}")))?;
    let result = acorde_core::Scale::best_fit(&pitches);
    serde_json::to_string(&result)
        .map_err(|e| js_err(format!("serialization failed: {e}")))
}

/// MIDI note number of the middle staff line for a `Clef` JSON value.
///
/// Used for stem direction heuristics. Treble=71 (B4), Bass=50 (D3), Alto=60 (C4),
/// Tenor=57 (A3), Percussion=71.
#[wasm_bindgen]
pub fn clef_middle_line_midi(clef_json: &str) -> Result<u8, JsValue> {
    let clef: acorde_core::Clef = serde_json::from_str(clef_json)
        .map_err(|e| js_err(format!("invalid clef JSON: {e}")))?;
    Ok(clef.middle_line_midi())
}

/// Suggest stem direction for a set of pitches in a given clef.
///
/// Returns `true` if the stem should point up (average MIDI < middle line),
/// `false` for stem down. Empty pitch arrays (rests) always return `true`.
///
/// `pitches_json`: JSON array of `Pitch` objects.
/// `clef_json`: JSON-encoded `Clef` value (e.g. `"\"Treble\""`).
#[wasm_bindgen]
pub fn suggested_stem_up(pitches_json: &str, clef_json: &str) -> Result<bool, JsValue> {
    let pitches: Vec<acorde_core::Pitch> = serde_json::from_str(pitches_json)
        .map_err(|e| js_err(format!("invalid pitches JSON: {e}")))?;
    let clef: acorde_core::Clef = serde_json::from_str(clef_json)
        .map_err(|e| js_err(format!("invalid clef JSON: {e}")))?;
    Ok(acorde_core::suggested_stem_up(&pitches, &clef))
}

/// Compute recommended `BeamState` values for a voice's notes.
///
/// Groups beamable notes (eighth or shorter, non-rest) within beat boundaries
/// derived from the time signature. Returns a JSON array of `BeamState` strings.
///
/// `notes_json`: JSON array of `Note` objects (a full voice).
/// `time_sig_json`: JSON-encoded `TimeSignature`.
#[wasm_bindgen]
pub fn compute_beams(notes_json: &str, time_sig_json: &str) -> Result<String, JsValue> {
    let notes: Vec<acorde_core::Note> = serde_json::from_str(notes_json)
        .map_err(|e| js_err(format!("invalid notes JSON: {e}")))?;
    let time_sig: acorde_core::TimeSignature = serde_json::from_str(time_sig_json)
        .map_err(|e| js_err(format!("invalid time_sig JSON: {e}")))?;
    let states = acorde_core::compute_beams(&notes, &time_sig);
    serde_json::to_string(&states)
        .map_err(|e| js_err(format!("beam serialization failed: {e}")))
}

/// Return the stable i18n key for a JSON-encoded `Command`.
///
/// Returns strings like `"AddNote"`, `"SetTempo"`, etc.
/// Use this to look up translations without hard-coding English labels.
#[wasm_bindgen]
pub fn command_key_from_json(cmd_json: &str) -> Result<String, JsValue> {
    let cmd: acorde_core::Command = serde_json::from_str(cmd_json)
        .map_err(|e| js_err(format!("invalid command JSON: {e}")))?;
    Ok(acorde_core::command_key(&cmd))
}

// ── ScoreEngine ───────────────────────────────────────────────────────────────

/// JavaScript-visible wrapper around `acorde_core::ScoreEngine`.
///
/// All Score and Command values are passed as JSON strings.
#[wasm_bindgen]
pub struct ScoreEngine {
    inner: acorde_core::ScoreEngine,
}

impl Default for ScoreEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl ScoreEngine {
    /// Create a new engine with a default score.
    #[wasm_bindgen(constructor)]
    pub fn new() -> ScoreEngine {
        ScoreEngine { inner: acorde_core::ScoreEngine::new() }
    }

    /// Return the current score as a JSON string.
    pub fn get_score(&self) -> Result<String, JsValue> {
        score_to_json(&self.inner.score)
    }

    /// Return the current undo/redo version counter.
    pub fn get_version(&self) -> u64 {
        self.inner.version
    }

    /// Apply a command (JSON string). Returns a [`ChangeHint`] JSON string on success.
    pub fn apply(&mut self, cmd_json: &str) -> Result<String, JsValue> {
        let cmd: Command = serde_json::from_str(cmd_json)
            .map_err(|e| js_err(format!("invalid command JSON: {e}")))?;
        let hint = self.inner.apply(cmd).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Undo the last command. Returns a [`ChangeHint`] JSON string on success.
    pub fn undo(&mut self) -> Result<String, JsValue> {
        let hint = self.inner.undo().map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Redo the last undone command. Returns a [`ChangeHint`] JSON string on success.
    pub fn redo(&mut self) -> Result<String, JsValue> {
        let hint = self.inner.redo().map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Apply multiple commands as a single undoable batch. Returns a [`ChangeHint`] JSON string.
    pub fn apply_batch(&mut self, cmds_json: &str) -> Result<String, JsValue> {
        let cmds: Vec<Command> = serde_json::from_str(cmds_json)
            .map_err(|e| js_err(format!("invalid commands JSON: {e}")))?;
        let hint = self.inner.batch_apply(cmds).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Apply multiple commands as a single undoable batch with an explicit label.
    ///
    /// The `label` is used as the undo/redo key (e.g. `"ApplyAI"`, `"PasteSelection"`).
    pub fn apply_batch_labeled(&mut self, cmds_json: &str, label: &str) -> Result<String, JsValue> {
        let cmds: Vec<Command> = serde_json::from_str(cmds_json)
            .map_err(|e| js_err(format!("invalid commands JSON: {e}")))?;
        let hint = self.inner.batch_apply_labeled(cmds, label).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Replace the entire score (JSON string).
    pub fn replace_score(&mut self, score_json: &str) -> Result<(), JsValue> {
        let score = score_from_json(score_json)?;
        self.inner.replace_score(score);
        Ok(())
    }

    /// Copy a voice into the engine's clipboard.
    pub fn copy_voice(
        &mut self,
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        voice_index: usize,
    ) -> Result<(), JsValue> {
        self.inner.copy_voice(part_index, staff_index, measure_index, voice_index)
            .map_err(js_err)
    }

    /// Paste the clipboard into a voice (undo-able). Returns a [`ChangeHint`] JSON string.
    pub fn paste_voice(
        &mut self,
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        voice_index: usize,
    ) -> Result<String, JsValue> {
        let hint = self.inner.paste_voice(part_index, staff_index, measure_index, voice_index)
            .map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Label of the next undoable command, or `undefined` if nothing to undo.
    pub fn get_undo_label(&self) -> Option<String> {
        self.inner.undo_label()
    }

    /// Label of the next redoable command, or `undefined` if nothing to redo.
    pub fn get_redo_label(&self) -> Option<String> {
        self.inner.redo_label()
    }

    /// i18n key of the next undoable command (e.g. `"SetTempo"`), or `undefined`.
    pub fn get_undo_key(&self) -> Option<String> {
        self.inner.undo_key()
    }

    /// i18n key of the next redoable command, or `undefined`.
    pub fn get_redo_key(&self) -> Option<String> {
        self.inner.redo_key()
    }

    /// Copy a range of voice measures into the range clipboard.
    ///
    /// `start_json` / `end_json` are JSON-encoded `NoteAddr` objects.
    /// `start` and `end` must share the same `part`, `staff`, and `voice`.
    pub fn copy_range(&mut self, start_json: &str, end_json: &str) -> Result<(), JsValue> {
        let start: acorde_core::NoteAddr = serde_json::from_str(start_json)
            .map_err(|e| js_err(format!("invalid start NoteAddr: {e}")))?;
        let end: acorde_core::NoteAddr = serde_json::from_str(end_json)
            .map_err(|e| js_err(format!("invalid end NoteAddr: {e}")))?;
        self.inner.copy_range(start, end).map_err(js_err)
    }

    /// Paste the range clipboard at `target` (undo-able). Returns a [`ChangeHint`] JSON string.
    pub fn paste_range(&mut self, target_json: &str) -> Result<String, JsValue> {
        let target: acorde_core::NoteAddr = serde_json::from_str(target_json)
            .map_err(|e| js_err(format!("invalid target NoteAddr: {e}")))?;
        let hint = self.inner.paste_range(target).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Export the command history as a JSON string for crash recovery or replay.
    ///
    /// The result can be stored and later restored with [`restore_history`].
    pub fn export_history(&self) -> Result<String, JsValue> {
        let h = self.inner.export_history();
        serde_json::to_string(&h)
            .map_err(|e| js_err(format!("history serialization failed: {e}")))
    }

    /// Restore an engine from a previously exported history JSON string.
    ///
    /// Replays all commands against the initial score. Returns an error if replay fails.
    pub fn restore_history(&mut self, json: &str) -> Result<(), JsValue> {
        let history: acorde_core::EngineHistory = serde_json::from_str(json)
            .map_err(|e| js_err(format!("invalid history JSON: {e}")))?;
        let restored = acorde_core::ScoreEngine::from_history(history).map_err(js_err)?;
        self.inner = restored;
        Ok(())
    }

    /// Toggle slur between two notes. Returns a [`ChangeHint`] JSON string.
    ///
    /// `start_json` and `end_json` are JSON-encoded `NoteAddr` objects.
    pub fn toggle_slur(&mut self, start_json: &str, end_json: &str) -> Result<String, JsValue> {
        let start: acorde_core::NoteAddr = serde_json::from_str(start_json)
            .map_err(|e| js_err(format!("invalid start NoteAddr: {e}")))?;
        let end: acorde_core::NoteAddr = serde_json::from_str(end_json)
            .map_err(|e| js_err(format!("invalid end NoteAddr: {e}")))?;
        let hint = self.inner.toggle_slur(start, end).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Add a new staff to an existing part. Returns a [`ChangeHint`] JSON string.
    ///
    /// `clef`: `"Treble"` / `"Bass"` / `"Alto"` / `"Tenor"` / `"Percussion"`.
    pub fn add_staff(&mut self, part_index: usize, clef: &str) -> Result<String, JsValue> {
        let clef = match clef {
            "Bass"       => acorde_core::Clef::Bass,
            "Alto"       => acorde_core::Clef::Alto,
            "Tenor"      => acorde_core::Clef::Tenor,
            "Percussion" => acorde_core::Clef::Percussion,
            _            => acorde_core::Clef::Treble,
        };
        let hint = self.inner.add_staff(part_index, clef).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Delete a staff from a part. Fails if it is the last remaining staff.
    /// Returns a [`ChangeHint`] JSON string.
    pub fn delete_staff(&mut self, part_index: usize, staff_index: usize) -> Result<String, JsValue> {
        let hint = self.inner.delete_staff(part_index, staff_index).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Set or clear the tuplet on a note (undo-able). Returns a [`ChangeHint`] JSON string.
    ///
    /// `addr_json`: JSON-encoded `NoteAddr`.
    /// `tuplet_json`: JSON-encoded `TupletInfo`, or `"null"` to clear.
    pub fn set_tuplet(&mut self, addr_json: &str, tuplet_json: &str) -> Result<String, JsValue> {
        let addr: acorde_core::NoteAddr = serde_json::from_str(addr_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let tuplet: Option<acorde_core::TupletInfo> = serde_json::from_str(tuplet_json)
            .map_err(|e| js_err(format!("invalid tuplet JSON: {e}")))?;
        let hint = self.inner.set_tuplet(addr, tuplet).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Respell all pitches in the score (undo-able). Returns a [`ChangeHint`] JSON string.
    ///
    /// `prefer_flat`: when `true` uses flat spellings (e.g. Db instead of C#).
    pub fn respell_score(&mut self, prefer_flat: bool) -> Result<String, JsValue> {
        let hint = self.inner.respell_score(prefer_flat).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Respell all pitches to match the key signature (undo-able). Returns a [`ChangeHint`] JSON string.
    pub fn respell_score_to_key(&mut self) -> Result<String, JsValue> {
        let hint = self.inner.respell_score_to_key().map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Set or clear the stem direction on a note (undo-able). Returns a [`ChangeHint`] JSON string.
    ///
    /// `addr_json`: JSON-encoded `NoteAddr`.
    /// `stem_up_json`: `"true"` (up) | `"false"` (down) | `"null"` (auto).
    pub fn set_stem(&mut self, addr_json: &str, stem_up_json: &str) -> Result<String, JsValue> {
        let addr: acorde_core::NoteAddr = serde_json::from_str(addr_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let stem_up: Option<bool> = serde_json::from_str(stem_up_json)
            .map_err(|e| js_err(format!("invalid stem_up value: {e}")))?;
        let hint = self.inner.set_stem(addr, stem_up).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Set or clear the arpeggio direction on a note (undo-able). Returns a [`ChangeHint`] JSON string.
    ///
    /// `addr_json`: JSON-encoded `NoteAddr`.
    /// `direction_json`: `"true"` (up) | `"false"` (down) | `"null"` (clear).
    pub fn set_arpeggio(&mut self, addr_json: &str, direction_json: &str) -> Result<String, JsValue> {
        let addr: acorde_core::NoteAddr = serde_json::from_str(addr_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let direction: Option<bool> = serde_json::from_str(direction_json)
            .map_err(|e| js_err(format!("invalid direction value: {e}")))?;
        let hint = self.inner.set_arpeggio(addr, direction).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Toggle a trill line span between two notes.
    ///
    /// `start_json` / `end_json`: JSON-encoded `NoteAddr`.
    pub fn toggle_trill_line(&mut self, start_json: &str, end_json: &str) -> Result<String, JsValue> {
        let start: acorde_core::NoteAddr = serde_json::from_str(start_json)
            .map_err(|e| js_err(format!("invalid start NoteAddr: {e}")))?;
        let end: acorde_core::NoteAddr = serde_json::from_str(end_json)
            .map_err(|e| js_err(format!("invalid end NoteAddr: {e}")))?;
        let hint = self.inner.toggle_trill_line(start, end).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Set or clear the cue flag on a note. Cue notes have zero beats.
    ///
    /// `addr_json`: JSON-encoded `NoteAddr`.
    /// `is_cue`: `true` to mark as cue, `false` to clear.
    pub fn set_cue(&mut self, addr_json: &str, is_cue: bool) -> Result<String, JsValue> {
        let addr: acorde_core::NoteAddr = serde_json::from_str(addr_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let hint = self.inner.set_cue(addr, is_cue).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Set the note head shape on a note.
    ///
    /// `addr_json`: JSON-encoded `NoteAddr`.
    /// `note_head_json`: JSON-encoded `NoteHead` (e.g. `"\"Diamond\""`, `"\"Normal\""`).
    pub fn set_note_head(&mut self, addr_json: &str, note_head_json: &str) -> Result<String, JsValue> {
        let addr: acorde_core::NoteAddr = serde_json::from_str(addr_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let note_head: acorde_core::NoteHead = serde_json::from_str(note_head_json)
            .map_err(|e| js_err(format!("invalid NoteHead: {e}")))?;
        let hint = self.inner.set_note_head(addr, note_head).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }

    /// Begin a slur at `start`. Must be followed by `end_slur` to complete it.
    ///
    /// `start_json`: JSON-encoded `NoteAddr`.
    pub fn begin_slur(&mut self, start_json: &str) -> Result<(), JsValue> {
        let start: acorde_core::NoteAddr = serde_json::from_str(start_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        self.inner.begin_slur(start).map_err(js_err)
    }

    /// Complete a slur that was started with `begin_slur`. Returns a [`ChangeHint`] JSON string.
    ///
    /// `end_json`: JSON-encoded `NoteAddr`. Returns an error if `begin_slur` was not called first.
    pub fn end_slur(&mut self, end_json: &str) -> Result<String, JsValue> {
        let end: acorde_core::NoteAddr = serde_json::from_str(end_json)
            .map_err(|e| js_err(format!("invalid NoteAddr: {e}")))?;
        let hint = self.inner.end_slur(end).map_err(js_err)?;
        serde_json::to_string(&hint).map_err(|e| js_err(format!("hint serialization failed: {e}")))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

// Native unit tests — must not call #[wasm_bindgen] functions since
// JsValue panics outside of a wasm32 runtime.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_roundtrip_via_json() {
        let score = Score::default();
        let json = score_to_json(&score).unwrap();
        let back: Score = serde_json::from_str(&json).unwrap();
        assert_eq!(score.id, back.id);
    }

    #[test]
    fn engine_default_score_has_parts() {
        let engine = ScoreEngine::new();
        // score_to_json calls serde but not JsValue, safe on native
        let json = serde_json::to_string(&engine.inner.score).unwrap();
        assert!(json.contains("parts"));
    }

    #[test]
    fn engine_version_increments_on_apply() {
        use acorde_core::Command;
        let mut engine = ScoreEngine::new();
        let v0 = engine.get_version();
        let cmd = Command::SetTempo(acorde_core::SetTempoCmd { bpm: 120 });
        let cmd_json = serde_json::to_string(&cmd).unwrap();
        engine.apply(&cmd_json).unwrap();
        assert_eq!(engine.get_version(), v0 + 1);
    }
}

// WASM integration tests — run with `wasm-pack test --headless --chrome`
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn parse_musicxml_empty_returns_err() {
        assert!(parse_musicxml("").is_err());
    }

    #[wasm_bindgen_test]
    fn parse_musicxml_garbage_returns_err() {
        assert!(parse_musicxml("not xml at all!!!").is_err());
    }

    #[wasm_bindgen_test]
    fn engine_undo_empty_returns_err() {
        let mut engine = ScoreEngine::new();
        assert!(engine.undo().is_err());
    }
}
