#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use acorde_core::{Barline, Duration, MeasureLength, Note, Pitch, Score, Step};
use serde_json::{Value, json};

fn profile() -> Value {
    json!({
        "bass_buttons":[{"id":"C bass","position":0,"midi":36}],
        "chord_buttons":[
            {"id":"C major","position":1,"midi":[60,64,67]},
            {"id":"G major","position":1,"midi":[67,71,74]}
        ],
        "max_distance":1
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn two_chord_buttons_are_reported_with_a_label() {
    let found: Value = serde_json::from_str(
        &acorde_wasm::standard_bass_double_chord_candidates(
            &profile().to_string(),
            r#"{"root":"C","kind":"major-seventh","bass":null}"#,
            true,
            true,
        )
        .unwrap(),
    )
    .unwrap();
    // No single button gives Cmaj7 here; C major + G major do, adding a D that is reported.
    assert_eq!(found[0]["label"], "C bass + C chord + G chord");
    assert_eq!(found[0]["added_pitch_classes"], json!([2]));
    assert_eq!(found[0]["requires_performer_review"], true);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn phrases_and_proposed_harmony_round_trip_through_json() {
    let mut source = Score::new("phrases", 120, 4, 4, 0, 2);
    let staff = &mut source.parts[0].staves[0];
    staff.measures[0].actual_length = MeasureLength::from_beats(1.0);
    staff.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
    staff.measures[0].voices[1] = vec![Note::new(Pitch::new(Step::E, 5), Duration::Quarter)];
    staff.measures[1].voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
    staff.measures[1].voices[1] = vec![Note::new(Pitch::new(Step::F, 5), Duration::Whole)];
    staff.measures[1].barline_right = Barline::RepeatEnd;
    let options = json!({
        "melody_part":0,"melody_staff":0,"melody_voice":1,"allow_source_omissions":true,
        "chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major-seventh"}}],
        "profile":{"bass_buttons":[{"id":"C bass","position":0,"midi":36}],
            "chord_buttons":[{"id":"E minor","position":1,"midi":[64,67,71]}],"max_distance":1},
        "allow_chord_omissions":false,"allow_chord_additions":false
    });
    let result: Value = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_with_phrases(
            &serde_json::to_string(&source).unwrap(),
            &options.to_string(),
            r#"[{"first_measure":1,"last_measure":1,"part":0,"staff":0,"voice":0}]"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["diagnostics"], json!([]));
    assert_eq!(
        result["edition_kinds"],
        json!(["combined-buttons", "simplified"])
    );
    assert_eq!(
        result["actions"][0]["candidate"]["label"],
        "C bass + Em chord"
    );
    let proposed: Value = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_proposed_harmony(
            &serde_json::to_string(&source).unwrap(),
            &options.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proposed["harmony_source"], "editorial-proposal");
}
