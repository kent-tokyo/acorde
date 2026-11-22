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

#[test]
fn explicit_transposed_staves_match_sounding_buttons_and_repeat_visits() {
    use acorde_core::{
        Barline, HarmonyStaffRef, replay_harmony_groove_arrangement, screen_standard_bass_staves,
        transpose_checked,
    };
    let mut source = fixture::source();
    for part in &mut source.parts {
        part.staves[0].measures[0].barline_left = Barline::RepeatStart;
        part.staves[0].measures[2].barline_right = Barline::RepeatEnd;
    }
    let candidate = suggest_harmony_groove_arrangements(&source, &fixture::request())
        .unwrap()
        .candidates
        .remove(0);
    let replay = replay_harmony_groove_arrangement(&source, &candidate.resolved).unwrap();
    let staves = [
        HarmonyStaffRef { part: 1, staff: 0 },
        HarmonyStaffRef {
            part: replay.arrangement.added_part,
            staff: 0,
        },
    ];
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
    assert!(replay_harmony_groove_arrangement(&changed, &candidate.resolved).is_err());
    for semitones in [-2, 2] {
        let score = transpose_checked(&replay.arrangement.score, semitones).unwrap();
        let all = acorde_core::to_playback_events(&score, &Default::default());
        let mut bass = std::collections::BTreeSet::new();
        let mut attacks = BTreeMap::<u64, Vec<u8>>::new();
        for event in &all {
            if let Some(addr) = &event.source {
                if addr.part == 1 {
                    bass.insert(event.pitch_midi);
                }
                if addr.part == staves[1].part {
                    attacks
                        .entry((event.time_beats * 960.0).round() as u64)
                        .or_default()
                        .push(event.pitch_midi);
                }
            }
        }
        let instrument = StandardBassProfile {
            bass_buttons: bass
                .into_iter()
                .enumerate()
                .map(|(i, midi)| StandardBassNoteButton {
                    id: format!("bass-{i}"),
                    position: 0,
                    midi,
                })
                .collect(),
            chord_buttons: attacks
                .into_values()
                .enumerate()
                .map(|(i, mut midi)| {
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
        let report = screen_standard_bass_staves(&score, &staves, &instrument).unwrap();
        assert!(
            report.playable_with_supplied_profile,
            "{:?}",
            report.diagnostics
        );
        assert!(report.requires_performer_review);
        assert!(report.button_states.len() >= 6);
        for state in &report.button_states {
            let mut expected = all
                .iter()
                .filter(|e| {
                    e.source.as_ref().is_some_and(|a| {
                        staves
                            .iter()
                            .any(|s| s.part == a.part && s.staff == a.staff)
                    }) && e.time_beats <= state.time_beats + 1e-7
                        && e.time_beats + e.duration_beats > state.time_beats + 1e-7
                })
                .map(|e| e.pitch_midi)
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(state.sounding_midi, expected);
            assert_eq!(state.button_midi, expected);
        }
    }
}

#[test]
fn explicit_staff_screen_rejects_bad_selections_and_controls() {
    use acorde_core::{
        HarmonyGrooveStandardBassError, HarmonyStaffRef, screen_standard_bass_staves,
    };
    let source = fixture::source();
    let staff = HarmonyStaffRef { part: 1, staff: 0 };
    for selection in [
        vec![],
        vec![staff, staff],
        vec![HarmonyStaffRef { part: 42, staff: 0 }],
    ] {
        assert!(matches!(
            screen_standard_bass_staves(&source, &selection, &profile()),
            Err(HarmonyGrooveStandardBassError::InvalidStaffSelection)
        ));
    }
    let mut invalid = profile();
    invalid.chord_buttons[0].id = invalid.bass_buttons[0].id.clone();
    assert!(matches!(
        screen_standard_bass_staves(&source, &[staff], &invalid),
        Err(HarmonyGrooveStandardBassError::Profile(_))
    ));
    let mut altered = source.clone();
    altered.parts[1].staves[0].measures[0].voices[0][0].pitches[0].microtone_cents = 25;
    let report = screen_standard_bass_staves(&altered, &[staff], &profile()).unwrap();
    assert!(!report.playable_with_supplied_profile);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.reason == HarmonyGrooveButtonFailure::UnsupportedMicrotoneOrBend)
    );
    let mut bend = source.clone();
    bend.parts[1].staves[0].measures[0].voices[0][0].guitar_bend_alter_cents = Some(50);
    assert!(
        matches!(screen_standard_bass_staves(&bend, &[staff], &profile()), Err(HarmonyGrooveStandardBassError::UnsupportedScoreEffect { address }) if address.part == 1 && address.measure == 0 && address.note == 0)
    );
    let mut pedal = source;
    pedal.parts[1].staves[0].measures[0].voices[0][0].pedal_start = true;
    assert!(matches!(
        screen_standard_bass_staves(&pedal, &[staff], &profile()),
        Err(HarmonyGrooveStandardBassError::UnsupportedPartControls { part: 1 })
    ));
}
