#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use acorde_core::{Score, to_playback_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn eventide_explicit_melody_and_loss_report_survive_export() {
    let source_json =
        acorde_wasm::parse_mscz(include_bytes!("fixtures/eventide-site-36997ef8.mscz")).unwrap();
    let source: Score = serde_json::from_str(&source_json).unwrap();
    assert_eq!(source.parts.len(), 2);
    let expected: &[&[u8]] = &[
        &[67, 67, 65],
        &[63, 70],
        &[72, 70, 70, 68],
        &[67],
        &[67, 68, 70],
        &[72, 70],
        &[68, 65, 67, 69],
        &[70],
        &[67, 67, 65],
        &[63, 70],
        &[70, 68, 68, 67],
        &[65],
        &[65, 67, 68],
        &[67, 65, 63, 68],
        &[67, 65],
        &[63],
    ];
    let mut selections = Vec::new();
    for (mi, bar) in source.parts[0].staves[0].measures.iter().enumerate() {
        assert_eq!(bar.voices[0].len(), expected[mi].len());
        for (ni, note) in bar.voices[0].iter().enumerate() {
            let index = note
                .pitches
                .iter()
                .position(|p| p.to_midi() == i16::from(expected[mi][ni]))
                .unwrap();
            if note.pitches.len() > 1 {
                selections.push(json!({"source":{"part":0,"staff":0,"measure":mi,"voice":0,"note":ni},"pitch_index":index}));
            }
        }
    }
    assert_eq!(selections.len(), 34);
    let bass = [
        ("Eb", 39),
        ("Bb", 46),
        ("Ab", 44),
        ("F", 41),
        ("C", 36),
        ("G", 43),
    ];
    let chord_buttons: [(&str, &str, &[u8]); 6] = [
        ("Eb", "major", &[63, 67, 70]),
        ("Bb", "major", &[70, 74, 77]),
        ("Ab", "major", &[68, 72, 75]),
        ("F", "minor", &[65, 68, 72]),
        ("C", "minor", &[60, 63, 67]),
        ("G", "minor", &[67, 70, 74]),
    ];
    let profile = json!({"bass_buttons":bass.iter().map(|(r,m)|json!({"id":format!("{r} bass"),"position":0,"midi":m})).collect::<Vec<_>>(),"chord_buttons":chord_buttons.iter().map(|(r,k,m)|json!({"id":format!("{r} {k}"),"position":0,"midi":m})).collect::<Vec<_>>(),"max_distance":0});
    let locations: Vec<Value> = bass
        .iter()
        .enumerate()
        .map(|(i, (r, _))| json!({"id":format!("{r} bass"),"row":"fundamental","column":i}))
        .chain(
            chord_buttons
                .iter()
                .enumerate()
                .map(|(i, (r, k, _))| json!({"id":format!("{r} {k}"),"row":k,"column":i})),
        )
        .collect();
    let instrument = json!({"profile":profile,"locations":locations});
    let chords = ["Eb","Bb","Ab","Eb","Ab","Eb","F","Bb","Eb","Bb","Ab","Bb","Bb","Ab","Bb","Eb"].iter().enumerate().map(|(i,r)|json!({"measure_index":i,"offset_beats":0,"chord":{"root":r,"kind":if *r=="F" {"minor"} else {"major"}}})).collect::<Vec<_>>();
    let options = json!({"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":true,"chords":chords,"profile":profile,"allow_chord_omissions":false,"allow_chord_additions":false});
    let policy = json!({"max_position_delta":0,"min_attack_interval_ms":0});
    let result: Value = serde_json::from_str(
        &acorde_wasm::find_located_standard_bass_path(
            &source_json,
            &options.to_string(),
            &json!(selections).to_string(),
            &instrument.to_string(),
            &policy.to_string(),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    let arrangement = &result["arrangement"]["arrangement"];
    assert_eq!(arrangement["diagnostics"], json!([]));
    assert_eq!(arrangement["requires_performer_review"], true);
    assert_eq!(result["transitions"].as_array().unwrap().len(), 16);
    let missing: Value = serde_json::from_str(
        &acorde_wasm::find_located_standard_bass_path(
            &source_json,
            &options.to_string(),
            "[]",
            &instrument.to_string(),
            &policy.to_string(),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        missing["arrangement"]["arrangement"]["diagnostics"][0]["code"],
        "ambiguous-melody"
    );
    assert!(missing["arrangement"]["arrangement"]["score"].is_null());
    assert_eq!(missing["transitions"], json!([]));
    let mut denied_options = options.clone();
    denied_options["allow_source_omissions"] = json!(false);
    let denied: Value = serde_json::from_str(
        &acorde_wasm::find_located_standard_bass_path(
            &source_json,
            &denied_options.to_string(),
            &json!(selections).to_string(),
            &instrument.to_string(),
            &policy.to_string(),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        denied["arrangement"]["arrangement"]["diagnostics"][0]["code"],
        "source-omission-not-authorized"
    );
    assert!(denied["arrangement"]["arrangement"]["score"].is_null());
    let mappings = arrangement["source_pitches"].as_array().unwrap();
    let mut expected_addresses = BTreeSet::new();
    for (pi, part) in source.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            for (mi, bar) in staff.measures.iter().enumerate() {
                for (vi, voice) in bar.voices.iter().enumerate() {
                    for (ni, note) in voice.iter().enumerate() {
                        if !note.is_rest {
                            for pitch in 0..note.pitches.len() {
                                expected_addresses.insert((pi, si, mi, vi, ni, pitch));
                            }
                        }
                    }
                }
            }
        }
    }
    let actual: BTreeSet<_> = mappings
        .iter()
        .map(|m| {
            let a = &m["source"];
            (
                a["part"].as_u64().unwrap() as usize,
                a["staff"].as_u64().unwrap() as usize,
                a["measure"].as_u64().unwrap() as usize,
                a["voice"].as_u64().unwrap() as usize,
                a["note"].as_u64().unwrap() as usize,
                m["source_pitch"].as_u64().unwrap() as usize,
            )
        })
        .collect();
    assert_eq!(mappings.len(), 157);
    assert_eq!(actual.len(), mappings.len());
    assert_eq!(actual, expected_addresses);
    assert_eq!(
        mappings.iter().filter(|m| m["target"].is_null()).count(),
        117
    );
    let output: Score = serde_json::from_value(arrangement["score"].clone()).unwrap();
    let melody_source = to_playback_events(&source, &Default::default())
        .into_iter()
        .filter(|e| {
            e.source.as_ref().is_some_and(|a| {
                a.part == 0
                    && a.staff == 0
                    && a.voice == 0
                    && e.pitch_midi == expected[a.measure][a.note]
            })
        })
        .map(|e| {
            (
                e.pitch_midi,
                e.time_beats,
                e.duration_beats,
                e.time_secs,
                e.duration_secs,
            )
        })
        .collect::<Vec<_>>();
    let melody_output = to_playback_events(&output, &Default::default())
        .into_iter()
        .filter(|e| {
            e.source
                .as_ref()
                .is_some_and(|a| a.part == 0 && a.staff == 0 && a.voice == 0)
        })
        .map(|e| {
            (
                e.pitch_midi,
                e.time_beats,
                e.duration_beats,
                e.time_secs,
                e.duration_secs,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(melody_source, melody_output);
    let score_json = serde_json::to_string(&output).unwrap();
    assert!(
        acorde_wasm::render_score_svg(&score_json, "{}")
            .unwrap()
            .contains("<svg")
    );
    let encoded = acorde_wasm::serialize_mscz(&score_json).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&encoded).unwrap()).unwrap();
    let playback = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| {
                (
                    e.pitch_midi,
                    e.time_beats,
                    e.duration_beats,
                    e.time_secs,
                    e.duration_secs,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(playback(&output), playback(&decoded));
    assert_eq!(
        serde_json::from_str::<Score>(&source_json)
            .unwrap()
            .parts
            .len(),
        2
    );
}
