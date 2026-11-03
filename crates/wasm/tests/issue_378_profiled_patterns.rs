#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use acorde_core::model::standard_bass_patterns::{
    ProfiledAccompanimentOptions, ProfiledAccompanimentResult, arrange_profiled_accompaniment,
};
use serde_json::json;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn chord(measure: usize, offset: f64, root: &str, kind: &str) -> serde_json::Value {
    json!({ "measure_index": measure, "offset_beats": offset,
            "chord": { "root": root, "kind": kind, "bass": null } })
}

fn options(
    pattern: &str,
    chords: Vec<serde_json::Value>,
    profile: serde_json::Value,
    allow_omissions: bool,
) -> String {
    json!({
        "pattern": pattern,
        "selection": { "chords": chords, "alternative_chords": null,
                       "inversion": null, "bass_mode": "standard-bass" },
        "profile": profile,
        "allow_chord_omissions": allow_omissions,
        "allow_chord_additions": false
    })
    .to_string()
}

fn g7_profile() -> serde_json::Value {
    json!({
        "bass_buttons": [{ "id": "G bass", "position": 0, "midi": 43 }],
        "chord_buttons": [{ "id": "G7 three tone", "position": 0, "midi": [67, 71, 77] }],
        "max_distance": 0
    })
}

fn c_profile(with_fifth: bool) -> serde_json::Value {
    let mut bass = vec![json!({ "id": "C bass", "position": 0, "midi": 36 })];
    if with_fifth {
        bass.push(json!({ "id": "G bass", "position": 1, "midi": 43 }));
    }
    json!({
        "bass_buttons": bass,
        "chord_buttons": [
            { "id": "C major", "position": 0, "midi": [60, 64, 67] },
            { "id": "G major", "position": 1, "midi": [55, 59, 62] }
        ],
        "max_distance": 1
    })
}

/// Runs WASM and native, asserts they agree, and returns the native result.
fn arrange(abc: &str, options_json: &str) -> (acorde_core::Score, ProfiledAccompanimentResult) {
    let source = acorde_wasm::parse_abc(abc).unwrap();
    let web = acorde_wasm::arrange_profiled_accompaniment(&source, options_json).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let parsed: ProfiledAccompanimentOptions = serde_json::from_str(options_json).unwrap();
    let native = arrange_profiled_accompaniment(&score, &parsed);
    // Scores carry fresh ids, so compare the rest as JSON and the scores by playback.
    let mut web: serde_json::Value = serde_json::from_str(&web).unwrap();
    let mut native_json = serde_json::to_value(&native).unwrap();
    let web_score: Option<acorde_core::Score> =
        serde_json::from_value(web["score"].take()).unwrap();
    native_json["score"] = serde_json::Value::Null;
    assert_eq!(web, native_json);
    assert_eq!(web_score.is_some(), native.score.is_some());
    if let (Some(web_score), Some(native_score)) = (&web_score, &native.score) {
        assert_eq!(events(web_score, 0), events(native_score, 0));
        assert_eq!(events(web_score, 1), events(native_score, 1));
    }
    (score, native)
}

fn left_attacks(score: &acorde_core::Score) -> Vec<Vec<u8>> {
    score.parts[0].staves[1]
        .measures
        .iter()
        .flat_map(|m| &m.voices[0])
        .filter(|n| !n.is_rest)
        .map(|n| n.pitches.iter().map(|p| p.to_midi() as u8).collect())
        .collect()
}

fn events(score: &acorde_core::Score, staff: usize) -> Vec<(u8, u64, u64)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == staff))
        .map(|e| {
            (
                e.pitch_midi,
                (e.time_secs * 1e6).round() as u64,
                (e.duration_secs * 1e6).round() as u64,
            )
        })
        .collect()
}

fn assert_consistent(source: &acorde_core::Score, result: &ProfiledAccompanimentResult) {
    let output = result.score.as_ref().expect("arrangement succeeds");
    assert!(result.requires_performer_review);
    // Each playback attack sounds exactly its action's MIDI, in performance order.
    let left = events(output, 1);
    let mut from_actions = Vec::new();
    for attack in &result.attacks {
        let action = &result.actions[attack.action_index];
        let mut midi = action.sounding_midi.clone();
        midi.sort_unstable();
        for pitch in midi {
            from_actions.push((pitch, (attack.time_secs * 1e6).round() as u64));
        }
    }
    let mut played: Vec<_> = left.iter().map(|(p, t, _)| (*p, *t)).collect();
    played.sort_by_key(|&(p, t)| (t, p));
    from_actions.sort_by_key(|&(p, t)| (t, p));
    assert_eq!(played, from_actions);
    // The selected melody is unchanged.
    assert_eq!(events(output, 0), events(source, 0));
    // MSCZ roundtrip keeps both hands' playback.
    let json = serde_json::to_string(output).unwrap();
    let restored: acorde_core::Score = serde_json::from_str(
        &acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&json).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(events(&restored, 0), events(output, 0));
    assert_eq!(events(&restored, 1), events(output, 1));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn waltz_uses_the_profiled_three_tone_g7() {
    let abc = "X:1\nM:3/4\nL:1/4\nK:C\nG A B|]";
    let chords = vec![chord(0, 0.0, "G", "dominant")];
    let (source, result) = arrange(
        abc,
        &options("waltz-bass", chords.clone(), g7_profile(), true),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.as_ref().unwrap();
    assert_eq!(
        left_attacks(output),
        vec![vec![43], vec![67, 71, 77], vec![67, 71, 77]]
    );
    let ids: Vec<_> = result
        .actions
        .iter()
        .map(|a| a.button_id.as_str())
        .collect();
    assert_eq!(ids, ["G bass", "G7 three tone", "G7 three tone"]);
    assert_eq!(result.actions[1].omitted_pitch_classes, vec![2]);
    assert_eq!(result.actions[1].candidate.omitted_pitch_classes, vec![2]);
    assert_consistent(&source, &result);

    let (_, rejected) = arrange(abc, &options("waltz-bass", chords, g7_profile(), false));
    assert!(rejected.score.is_none() && rejected.actions.is_empty());
    assert_eq!(rejected.diagnostics[0].code, "no-standard-bass-candidate");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn alternating_bass_needs_a_supplied_fifth_button() {
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\nc d e f|]";
    let chords = vec![chord(0, 0.0, "C", "major")];
    let (_, missing) = arrange(
        abc,
        &options("alternating-bass", chords.clone(), c_profile(false), false),
    );
    assert!(missing.score.is_none() && missing.attacks.is_empty());
    assert_eq!(missing.diagnostics[0].code, "missing-alternate-bass-button");

    let (source, result) = arrange(
        abc,
        &options("alternating-bass", chords, c_profile(true), false),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let pressed: Vec<_> = result
        .actions
        .iter()
        .map(|a| (a.button_id.as_str(), a.alternate_bass))
        .collect();
    assert_eq!(
        pressed,
        [
            ("C bass", false),
            ("C major", false),
            ("G bass", true),
            ("C major", false)
        ]
    );
    // A bass attack alone does not sound the chord it belongs to.
    assert_eq!(result.actions[0].omitted_pitch_classes, vec![4, 7]);
    assert_eq!(result.actions[2].omitted_pitch_classes, vec![0, 4]);
    assert_consistent(&source, &result);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn harmony_change_pickup_tie_and_repeat_follow_performance_order() {
    // Waltz with a pickup, a mid-bar chord change and a tied melody note.
    let abc = "X:1\nM:3/4\nL:1/4\nK:C\nG|c2 d|e3-|e2 d|c3|]";
    let chords = vec![
        chord(0, 0.0, "G", "major"),
        chord(1, 0.0, "C", "major"),
        chord(1, 2.0, "G", "major"),
        chord(2, 0.0, "C", "major"),
    ];
    let mut profile = c_profile(true);
    profile["bass_buttons"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "D bass", "position": 2, "midi": 38 }));
    let (source, result) = arrange(
        abc,
        &options("waltz-alternating-bass", chords, profile, false),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.actions.iter().any(|a| a.alternate_bass));
    assert!(result.actions.iter().any(|a| a.button_id == "G major"));
    assert_consistent(&source, &result);

    // A repeated section is attacked again in playback order, and screening applies there.
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\n|:c d e f|g a b c':|]";
    let chords = vec![chord(0, 0.0, "C", "major"), chord(1, 0.0, "G", "major")];
    let base = options("bass-pulse", chords, c_profile(true), false);
    let (source, result) = arrange(abc, &base);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.actions.len(), 8);
    assert_eq!(result.attacks.len(), 16);
    // The return from bar 2 to bar 1 moves the bass button back.
    assert_eq!(result.attacks[8].position_delta, Some(1));
    assert_consistent(&source, &result);

    let mut strict: serde_json::Value = serde_json::from_str(&base).unwrap();
    strict["transition_policy"] = json!({ "max_position_delta": 0, "min_attack_interval_ms": 0 });
    let (_, screened) = arrange(abc, &strict.to_string());
    assert!(screened.score.is_none() && screened.attacks.is_empty());
    assert_eq!(screened.diagnostics[0].code, "transition-too-far");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn offbeat_chords_and_unsupported_requests_are_reported() {
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\nc d e f|]";
    let chords = vec![chord(0, 0.0, "C", "major")];
    let (source, result) = arrange(
        abc,
        &options("offbeat-chords", chords.clone(), c_profile(false), false),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.actions.iter().all(|a| a.button_id == "C major"));
    assert_eq!(result.attacks.len(), 4);
    assert_consistent(&source, &result);

    let (_, meter) = arrange(
        abc,
        &options("waltz-bass", chords.clone(), c_profile(false), false),
    );
    assert_eq!(meter.diagnostics[0].code, "unsupported-meter");
    let (_, pattern) = arrange(
        abc,
        &options("walking-bass", chords, c_profile(false), false),
    );
    assert_eq!(pattern.diagnostics[0].code, "unsupported-profiled-pattern");

    let capabilities: serde_json::Value =
        serde_json::from_str(&acorde_wasm::profiled_accompaniment_capabilities().unwrap()).unwrap();
    let waltz = capabilities["patterns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "waltz-bass")
        .unwrap();
    assert_eq!(waltz["meters"], json!([[3, 4]]));
    assert_eq!(waltz["melody_layers"], json!(["unchanged"]));
    let walking = capabilities["unsupported"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "walking-bass")
        .unwrap();
    assert_eq!(walking["reason"], "requires free bass");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn every_listed_pattern_and_meter_generates() {
    let capabilities =
        acorde_core::model::standard_bass_patterns::profiled_accompaniment_capabilities();
    for pattern in capabilities.patterns {
        for [numerator, denominator] in pattern.meters {
            let bar = match (numerator, denominator) {
                (9, 8) => "c3 d3 e3",
                (n, 4) => &"c d e f"[..2 * usize::from(n) - 1],
                _ => unreachable!(),
            };
            let abc = format!("X:1\nM:{numerator}/{denominator}\nL:1/4\nK:C\n{bar}|]");
            let abc = if denominator == 8 {
                abc.replace("L:1/4", "L:1/8")
            } else {
                abc
            };
            let (source, result) = arrange(
                &abc,
                &options(
                    &pattern.id,
                    vec![chord(0, 0.0, "C", "major")],
                    c_profile(true),
                    false,
                ),
            );
            assert!(
                result.diagnostics.is_empty(),
                "{} {numerator}/{denominator}: {:?}",
                pattern.id,
                result.diagnostics
            );
            assert_consistent(&source, &result);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn c_bass_with_e_minor_is_cmaj7_only_when_sounding_together() {
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\nc d e f|]";
    let profile = json!({
        "bass_buttons": [{ "id": "C bass", "position": 0, "midi": 36 }],
        "chord_buttons": [{ "id": "E minor", "position": 0, "midi": [64, 67, 71] }],
        "max_distance": 0
    });
    let chords = vec![chord(0, 0.0, "C", "major-seventh")];
    let (source, result) = arrange(abc, &options("bass-pulse", chords, profile, false));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    // The pair together is a complete Cmaj7 ...
    assert!(result.actions[0].candidate.omitted_pitch_classes.is_empty());
    // ... but each separate attack sounds only part of it.
    assert_eq!(result.actions[0].button_id, "C bass");
    assert_eq!(result.actions[0].omitted_pitch_classes, vec![4, 7, 11]);
    assert_eq!(result.actions[1].button_id, "E minor");
    assert_eq!(result.actions[1].omitted_pitch_classes, vec![0]);
    // Melody notes map to the same addresses; every action targets a generated note.
    assert_eq!(result.source_notes.len(), 4);
    assert!(result.source_notes.iter().all(|addr| addr.staff == 0));
    assert!(
        result
            .actions
            .iter()
            .all(|action| result.generated_notes.contains(&action.target))
    );
    assert_consistent(&source, &result);
}
