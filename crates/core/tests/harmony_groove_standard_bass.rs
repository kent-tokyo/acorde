#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;

use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::{
    HarmonyGrooveButtonFailure, HarmonyGrooveRequest, Score, screen_harmony_groove_standard_bass,
    suggest_harmony_groove_arrangements,
};
use std::collections::BTreeMap;

fn longer_fixture(bars: usize) -> (Score, HarmonyGrooveRequest) {
    let original = fixture::source();
    let mut source = Score::new("standard bass groove", 120, 4, 4, 0, bars as u32);
    source.parts[0].staves[0].measures = (0..bars)
        .map(|i| original.parts[0].staves[0].measures[i % 3].clone())
        .collect();
    let mut bass = original.parts[1].clone();
    bass.staves[0].measures = (0..bars)
        .map(|i| original.parts[1].staves[0].measures[i % 3].clone())
        .collect();
    source.parts.push(bass);
    let mut request = fixture::request();
    let entries = request.base.chords.entries.clone();
    request.base.chords.entries = (0..bars)
        .map(|i| {
            let mut entry = entries[i % entries.len()].clone();
            entry.position.measure_index = i;
            entry
        })
        .collect();
    request.base.end_measure_exclusive = bars;
    request.tensions = (0..bars)
        .map(|i| {
            let mut allowance = request.tensions[i % request.tensions.len()].clone();
            allowance.chord_entry = i;
            allowance
        })
        .collect();
    (source, request)
}

fn profile() -> StandardBassProfile {
    StandardBassProfile {
        bass_buttons: vec![StandardBassNoteButton {
            id: "D counterbass".into(),
            position: 0,
            midi: 38,
        }],
        chord_buttons: vec![StandardBassChordButton {
            id: "D minor".into(),
            position: 0,
            midi: vec![62, 65, 69],
        }],
        max_distance: 2,
    }
}

#[test]
fn each_fixed_length_profile_has_an_exact_playability_verdict_and_replay() {
    for bars in [8, 16] {
        let (source, request) = longer_fixture(bars);
        let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
        assert!(result.candidates.len() >= 2);
        for candidate in result.candidates {
            assert_eq!(
                fixture::sound(&candidate.arrangement.score)
                    .into_iter()
                    .filter(|event| event.0 < source.parts.len())
                    .collect::<Vec<_>>(),
                fixture::sound(&source)
            );
            let report =
                screen_harmony_groove_standard_bass(&source, &candidate.resolved, &profile())
                    .unwrap();
            assert_eq!(
                report.playable_with_supplied_profile,
                report.diagnostics.is_empty()
            );
            assert!(!report.button_states.is_empty() || !report.diagnostics.is_empty());
            for state in &report.button_states {
                assert_eq!(state.sounding_midi, state.button_midi);
                assert!(state.measure < bars);
            }
            for diagnostic in &report.diagnostics {
                assert!(diagnostic.measure.is_some_and(|measure| measure < bars));
                assert!(diagnostic.time_beats.is_some());
                assert_eq!(
                    diagnostic.reason,
                    HarmonyGrooveButtonFailure::NoExactButtons
                );
            }
        }
    }
}

#[test]
fn an_eight_bar_plain_candidate_uses_exact_bass_and_chord_buttons() {
    let (source, request) = longer_fixture(8);
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    let candidate = &result.candidates[0];
    let added = candidate.arrangement.added_part;
    let mut by_attack = BTreeMap::<u64, Vec<u8>>::new();
    let mut bass_notes = std::collections::BTreeSet::new();
    for event in acorde_core::to_playback_events(&candidate.arrangement.score, &Default::default())
    {
        if event
            .source
            .as_ref()
            .is_some_and(|address| address.part == added)
        {
            by_attack
                .entry((event.time_beats * 960.0).round() as u64)
                .or_default()
                .push(event.pitch_midi);
        } else if event
            .source
            .as_ref()
            .is_some_and(|address| address.part == 1)
        {
            bass_notes.insert(event.pitch_midi);
        }
    }
    assert_eq!(by_attack.len(), 8);
    let profile = StandardBassProfile {
        bass_buttons: bass_notes
            .into_iter()
            .enumerate()
            .map(|(i, midi)| StandardBassNoteButton {
                id: format!("bass-{i}"),
                position: 0,
                midi,
            })
            .collect(),
        chord_buttons: by_attack
            .into_iter()
            .enumerate()
            .map(|(i, (_, mut midi))| {
                midi.sort_unstable();
                StandardBassChordButton {
                    id: format!("chord-{i}"),
                    position: 0,
                    midi,
                }
            })
            .collect(),
        max_distance: 0,
    };
    let report =
        screen_harmony_groove_standard_bass(&source, &candidate.resolved, &profile).unwrap();
    assert!(
        report.playable_with_supplied_profile,
        "{:?}",
        report.diagnostics
    );
    assert_eq!(report.button_states.len(), 8);
    assert!(
        report
            .button_states
            .iter()
            .all(|state| state.chord_button_id.is_some()
                && state.bass_button_id.is_some()
                && state.new_attack)
    );
}

#[test]
fn a_saved_candidate_rejects_a_modified_source_and_invalid_profile() {
    let (source, request) = longer_fixture(8);
    let candidate = suggest_harmony_groove_arrangements(&source, &request)
        .unwrap()
        .candidates
        .remove(0);
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
    assert!(
        screen_harmony_groove_standard_bass(&changed, &candidate.resolved, &profile()).is_err()
    );
    let mut invalid = profile();
    invalid.chord_buttons[0].id = invalid.bass_buttons[0].id.clone();
    assert!(screen_harmony_groove_standard_bass(&source, &candidate.resolved, &invalid).is_err());
}
