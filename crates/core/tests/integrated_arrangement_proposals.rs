use std::collections::BTreeSet;

use acorde_core::{
    IntegratedArrangementError, replay_integrated_arrangement, suggest_integrated_arrangements,
    to_playback_events, validate,
};

#[path = "../../../tests/fixtures/arrangement/integrated_arrangement.rs"]
mod fixture;

fn sound(score: &acorde_core::Score) -> Vec<(usize, u64, u64, i32, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|address| {
                (
                    address.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
        })
        .collect()
}

#[test]
fn selects_three_replayable_distinct_compositions_with_source_retained() {
    let source = fixture::source();
    let result = suggest_integrated_arrangements(&source, &fixture::request()).unwrap();
    assert_eq!(result.candidates.len(), 3);
    assert_eq!(result.realizations, 6);
    assert!(result.candidates[1..].iter().any(|candidate| {
        candidate
            .difference_codes
            .iter()
            .any(|code| code == "melody-changed")
    }));
    assert!(result.candidates[1..].iter().any(|candidate| {
        candidate
            .difference_codes
            .iter()
            .any(|code| code == "rhythm-changed")
    }));
    let mut unique = BTreeSet::new();
    for candidate in &result.candidates {
        assert!(validate(&candidate.score).is_valid());
        assert!(unique.insert(sound(&candidate.score)));
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[0].staves[0].measures[1..]).unwrap(),
            serde_json::to_value(&source.parts[0].staves[0].measures[1..]).unwrap()
        );
        assert_eq!(
            sound(&replay_integrated_arrangement(&source, &candidate.resolved).unwrap()),
            sound(&candidate.score)
        );
    }
}

#[test]
fn rejects_excess_budgets_and_stale_source() {
    let source = fixture::source();
    let result = suggest_integrated_arrangements(&source, &fixture::request()).unwrap();
    let mut over_budget = fixture::request();
    over_budget.max_melody_anticipations = 0;
    assert!(matches!(
        suggest_integrated_arrangements(&source, &over_budget),
        Err(IntegratedArrangementError::InvalidRequest)
    ));
    let mut inconsistent_lock = fixture::request();
    inconsistent_lock.rhythms[0].locked_measures.push(1);
    assert!(matches!(
        suggest_integrated_arrangements(&source, &inconsistent_lock),
        Err(IntegratedArrangementError::InvalidRequest)
    ));
    let mut changed = source.clone();
    changed.metadata.title.push('!');
    assert!(matches!(
        replay_integrated_arrangement(&changed, &result.candidates[0].resolved),
        Err(IntegratedArrangementError::SourceMismatch)
    ));
    let mut tampered = result.candidates[0].resolved.clone();
    tampered.melody_variant = 4;
    assert!(matches!(
        replay_integrated_arrangement(&source, &tampered),
        Err(IntegratedArrangementError::SelectionMismatch)
    ));
    let mut limited = fixture::request();
    limited.max_realizations = 1;
    assert!(matches!(
        suggest_integrated_arrangements(&source, &limited),
        Err(IntegratedArrangementError::InsufficientDistinct {
            found: 1,
            search_limited: true,
            ..
        })
    ));
}

#[test]
fn selected_region_keeps_later_source_measures_and_generated_parts_silent() {
    let mut source = fixture::source();
    let mut extra = source.parts[0].staves[0].measures.clone();
    for (index, measure) in extra.iter_mut().enumerate() {
        measure.number = (index + 5) as u32;
        for (voice_index, voice) in measure.voices.iter_mut().enumerate() {
            for (note_index, note) in voice.iter_mut().enumerate() {
                note.id = format!("local-extra-{index}-{voice_index}-{note_index}");
            }
        }
    }
    source.parts[0].staves[0].measures.extend(extra);
    let mut request = fixture::request();
    request.end_measure_exclusive = 4;
    for melody in &mut request.melodies {
        melody.countermelody_active_measures.resize(8, false);
    }
    let result = suggest_integrated_arrangements(&source, &request).unwrap();
    for candidate in &result.candidates {
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[0].staves[0].measures[4..]).unwrap(),
            serde_json::to_value(&source.parts[0].staves[0].measures[4..]).unwrap()
        );
        assert!(
            acorde_core::to_playback_events(&candidate.score, &Default::default())
                .iter()
                .all(|event| event
                    .source
                    .as_ref()
                    .is_none_or(|address| { address.part == 0 || address.measure < 4 }))
        );
    }
}

#[test]
fn modulation_paths_can_supply_harmony_axis() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.harmony = serde_json::from_value(serde_json::json!({
        "base": request.harmony.base,
        "secondary_dominants": [],
        "modulations": [{
            "start_entry": 0,
            "arrival_entry": 3,
            "source_key": {"fifths": 0, "mode": "major"},
            "destination_key": {"fifths": 1, "mode": "major"},
            "approaches": ["common-chord", "dominant"]
        }],
        "locked_chord_entries": [],
        "max_changed_harmonies": 4,
        "count": 2,
        "max_realizations": 64,
        "seed": 42,
        "max_melody_friction_ticks": null
    }))
    .unwrap();
    request.max_realizations = 12;
    let result = suggest_integrated_arrangements(&source, &request).unwrap();
    assert!(result.candidates.iter().any(|candidate| {
        candidate
            .difference_codes
            .iter()
            .any(|code| code == "harmony-changed")
    }));
}
