#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn playback(score_json: &str) -> Vec<(u8, u64, u64, usize)> {
    let score: acorde_core::Score = serde_json::from_str(score_json).unwrap();
    acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    event.pitch_midi,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    source.part,
                )
            })
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn chordal_pad_melody_clearance_matches_native_and_survives_exports() {
    let mut source = acorde_core::Score::new("clearance", 120, 4, 4, 0, 1);
    source.parts[0].staves[0].measures[0].voices[0] = vec![
        acorde_core::Note::new(
            acorde_core::Pitch::from_midi(72, false),
            acorde_core::Duration::Half,
        ),
        acorde_core::Note::new(
            acorde_core::Pitch::from_midi(60, false),
            acorde_core::Duration::Half,
        ),
    ];
    let input = serde_json::to_string(&source).unwrap();
    let options = json!({
        "chords":[{"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}],
        "pattern":"held", "voicing":"root-position", "midi_program":48,
        "low_midi":48, "high_midi":84, "center_midi":64,
        "min_melody_gap_semitones":4
    });
    let web: acorde_core::ChordalPadResult = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &options.to_string()).unwrap(),
    )
    .unwrap();
    let native =
        acorde_core::arrange_chordal_pad(&source, serde_json::from_value(options).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
    let native = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&generated), playback(&native));
    let pad = &web.score.as_ref().unwrap().parts[1].staves[0].measures[0].voices[0][0];
    assert!(pad.pitches.iter().all(|pitch| pitch.to_midi() <= 56));
    let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
    assert_eq!(
        playback(&generated),
        playback(&acorde_wasm::parse_mscz(&mscz).unwrap())
    );
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
        assert_eq!(
            playback(&generated),
            playback(&acorde_wasm::parse_musicxml(&xml).unwrap())
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn metered_phrase_pad_matches_native_and_survives_exports() {
    let source = acorde_core::Score::new("metered phrase", 120, 5, 8, 0, 2);
    let input = serde_json::to_string(&source).unwrap();
    let options = json!({
        "chords":[
            {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
            {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}}
        ],
        "pattern":"meter-pulse",
        "meter_group_sections":[{"start_measure":1,"end_measure_exclusive":2,"groups":[2,3]}],
        "phrase_end_measures":[1],
        "midi_program":48, "low_midi":48, "high_midi":84, "center_midi":64
    });
    let web: acorde_core::ChordalPadResult = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &options.to_string()).unwrap(),
    )
    .unwrap();
    let native =
        acorde_core::arrange_chordal_pad(&source, serde_json::from_value(options).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    assert_eq!(web.resolved_meter_groups, native.resolved_meter_groups);
    assert_eq!(web.phrase_start_ticks, native.phrase_start_ticks);
    assert_eq!(web.resolved_meter_groups, vec![vec![3, 2], vec![2, 3]]);
    assert_eq!(web.phrase_start_ticks, vec![None, Some(0)]);
    let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
    let expected = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&generated), playback(&expected));
    let has_accent = |json: &str| {
        let score: acorde_core::Score = serde_json::from_str(json).unwrap();
        score.parts[1].staves[0].measures[0].voices[0][0]
            .articulations
            .contains(&acorde_core::Articulation::Accent)
    };
    assert!(has_accent(&generated));
    let attacks = playback(&generated)
        .into_iter()
        .filter(|event| event.3 == 1)
        .map(|event| event.1)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(attacks, [0, 720, 1200].into_iter().collect());
    let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
    let imported = acorde_wasm::parse_mscz(&mscz).unwrap();
    assert_eq!(playback(&generated), playback(&imported));
    assert!(has_accent(&imported));
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
        let imported = acorde_wasm::parse_musicxml(&xml).unwrap();
        assert_eq!(playback(&generated), playback(&imported));
        assert!(has_accent(&imported));
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn chordal_pad_preserves_existing_parts_and_survives_exports() {
    let mut source: acorde_core::Score =
        serde_json::from_str(&acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc4 | d4 |").unwrap())
            .unwrap();
    let mut second = acorde_core::Part::new("Second", "2nd");
    second.midi_channel = 1;
    second.midi_program = 40;
    let mut staff = acorde_core::Staff::new(acorde_core::Clef::Treble);
    for number in 1..=2 {
        let mut bar = acorde_core::Measure::empty(4, 4);
        bar.number = number;
        bar.voices[0] = vec![acorde_core::Note::new(
            acorde_core::Pitch::from_midi(76, false),
            acorde_core::Duration::Whole,
        )];
        staff.measures.push(bar);
    }
    second.staves.push(staff);
    source.parts.push(second);
    let input = serde_json::to_string(&source).unwrap();
    let options = json!({
        "chords":[
            {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
            {"measure_index":0,"offset_beats":2.0,"chord":{"root":"F","kind":"major"}}
        ],
        "pattern":"quarter-pulse", "midi_program":48,
        "low_midi":48, "high_midi":84, "center_midi":64
    });
    let web: acorde_core::ChordalPadResult = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &options.to_string()).unwrap(),
    )
    .unwrap();
    let native =
        acorde_core::arrange_chordal_pad(&source, serde_json::from_value(options.clone()).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    assert_eq!(web.added_part, Some(2));
    assert_eq!(web.generated_notes, native.generated_notes);
    assert_eq!(web.voicings, native.voicings);
    assert_eq!(web.motion_report, native.motion_report);
    let output = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
    let native_output = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&output), playback(&native_output));
    assert_eq!(
        serde_json::to_value(&web.score.as_ref().unwrap().parts[..2]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
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
    for pattern in ["offbeat-pulse", "arpeggio-up-down", "meter-pulse"] {
        let mut patterned = options.clone();
        patterned["pattern"] = json!(pattern);
        let web: acorde_core::ChordalPadResult = serde_json::from_str(
            &acorde_wasm::arrange_chordal_pad(&input, &patterned.to_string()).unwrap(),
        )
        .unwrap();
        let native =
            acorde_core::arrange_chordal_pad(&source, serde_json::from_value(patterned).unwrap());
        assert!(
            web.diagnostics.is_empty(),
            "{pattern}: {:?}",
            web.diagnostics
        );
        let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
        let native = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
        assert_eq!(playback(&generated), playback(&native), "{pattern}");
        let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
        assert_eq!(
            playback(&generated),
            playback(&acorde_wasm::parse_mscz(&mscz).unwrap())
        );
        #[cfg(feature = "musicxml")]
        {
            let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
            assert_eq!(
                playback(&generated),
                playback(&acorde_wasm::parse_musicxml(&xml).unwrap()),
                "{pattern}"
            );
        }
    }
    for voicing in ["smooth", "open", "global-smooth"] {
        let mut voiced = options.clone();
        voiced["pattern"] = json!("held");
        voiced["voicing"] = json!(voicing);
        let web: acorde_core::ChordalPadResult = serde_json::from_str(
            &acorde_wasm::arrange_chordal_pad(&input, &voiced.to_string()).unwrap(),
        )
        .unwrap();
        let native =
            acorde_core::arrange_chordal_pad(&source, serde_json::from_value(voiced).unwrap());
        assert!(
            web.diagnostics.is_empty(),
            "{voicing}: {:?}",
            web.diagnostics
        );
        let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
        let native = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
        assert_eq!(playback(&generated), playback(&native), "{voicing}");
        let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
        assert_eq!(
            playback(&generated),
            playback(&acorde_wasm::parse_mscz(&mscz).unwrap()),
            "{voicing}"
        );
        #[cfg(feature = "musicxml")]
        {
            let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
            assert_eq!(
                playback(&generated),
                playback(&acorde_wasm::parse_musicxml(&xml).unwrap()),
                "{voicing}"
            );
        }
    }
    let mut sectioned = options.clone();
    sectioned["pattern"] = json!("held");
    sectioned["pattern_sections"] = json!([{
        "start_measure": 1,
        "end_measure_exclusive": 2,
        "pattern": "offbeat-pulse"
    }]);
    let web: acorde_core::ChordalPadResult = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &sectioned.to_string()).unwrap(),
    )
    .unwrap();
    let native =
        acorde_core::arrange_chordal_pad(&source, serde_json::from_value(sectioned).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
    let native = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&generated), playback(&native));
    assert_eq!(
        web.score.as_ref().unwrap().parts[2].staves[0].measures[0].voices[0].len(),
        2
    );
    assert_eq!(
        web.score.as_ref().unwrap().parts[2].staves[0].measures[1].voices[0].len(),
        8
    );
    let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
    assert_eq!(
        playback(&generated),
        playback(&acorde_wasm::parse_mscz(&mscz).unwrap())
    );
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
        assert_eq!(
            playback(&generated),
            playback(&acorde_wasm::parse_musicxml(&xml).unwrap())
        );
    }
    let mut bad_sections = options.clone();
    bad_sections["pattern_sections"] = json!([{
        "start_measure": 1,
        "end_measure_exclusive": 3,
        "pattern": "offbeat-pulse"
    }]);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &bad_sections.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected["score"].is_null());
    assert_eq!(rejected["diagnostics"][0]["code"], "pattern-section");
    let mut invalid = options;
    invalid["chords"][0]["offset_beats"] = json!(0.015625);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_chordal_pad(&input, &invalid.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected["score"].is_null());
    assert_eq!(rejected["diagnostics"][0]["code"], "chord-position");
}
