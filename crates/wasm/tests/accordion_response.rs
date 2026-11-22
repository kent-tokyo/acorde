#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn playback(score_json: &str) -> Vec<(u8, u64, usize)> {
    let score: acorde_core::Score = serde_json::from_str(score_json).unwrap();
    acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    event.pitch_midi,
                    (event.time_beats * 480.0).round() as u64,
                    source.staff,
                )
            })
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn response_fills_match_native_and_survive_score_exports() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc z2 e | c z2 e |").unwrap();
    let accompaniment = json!({
        "chords":[
            {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
            {"measure_index":1,"offset_beats":2.0,"chord":{"root":"G","kind":"major"}}
        ],
        "bass_mode":"free-bass", "default_pattern":null, "sections":[]
    });
    let response = json!({"rest_notes":[
        {"part":0,"staff":0,"measure":0,"voice":0,"note":1},
        {"part":0,"staff":0,"measure":1,"voice":0,"note":1}
    ]});
    let json = acorde_wasm::arrange_with_response_fills(
        &input,
        &accompaniment.to_string(),
        &response.to_string(),
    )
    .unwrap();
    let web: acorde_core::ResponseFilledAccordionResult = serde_json::from_str(&json).unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let native = acorde_core::arrange_with_response_fills(
        &source,
        serde_json::from_value(accompaniment.clone()).unwrap(),
        serde_json::from_value(response.clone()).unwrap(),
    );
    assert!(
        web.arrangement.diagnostics.is_empty(),
        "{:?}",
        web.arrangement.diagnostics
    );
    assert_eq!(web.fills, native.fills);
    let output = serde_json::to_string(web.arrangement.score.as_ref().unwrap()).unwrap();
    let native_output = serde_json::to_string(native.arrangement.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&output), playback(&native_output));
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let mscz = acorde_wasm::serialize_mscz(&output).unwrap();
    let imported = acorde_wasm::parse_mscz(&mscz).unwrap();
    assert_eq!(playback(&output), playback(&imported));
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&output).unwrap();
        let imported = acorde_wasm::parse_musicxml(&xml).unwrap();
        assert_eq!(playback(&output), playback(&imported));
    }
    let mut invalid = response;
    invalid["rest_notes"][1]["note"] = json!(0);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_with_response_fills(
            &input,
            &accompaniment.to_string(),
            &invalid.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected["arrangement"]["score"].is_null());
    assert_eq!(
        rejected["arrangement"]["diagnostics"][0]["code"],
        "response-rest"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn phrase_ending_response_survives_export() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc2 z2 | e4 |").unwrap();
    let accompaniment = json!({
        "chords":[{"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}],
        "bass_mode":"free-bass", "default_pattern":null, "sections":[]
    });
    let response = json!({"rest_notes":[{"part":0,"staff":0,"measure":0,"voice":0,"note":1}]});
    let result: acorde_core::ResponseFilledAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_with_response_fills(
            &input,
            &accompaniment.to_string(),
            &response.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = serde_json::to_string(result.arrangement.score.as_ref().unwrap()).unwrap();
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let mscz = acorde_wasm::serialize_mscz(&output).unwrap();
    assert_eq!(
        playback(&output),
        playback(&acorde_wasm::parse_mscz(&mscz).unwrap())
    );
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&output).unwrap();
        assert_eq!(
            playback(&output),
            playback(&acorde_wasm::parse_musicxml(&xml).unwrap())
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn response_timing_spacers_are_hidden_and_silent_measures_have_no_extra_voice() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc z2 e | c z2 e |]").unwrap();
    let options = json!({"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}},{"measure_index":1,"offset_beats":0,"chord":{"root":"C","kind":"major"}}],"bass_mode":"standard-bass","default_pattern":"bass-chord-pulse","sections":[]});
    for rest_notes in [
        json!([]),
        json!([{"part":0,"staff":0,"measure":0,"voice":0,"note":1}]),
    ] {
        let result: acorde_core::ResponseFilledAccordionResult = serde_json::from_str(
            &acorde_wasm::arrange_with_response_fills(
                &input,
                &options.to_string(),
                &json!({"rest_notes": rest_notes}).to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        let score = result.arrangement.score.unwrap();
        let bars = &score.parts[0].staves[0].measures;
        assert!(bars[1].voices[1].is_empty());
        assert!(bars[0].voices[0].iter().any(|n| n.is_rest && !n.hidden));
        assert!(
            bars[0].voices[1]
                .iter()
                .filter(|n| n.is_rest)
                .all(|n| n.hidden)
        );
        let serialized = serde_json::to_string(&score).unwrap();
        let svg = acorde_wasm::render_score_svg(&serialized, "{\"interactive\":true}").unwrap();
        for (index, note) in bars[0].voices[1].iter().enumerate() {
            let marker = format!("data-note-addr=\"0:0:0:1:{index}\"");
            let begin = svg.find(&marker).unwrap();
            let tag = svg[begin..].split('>').next().unwrap();
            assert_eq!(tag.contains("visibility=\"hidden\""), note.is_rest);
        }
        let mscz = acorde_wasm::serialize_mscz(&serialized).unwrap();
        let restored = acorde_wasm::parse_mscz(&mscz).unwrap();
        assert_eq!(playback(&serialized), playback(&restored));
        let decoded: acorde_core::Score = serde_json::from_str(&restored).unwrap();
        assert!(
            decoded.parts[0].staves[0].measures[0].voices[1]
                .iter()
                .filter(|n| n.is_rest)
                .all(|n| n.hidden)
        );
        #[cfg(feature = "musicxml")]
        {
            let xml = acorde_wasm::serialize_musicxml(&serialized).unwrap();
            let restored = acorde_wasm::parse_musicxml(&xml).unwrap();
            assert_eq!(playback(&serialized), playback(&restored));
            let decoded: acorde_core::Score = serde_json::from_str(&restored).unwrap();
            assert!(
                decoded.parts[0].staves[0].measures[0].voices[1]
                    .iter()
                    .filter(|n| n.is_rest)
                    .all(|n| n.hidden)
            );
        }
    }
}
