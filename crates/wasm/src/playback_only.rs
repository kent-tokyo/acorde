//! Minimal browser binding for scores displayed as original images.

use acorde_core::{PlaybackOptions, Score};
use wasm_bindgen::prelude::*;

const MAX_SCORE_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_OPTIONS_JSON_BYTES: usize = 64 * 1024;

fn js_err(message: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&message.to_string())
}

/// Parse a MuseScore MSCZ archive into the same canonical score JSON as the
/// full and viewer bindings.
#[wasm_bindgen]
pub fn parse_mscz(data: &[u8]) -> Result<String, JsValue> {
    let score = acorde_io::parse_mscz(data).map_err(js_err)?;
    serde_json::to_string(&score)
        .map_err(|error| js_err(format!("score serialization failed: {error}")))
}

/// Compute the same bounded playback schedule as the full and viewer bindings.
#[wasm_bindgen]
pub fn to_playback_events_ex(score_json: &str, options_json: &str) -> Result<String, JsValue> {
    if score_json.len() > MAX_SCORE_JSON_BYTES {
        return Err(js_err(format!(
            "score JSON exceeds limit: {} bytes",
            score_json.len()
        )));
    }
    if options_json.len() > MAX_OPTIONS_JSON_BYTES {
        return Err(js_err(format!(
            "options JSON exceeds limit: {} bytes",
            options_json.len()
        )));
    }
    let score: Score = serde_json::from_str(score_json)
        .map_err(|error| js_err(format!("invalid score JSON: {error}")))?;
    let options: PlaybackOptions = serde_json::from_str(options_json)
        .map_err(|error| js_err(format!("invalid options JSON: {error}")))?;
    let events = acorde_core::to_playback_events_bounded(&score, &options).map_err(js_err)?;
    serde_json::to_string(&events)
        .map_err(|error| js_err(format!("playback serialization failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, Note, Pitch, Step};

    #[test]
    fn mscz_playback_keeps_pitch_duration_part_and_source_address() {
        let mut score = Score::new("Playback", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let mscz = acorde_io::serialize_mscz(&score).unwrap();
        let restored = parse_mscz(&mscz).unwrap();
        let events: Vec<acorde_core::PlaybackEvent> =
            serde_json::from_str(&to_playback_events_ex(&restored, "{}").unwrap()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 60);
        assert_eq!(events[0].duration_beats, 1.0);
        assert_eq!(events[0].part_index, 0);
        assert_eq!(events[0].address.as_deref(), Some("0:0:0:0:0"));
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod browser_tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn rejects_untrusted_inputs() {
        assert!(parse_mscz(b"not a ZIP").is_err());
        assert!(to_playback_events_ex("{}", "not JSON").is_err());
    }
}
