use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::{
    StandardBassArrangementOptions, arrange_standard_bass,
};
use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, BeatFraction, ChordPlan, ChordPlanEntry,
    ChordPlanOrigin, ChordPlanPosition, ChordSymbol, Duration, Note, Pitch,
    PositionedAccordionChord, Score, Step, arrange_accordion_preset,
    chord_plan_to_preset_selection, to_playback_events,
};

fn chord(root: &str, kind: &str) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root":root,"kind":kind})).unwrap()
}
fn source() -> Score {
    let mut score = Score::new("melody", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    score
}
fn selection(chord: ChordSymbol) -> AccordionPresetSelection {
    AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord,
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    }
}
fn plan(chord: ChordSymbol) -> ChordPlan {
    ChordPlan {
        contract_version: 1,
        entries: vec![ChordPlanEntry {
            position: ChordPlanPosition {
                measure_index: 0,
                offset: BeatFraction::ZERO,
            },
            offset_beats: 0.0,
            chord,
            origin: ChordPlanOrigin::HostAuthored,
            sources: vec![],
        }],
    }
}

#[test]
fn default_standard_preset_and_chord_plan_distinguish_real_button_kinds() {
    let score = source();
    let before = serde_json::to_value(&score).unwrap();
    for (root, bass, chord_notes) in [
        ("G", 43, vec![67, 71, 74, 77]),
        ("D", 38, vec![62, 66, 69, 72]),
    ] {
        let symbol = chord(root, "dominant");
        let selected =
            chord_plan_to_preset_selection(&plan(symbol.clone()), AccordionBassMode::StandardBass);
        assert!(
            selected.diagnostics.is_empty(),
            "{:?}",
            selected.diagnostics
        );
        let result = arrange_accordion_preset(&score, selection(symbol), "bass-pulse");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        let notes = &output.parts[0].staves[1].measures[0].voices[0];
        assert_eq!(
            notes[0]
                .pitches
                .iter()
                .map(|p| p.to_midi())
                .collect::<Vec<_>>(),
            [bass]
        );
        assert_eq!(
            notes[1]
                .pitches
                .iter()
                .map(|p| p.to_midi())
                .collect::<Vec<_>>(),
            chord_notes
        );
        assert_eq!(notes[0].chord_symbol.as_ref().unwrap().kind, "dominant");
        let events = to_playback_events(&output, &Default::default());
        assert!(
            events
                .iter()
                .any(|e| e.pitch_midi == bass as u8 && e.time_beats == 0.0)
        );
        for midi in &chord_notes {
            assert!(
                events
                    .iter()
                    .any(|e| e.pitch_midi == *midi as u8 && e.time_beats == 1.0)
            );
        }
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }
    for kind in ["major", "minor"] {
        assert!(
            arrange_accordion_preset(&score, selection(chord("C", kind)), "bass-pulse")
                .score
                .is_some()
        );
    }
    for kind in ["augmented", "diminished", "major-seventh", "minor-seventh"] {
        let result = arrange_accordion_preset(&score, selection(chord("C", kind)), "bass-pulse");
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "unsupported-standard-bass");
        assert!(
            chord_plan_to_preset_selection(
                &plan(chord("C", kind)),
                AccordionBassMode::StandardBass
            )
            .selection
            .is_none()
        );
    }
}

#[test]
fn explicit_profile_distinguishes_fifth_omission_and_missing_seventh_button() {
    let score = source();
    for (root, bass, tones, fifth) in [
        ("G", 43, vec![67, 71, 77], 2),
        ("D", 38, vec![62, 66, 72], 9),
    ] {
        let profile = StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "bass".into(),
                position: 0,
                midi: bass,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "seventh".into(),
                position: 0,
                midi: tones.clone(),
            }],
            max_distance: 0,
        };
        let mut options = StandardBassArrangementOptions {
            harmony_source: Default::default(),
            melody_part: 0,
            melody_staff: 0,
            melody_voice: 0,
            allow_source_omissions: false,
            chords: vec![PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: chord(root, "dominant"),
            }],
            profile,
            allow_chord_omissions: true,
            allow_chord_additions: false,
        };
        let result = arrange_standard_bass(&score, &options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.actions[0].candidate.omitted_pitch_classes,
            vec![fifth]
        );
        assert_eq!(result.actions[0].candidate.chord_midi, tones);
        assert_eq!(
            result.score.unwrap().parts[0].staves[1].measures[0].voices[0][0]
                .chord_symbol
                .as_ref()
                .unwrap()
                .kind,
            "dominant"
        );
        options.profile.chord_buttons.clear();
        let missing = arrange_standard_bass(&score, &options);
        assert!(missing.score.is_none());
        assert_eq!(missing.diagnostics[0].code, "no-standard-bass-candidate");
    }
}

#[test]
fn profiled_pulse_uses_actual_separate_buttons_and_reports_missing_fifth() {
    use acorde_core::model::standard_bass_preset::{
        ProfiledBassPulseActionKind, ProfiledBassPulseOptions, arrange_profiled_bass_pulse,
    };
    let score = source();
    let before = serde_json::to_value(&score).unwrap();
    for (root, bass, tones, fifth) in [
        ("G", 43, vec![67, 71, 77], 2),
        ("D", 38, vec![62, 66, 72], 9),
    ] {
        let mut options = ProfiledBassPulseOptions {
            selection: selection(chord(root, "dominant")),
            profile: StandardBassProfile {
                bass_buttons: vec![StandardBassNoteButton {
                    id: "bass".into(),
                    position: 0,
                    midi: bass,
                }],
                chord_buttons: vec![StandardBassChordButton {
                    id: "seventh".into(),
                    position: 0,
                    midi: tones.clone(),
                }],
                max_distance: 0,
            },
            allow_chord_omissions: true,
            allow_chord_additions: false,
        };
        let result = arrange_profiled_bass_pulse(&score, &options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.actions.len(), 4);
        assert_eq!(
            result.actions.iter().map(|a| a.kind).collect::<Vec<_>>(),
            [
                ProfiledBassPulseActionKind::Bass,
                ProfiledBassPulseActionKind::Chord,
                ProfiledBassPulseActionKind::Bass,
                ProfiledBassPulseActionKind::Chord
            ]
        );
        assert_eq!(result.actions[0].sounding_midi, vec![bass]);
        assert_eq!(result.actions[1].sounding_midi, tones);
        assert_eq!(
            result.actions[1].candidate.omitted_pitch_classes,
            vec![fifth]
        );
        let output = result.score.unwrap();
        let events = to_playback_events(&output, &Default::default());
        let at = |beat| {
            events
                .iter()
                .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1) && e.time_beats == beat)
                .map(|e| e.pitch_midi)
                .collect::<Vec<_>>()
        };
        assert_eq!(at(0.0), vec![bass]);
        assert_eq!(at(1.0), tones);
        assert!(
            events
                .iter()
                .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1))
                .all(|e| e.duration_beats == 1.0)
        );
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
        let original_profile = options.profile.clone();
        options.profile.chord_buttons.clear();
        let missing = arrange_profiled_bass_pulse(&score, &options);
        assert_eq!(missing.diagnostics[0].code, "no-standard-bass-candidate");
        assert!(missing.score.is_none() && missing.actions.is_empty());
        options.profile = original_profile;
        options.profile.bass_buttons.clear();
        let missing_root = arrange_profiled_bass_pulse(&score, &options);
        assert_eq!(
            missing_root.diagnostics[0].code,
            "no-standard-bass-candidate"
        );
        assert!(missing_root.score.is_none() && missing_root.actions.is_empty());
    }
}

#[test]
fn profiled_pulse_does_not_claim_bass_and_chord_sound_simultaneously() {
    use acorde_core::model::standard_bass_preset::{
        ProfiledBassPulseOptions, arrange_profiled_bass_pulse,
    };
    let options = ProfiledBassPulseOptions {
        selection: selection(chord("C", "major-seventh")),
        profile: StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "C bass".into(),
                position: 0,
                midi: 36,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "Em chord".into(),
                position: 0,
                midi: vec![64, 67, 71],
            }],
            max_distance: 0,
        },
        allow_chord_omissions: false,
        allow_chord_additions: false,
    };
    let result = arrange_profiled_bass_pulse(&source(), &options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.actions[0].sounding_midi, vec![36]);
    assert_eq!(result.actions[1].sounding_midi, vec![64, 67, 71]);
    let events = to_playback_events(result.score.as_ref().unwrap(), &Default::default());
    let at_one: Vec<_> = events
        .iter()
        .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1) && e.time_beats == 1.0)
        .map(|e| e.pitch_midi)
        .collect();
    assert_eq!(at_one, vec![64, 67, 71]);
    assert!(!at_one.contains(&36));
}

#[test]
fn profiled_pulse_rejects_free_bass_or_unapproved_omissions() {
    use acorde_core::model::standard_bass_preset::{
        ProfiledBassPulseOptions, arrange_profiled_bass_pulse,
    };
    let mut options = ProfiledBassPulseOptions {
        selection: selection(chord("G", "dominant")),
        profile: StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "bass".into(),
                position: 0,
                midi: 43,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "seventh".into(),
                position: 0,
                midi: vec![67, 71, 77],
            }],
            max_distance: 0,
        },
        allow_chord_omissions: false,
        allow_chord_additions: false,
    };
    let denied = arrange_profiled_bass_pulse(&source(), &options);
    assert!(denied.score.is_none());
    assert_eq!(denied.diagnostics[0].code, "no-standard-bass-candidate");
    options.selection.bass_mode = AccordionBassMode::FreeBass;
    let wrong_mode = arrange_profiled_bass_pulse(&source(), &options);
    assert!(wrong_mode.score.is_none());
    assert_eq!(wrong_mode.diagnostics[0].code, "invalid-bass-mode");
}

#[test]
fn profiled_chord_plan_uses_actual_buttons_and_rejects_missing_ones() {
    use acorde_core::model::profiled_chord_plan::profile_chord_plan;
    use acorde_core::model::standard_bass_preset::arrange_profiled_bass_pulse;
    let score = source();
    for (root, kind, bass, tones) in [
        ("C", "major-seventh", 36, vec![64, 67, 71]),
        ("D", "minor-seventh", 38, vec![65, 69, 72]),
        ("A", "minor-seventh", 45, vec![60, 64, 67]),
    ] {
        let profile = StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "bass".into(),
                position: 0,
                midi: bass,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "chord".into(),
                position: 0,
                midi: tones.clone(),
            }],
            max_distance: 0,
        };
        let plan = plan(chord(root, kind));
        let result = profile_chord_plan(&plan, &profile, false, false);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.requires_performer_review);
        let arranged = arrange_profiled_bass_pulse(&score, &result.options.unwrap());
        assert!(
            arranged.diagnostics.is_empty(),
            "{:?}",
            arranged.diagnostics
        );
        assert_eq!(arranged.actions[0].sounding_midi, [bass]);
        assert_eq!(arranged.actions[1].sounding_midi, tones);
        assert_eq!(
            arranged.actions[0].candidate.omitted_pitch_classes,
            Vec::<u8>::new()
        );
        let mut missing = profile.clone();
        missing.chord_buttons.clear();
        let result = profile_chord_plan(&plan, &missing, false, false);
        assert!(result.options.is_none());
        assert_eq!(result.diagnostics[0].code, "no-standard-bass-candidate");
    }
}

#[test]
fn profiled_chord_plan_validates_policy_positions_and_bounds() {
    use acorde_core::model::profiled_chord_plan::profile_chord_plan;
    let profile = StandardBassProfile {
        bass_buttons: vec![StandardBassNoteButton {
            id: "G".into(),
            position: 0,
            midi: 43,
        }],
        chord_buttons: vec![StandardBassChordButton {
            id: "G7".into(),
            position: 0,
            midi: vec![67, 71, 77],
        }],
        max_distance: 0,
    };
    let mut plan = plan(chord("G", "dominant"));
    assert!(
        profile_chord_plan(&plan, &profile, false, false)
            .options
            .is_none()
    );
    assert!(
        profile_chord_plan(&plan, &profile, true, false)
            .options
            .is_some()
    );
    plan.entries[0].position.offset.denominator = 0;
    let invalid = profile_chord_plan(&plan, &profile, true, false);
    assert!(invalid.options.is_none());
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|d| d.code == "invalid-position")
    );
    plan.entries = vec![plan.entries[0].clone(); 513];
    let excessive = profile_chord_plan(&plan, &profile, true, false);
    assert!(excessive.options.is_none());
    assert_eq!(excessive.diagnostics[0].code, "plan-entry-limit");
}
