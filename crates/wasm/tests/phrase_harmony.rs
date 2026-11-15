#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn phrase_harmony_json_matches_native_and_rejects_invalid_schedule() {
    let score_json =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC E G B | C E G B | C E G B | C E G B |\n")
            .unwrap();
    let request = acorde_core::PhraseHarmonyRequest {
        phrases: vec![
            acorde_core::PhraseHarmonySection {
                start_measure: 0,
                end_measure_exclusive: 2,
                pace: acorde_core::PhraseHarmonicPace::EverySecondBar,
                meter_groups: vec![],
            },
            acorde_core::PhraseHarmonySection {
                start_measure: 2,
                end_measure_exclusive: 4,
                pace: acorde_core::PhraseHarmonicPace::TwoGroupsPerBar,
                meter_groups: vec![],
            },
        ],
        proposal: Default::default(),
        locked_chord_entries: vec![],
        source_reanchor: None,
        local_keys: vec![],
        cadences: vec![],
    };
    let native =
        acorde_core::suggest_phrase_harmony(&serde_json::from_str(&score_json).unwrap(), &request);
    let wasm: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&wasm).unwrap()
    );
    assert!(wasm.diagnostics.is_empty(), "{:?}", wasm.diagnostics);
    assert_eq!(wasm.positions.len(), 5);
    assert_eq!(wasm.positions[2].offset.beats(), 2.0);
    assert!(!wasm.proposals.candidates.is_empty());
    let mut invalid = request;
    invalid.phrases[1].start_measure = 1;
    let rejected: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&invalid).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected.positions.is_empty());
    assert!(rejected.proposals.candidates.is_empty());
    assert_eq!(rejected.diagnostics[0].code, "invalid-harmonic-phrase");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn source_reanchor_keeps_locked_chord_and_provenance_in_wasm() {
    let score_json =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC E G B | C E G B |\n").unwrap();
    let chord: acorde_core::ChordSymbol =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major"})).unwrap();
    let source_plan = acorde_core::ChordPlan {
        contract_version: 1,
        entries: (0..2)
            .map(|measure| acorde_core::ChordPlanEntry {
                position: acorde_core::ChordPlanPosition {
                    measure_index: measure,
                    offset: acorde_core::BeatFraction::ZERO,
                },
                offset_beats: 0.0,
                chord: chord.clone(),
                origin: acorde_core::ChordPlanOrigin::HostAuthored,
                sources: vec![acorde_core::ChordPlanSource {
                    note: None,
                    reference: Some(format!("host:bar-{measure}")),
                }],
            })
            .collect(),
    };
    let mut proposal = acorde_core::ChordProposalRequest::default();
    proposal.source_plan = Some(source_plan.clone());
    let request = acorde_core::PhraseHarmonyRequest {
        phrases: vec![acorde_core::PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: 2,
            pace: acorde_core::PhraseHarmonicPace::TwoGroupsPerBar,
            meter_groups: vec![],
        }],
        proposal,
        locked_chord_entries: vec![1],
        source_reanchor: Some(vec![0, 0, 1, 1]),
        local_keys: vec![],
        cadences: vec![],
    };
    let native =
        acorde_core::suggest_phrase_harmony(&serde_json::from_str(&score_json).unwrap(), &request);
    let wasm: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&wasm).unwrap()
    );
    assert!(wasm.diagnostics.is_empty(), "{:?}", wasm.diagnostics);
    let reanchored = wasm.reanchored_source_plan.unwrap();
    assert_eq!(
        reanchored.entries[3].sources,
        source_plan.entries[1].sources
    );
    assert_eq!(
        reanchored.entries[3].origin,
        acorde_core::ChordPlanOrigin::GeneratedReharmonization
    );
    assert!(wasm.proposals.candidates.iter().all(|candidate| {
        candidate.plan.entries.iter().any(|entry| {
            entry.position == source_plan.entries[1].position
                && entry.chord == source_plan.entries[1].chord
                && entry.origin == source_plan.entries[1].origin
                && entry.sources == source_plan.entries[1].sources
        })
    }));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn local_key_schedule_matches_native_without_changing_source_identity() {
    let score_json = acorde_wasm::parse_abc(
        "X:1\nM:4/4\nL:1/4\nK:C\nC E G B | C E G B | ^F A c ^F | ^F A c ^F |\n",
    )
    .unwrap();
    let mut request = acorde_core::PhraseHarmonyRequest {
        phrases: vec![acorde_core::PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: 4,
            pace: acorde_core::PhraseHarmonicPace::EveryBar,
            meter_groups: vec![],
        }],
        proposal: Default::default(),
        locked_chord_entries: vec![],
        source_reanchor: None,
        local_keys: vec![],
        cadences: vec![],
    };
    let source: acorde_core::Score = serde_json::from_str(&score_json).unwrap();
    let baseline = acorde_core::suggest_phrase_harmony(&source, &request);
    request.local_keys = vec![acorde_core::ChordKeyRegion {
        start_measure: 2,
        end_measure_exclusive: 4,
        key: acorde_core::KeySignature {
            fifths: 1,
            mode: "major".into(),
        },
    }];
    request.cadences = vec![acorde_core::ChordCadenceTarget {
        arrival_position_index: 3,
        kind: acorde_core::ChordCadenceKind::DominantTonic,
    }];
    let native = acorde_core::suggest_phrase_harmony(&source, &request);
    let wasm: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&wasm).unwrap()
    );
    assert!(wasm.diagnostics.is_empty(), "{:?}", wasm.diagnostics);
    assert_eq!(wasm.applied_local_keys, request.local_keys);
    assert_eq!(wasm.applied_cadences, request.cadences);
    assert_eq!(
        wasm.proposals.input_fingerprint,
        baseline.proposals.input_fingerprint
    );
    assert!(!wasm.proposals.candidates.is_empty());
    assert!(wasm.proposals.candidates.iter().all(|candidate| {
        candidate
            .plan
            .entries
            .iter()
            .any(|entry| entry.position == wasm.positions[2] && entry.chord.root == "D")
            && candidate
                .plan
                .entries
                .iter()
                .any(|entry| entry.position == wasm.positions[3] && entry.chord.root == "G")
    }));
}
