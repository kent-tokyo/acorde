#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use acorde_core::model::standard_bass_transposition::{
    StandardBassTranspositionSearch, TransposedStandardBassOptions, TransposedStandardBassResult,
    TranspositionStatus, arrange_standard_bass_transposed, evaluate_standard_bass_transpositions,
};
use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

fn chord(measure: usize, offset: f64, root: &str, bass: Option<&str>) -> Value {
    json!({ "measure_index": measure, "offset_beats": offset,
            "chord": { "root": root, "kind": "major", "bass": bass } })
}

/// Bass and major-chord buttons for the given pitch classes, sharing a position per root.
fn profile(roots: &[u8]) -> Value {
    json!({
        "bass_buttons": roots.iter().map(|&pc| json!({
            "id": format!("{} bass", NAMES[pc as usize]), "position": pc, "midi": 36 + pc
        })).collect::<Vec<_>>(),
        "chord_buttons": roots.iter().map(|&pc| json!({
            "id": format!("{} major", NAMES[pc as usize]), "position": pc,
            "midi": [60 + pc, 64 + pc, 67 + pc]
        })).collect::<Vec<_>>(),
        "max_distance": 11
    })
}

fn arrangement(chords: Vec<Value>, profile: Value) -> Value {
    json!({
        "melody_part": 0, "melody_staff": 0, "melody_voice": 0,
        "allow_source_omissions": false,
        "chords": chords,
        "profile": profile,
        "allow_chord_omissions": false, "allow_chord_additions": false
    })
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

/// Runs WASM and native, asserts they agree, and returns the source and native result.
fn transposed(
    abc: &str,
    arrangement: &Value,
    semitones: i8,
    policy: Option<Value>,
) -> (acorde_core::Score, TransposedStandardBassResult) {
    let options = json!({ "arrangement": arrangement, "semitones": semitones,
                          "transition_policy": policy })
    .to_string();
    let source = acorde_wasm::parse_abc(abc).unwrap();
    let web = acorde_wasm::arrange_standard_bass_transposed(&source, &options).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let parsed: TransposedStandardBassOptions = serde_json::from_str(&options).unwrap();
    let native = arrange_standard_bass_transposed(&score, &parsed);
    // Scores carry fresh ids, so compare the rest as JSON and the scores by playback.
    let mut web: Value = serde_json::from_str(&web).unwrap();
    let mut native_json = serde_json::to_value(&native).unwrap();
    let web_score: Option<acorde_core::Score> =
        serde_json::from_value(web["arrangement"]["score"].take()).unwrap();
    native_json["arrangement"]["score"] = Value::Null;
    assert_eq!(web, native_json);
    assert_eq!(web_score.is_some(), native.arrangement.score.is_some());
    if let (Some(a), Some(b)) = (&web_score, &native.arrangement.score) {
        assert_eq!(events(a, 0), events(b, 0));
        assert_eq!(events(a, 1), events(b, 1));
    }
    (score, native)
}

fn pressed(result: &TransposedStandardBassResult) -> Vec<(String, String, Vec<u8>)> {
    result
        .arrangement
        .actions
        .iter()
        .map(|a| {
            let mut midi = vec![a.candidate.bass_midi];
            midi.extend(&a.candidate.chord_midi);
            (
                a.candidate.bass_button_id.clone(),
                a.candidate.chord_button_id.clone(),
                midi,
            )
        })
        .collect()
}

const ISSUE_ABC: &str = "X:1\nM:4/4\nL:1/4\nK:C\nC D E F|]";

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn c_only_profile_needs_explicit_d_buttons_after_transposition() {
    let chords = vec![chord(0, 0.0, "C", None)];
    let c_only = arrangement(chords.clone(), profile(&[0]));
    let (_, original) = transposed(ISSUE_ABC, &c_only, 0, None);
    assert!(original.arrangement.diagnostics.is_empty());
    assert_eq!(
        pressed(&original),
        [("C bass".into(), "C major".into(), vec![36, 60, 64, 67])]
    );

    let (_, missing) = transposed(ISSUE_ABC, &c_only, 2, None);
    assert!(missing.arrangement.score.is_none());
    assert!(
        missing.arrangement.actions.is_empty() && missing.arrangement.source_pitches.is_empty()
    );
    assert_eq!(
        missing.arrangement.diagnostics[0].code,
        "no-standard-bass-candidate"
    );

    let with_d = arrangement(chords, profile(&[0, 2]));
    let (_, shifted) = transposed(ISSUE_ABC, &with_d, 2, None);
    assert!(shifted.arrangement.diagnostics.is_empty());
    assert_eq!(
        pressed(&shifted),
        [("D bass".into(), "D major".into(), vec![38, 62, 66, 69])]
    );
    // No original action or harmony survives; the profile itself is unchanged.
    let resolved = shifted.arrangement.resolved_options.as_ref().unwrap();
    assert_eq!(resolved.chords[0].chord.root, "D");
    assert_eq!(resolved.profile, shifted.source_options.profile);
    assert_eq!(shifted.source_options.chords[0].chord.root, "C");
    assert!(shifted.arrangement.requires_performer_review);

    // Key-aware respelling may produce a double flat: Db in C down a major third is Bbb.
    let (_, double) = transposed(
        "X:1\nM:4/4\nL:1/4\nK:C\nc d e f|]",
        &arrangement(vec![chord(0, 0.0, "Db", None)], profile(&[1, 9])),
        -4,
        None,
    );
    assert!(
        double.arrangement.diagnostics.is_empty(),
        "{:?}",
        double.arrangement.diagnostics
    );
    let resolved = double.arrangement.resolved_options.as_ref().unwrap();
    assert_eq!(resolved.chords[0].chord.root, "Bbb");
    assert_eq!(pressed(&double)[0].0, "A bass");

    // c'''' is MIDI 120; +8 leaves the MIDI range before any arrangement is attempted.
    let (_, high) = transposed("X:1\nM:4/4\nL:1/4\nK:C\nc'''' D E F|]", &with_d, 8, None);
    assert!(high.arrangement.score.is_none());
    assert_eq!(
        high.arrangement.diagnostics[0].code,
        "transposition-out-of-range"
    );
}

// Repeats, a tie, a slash chord and a local key change.
const SONG: &str = "X:1\nM:4/4\nL:1/4\nK:C\n|:c2 B2|c4-|c4:|[K:F]A2 B2|]";

fn song_chords() -> Vec<Value> {
    vec![
        chord(0, 0.0, "C", None),
        chord(0, 2.0, "G", Some("B")),
        chord(1, 0.0, "C", None),
        chord(3, 0.0, "F", None),
        chord(3, 2.0, "Bb", None),
    ]
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn harmony_spelling_melody_and_structure_follow_the_transposition() {
    let all = arrangement(song_chords(), profile(&(0..12).collect::<Vec<_>>()));
    let policy = json!({ "max_position_delta": 11, "min_attack_interval_ms": 0 });
    for (semitones, expected) in [
        (
            2,
            [
                ("D", None),
                ("A", Some("C#")),
                ("D", None),
                ("G", None),
                ("C", None),
            ],
        ),
        (
            3,
            [
                ("Eb", None),
                ("Bb", Some("D")),
                ("Eb", None),
                ("Ab", None),
                ("Db", None),
            ],
        ),
    ] {
        let (source, result) = transposed(SONG, &all, semitones, Some(policy.clone()));
        assert!(
            result.arrangement.diagnostics.is_empty(),
            "{:?}",
            result.arrangement.diagnostics
        );
        let resolved = result.arrangement.resolved_options.as_ref().unwrap();
        let spelled: Vec<_> = resolved
            .chords
            .iter()
            .map(|c| (c.chord.root.as_str(), c.chord.bass.as_deref()))
            .collect();
        assert_eq!(spelled, expected);
        let output = result.arrangement.score.as_ref().unwrap();
        // The melody moves by exactly the interval, with timing, ties and repeats kept.
        let shifted: Vec<_> = events(&source, 0)
            .into_iter()
            .map(|(p, t, d)| (p + semitones as u8, t, d))
            .collect();
        assert_eq!(events(output, 0), shifted);
        let (from, to) = (&source.parts[0].staves[0], &output.parts[0].staves[0]);
        for (a, b) in from.measures.iter().zip(&to.measures) {
            assert_eq!(
                (&a.barline_left, &a.barline_right),
                (&b.barline_left, &b.barline_right)
            );
            let shape = |m: &acorde_core::Measure| {
                m.voices[0]
                    .iter()
                    .map(|n| (n.duration.clone(), n.dot_count, n.tie_start, n.tie_end))
                    .collect::<Vec<_>>()
            };
            assert_eq!(shape(a), shape(b));
        }
        assert_eq!(
            to.measures[3].key_sig.as_ref().unwrap().fifths,
            if semitones == 2 { 1 } else { -4 }
        );
        // Screening checked every expanded attack against its buttons, repeats included.
        assert!(result.transitions.len() > result.arrangement.actions.len());
        // Left-hand sound equals the pressed buttons at each attack.
        let attacks: std::collections::BTreeSet<_> = pressed(&result)
            .into_iter()
            .flat_map(|(_, _, midi)| midi)
            .collect();
        assert!(
            events(output, 1)
                .iter()
                .all(|(p, _, _)| attacks.contains(p))
        );

        // MSCZ roundtrip and SVG rendering of the re-arranged edition.
        let json = serde_json::to_string(output).unwrap();
        let restored: acorde_core::Score = serde_json::from_str(
            &acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&json).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(events(&restored, 0), events(output, 0));
        assert_eq!(events(&restored, 1), events(output, 1));
        assert!(
            acorde_wasm::render_score_svg(&json, "{}")
                .unwrap()
                .contains("<svg")
        );
    }

    let strict = json!({ "max_position_delta": 0, "min_attack_interval_ms": 0 });
    let (_, rejected) = transposed(SONG, &all, 2, Some(strict));
    assert!(rejected.arrangement.score.is_none() && rejected.transitions.is_empty());
    assert_eq!(
        rejected.arrangement.diagnostics[0].code,
        "transition-too-far"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn bounded_search_distinguishes_range_buttons_and_limits() {
    // c' is MIDI 84: above +12 it leaves the right hand's 48..=96 range.
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\nc' D E F|]";
    let search = json!({
        "arrangement": arrangement(vec![chord(0, 0.0, "C", None)], profile(&[0, 2])),
        "min_semitones": -2, "max_semitones": 14
    });
    let source = acorde_wasm::parse_abc(abc).unwrap();
    let web: Value = serde_json::from_str(
        &acorde_wasm::evaluate_standard_bass_transpositions(&source, &search.to_string()).unwrap(),
    )
    .unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let parsed: StandardBassTranspositionSearch = serde_json::from_value(search.clone()).unwrap();
    let native = evaluate_standard_bass_transpositions(&score, &parsed);
    assert_eq!(web, serde_json::to_value(&native).unwrap());
    let statuses: Vec<_> = native
        .evaluations
        .iter()
        .map(|e| (e.semitones, e.status))
        .collect();
    use TranspositionStatus::*;
    let expected: Vec<_> = (-2..=14)
        .map(|n| {
            (
                n,
                match n {
                    13.. => OutOfRange,
                    0 | 2 | 12 => Arranged,
                    _ => NoButtonCombination,
                },
            )
        })
        .collect();
    assert_eq!(
        statuses,
        expected,
        "{:?}",
        native
            .evaluations
            .iter()
            .map(|e| &e.diagnostics)
            .collect::<Vec<_>>()
    );

    let mut wide = search;
    wide["min_semitones"] = json!(-11);
    let limited: Value = serde_json::from_str(
        &acorde_wasm::evaluate_standard_bass_transpositions(&source, &wide.to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(limited["evaluations"], json!([]));
    assert_eq!(
        limited["diagnostics"][0]["code"],
        "transposition-search-limit"
    );
}
