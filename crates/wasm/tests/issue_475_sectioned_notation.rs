#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn sound(score: &str) -> Vec<(u8, i64, i64)> {
    let score: acorde_core::Score = serde_json::from_str(score).unwrap();
    let mut out = acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.pitch_midi,
                (e.time_beats * 960.0).round() as i64,
                (e.duration_beats * 960.0).round() as i64,
            )
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}
fn chords(score: &str) -> Vec<(usize, usize, i64, String, String)> {
    let score: acorde_core::Score = serde_json::from_str(score).unwrap();
    let mut out = Vec::new();
    for (staff_index, staff) in score.parts[0].staves.iter().enumerate() {
        for (measure_index, bar) in staff.measures.iter().enumerate() {
            for voice in &bar.voices {
                let mut time = 0.0;
                for note in voice {
                    if let Some(chord) = &note.chord_symbol {
                        out.push((
                            staff_index,
                            measure_index,
                            (time * 960.0_f64).round() as i64,
                            chord.root.clone(),
                            chord.kind.clone(),
                        ));
                    }
                    time += note.beats();
                }
            }
        }
    }
    out.sort();
    out
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn standard_bass_is_readable_and_exports_without_changing_playback() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc z2 e | c z2 e |]").unwrap();
    let options = json!({"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}},{"measure_index":1,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}}],"bass_mode":"standard-bass","default_pattern":"bass-chord-pulse","sections":[]}).to_string();
    for selection in [
        json!({"rest_notes":[]}),
        json!({"rest_notes":[{"part":0,"staff":0,"measure":0,"voice":0,"note":1}]}),
    ] {
        let request = selection.to_string();
        let old: Value = serde_json::from_str(
            &acorde_wasm::arrange_with_response_fills(&source, &options, &request).unwrap(),
        )
        .unwrap();
        let new: Value = serde_json::from_str(
            &acorde_wasm::arrange_with_response_fills_readable_standard_bass(
                &source, &options, &request,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(old["fills"], new["fills"]);
        let a = old["arrangement"]["score"].to_string();
        let b = new["arrangement"]["score"].to_string();
        assert_eq!(sound(&a), sound(&b));
        assert_eq!(
            new["arrangement"]["score"]["parts"][0]["staves"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let svg = acorde_wasm::render_score_svg(&b, "{\"interactive\":true}").unwrap();
        for addr in new["arrangement"]["generated_notes"].as_array().unwrap() {
            let marker = format!(
                "data-note-addr=\"{}:{}:{}:{}:{}\"",
                addr["part"], addr["staff"], addr["measure"], addr["voice"], addr["note"]
            );
            assert!(svg.contains(&marker), "missing interactive marker {marker}");
        }
        let mscz = acorde_wasm::serialize_mscz(&b).unwrap();
        let restored = acorde_wasm::parse_mscz(&mscz).unwrap();
        assert_eq!(sound(&b), sound(&restored));
        assert_eq!(chords(&b), chords(&restored));
        #[cfg(feature = "musicxml")]
        {
            let xml = acorde_wasm::serialize_musicxml(&b).unwrap();
            let restored = acorde_wasm::parse_musicxml(&xml).unwrap();
            assert_eq!(sound(&b), sound(&restored));
            assert_eq!(chords(&b), chords(&restored));
        }
    }
}
