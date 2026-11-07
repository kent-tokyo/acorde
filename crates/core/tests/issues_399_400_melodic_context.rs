use acorde_core::{
    BeatFraction, ChordPlan, ChordPlanEntry, ChordPlanOrigin, ChordPlanPosition, ChordSymbol,
    Duration, KeySignature, MelodicInsertionContext, MelodicInsertionKind, MelodicInsertionOptions,
    MelodicInsertionSelection, MelodicInsertionSource, MelodicLocalKey, Note, NoteAddr, Pitch,
    Score, Step, insert_melodic_notes, insert_melodic_notes_with_context, to_playback_events,
};

fn note(step: Step, duration: Duration) -> Note {
    Note::new(Pitch::new(step, 5), duration)
}

fn options(measures: usize) -> MelodicInsertionOptions {
    MelodicInsertionOptions {
        part: 0,
        staff: 0,
        voice: 0,
        start_measure: 0,
        end_measure: measures,
        key: KeySignature::default(),
        kind: MelodicInsertionKind::Passing,
        source: MelodicInsertionSource::ShortenNote,
        max_insertions: 4,
        min_midi: 48,
        max_midi: 96,
        max_leap_semitones: 3,
    }
}

fn chord(measure: usize, numerator: u32, denominator: u32, root: &str) -> ChordPlanEntry {
    ChordPlanEntry {
        position: ChordPlanPosition {
            measure_index: measure,
            offset: BeatFraction {
                numerator,
                denominator,
            },
        },
        offset_beats: f64::from(numerator) / f64::from(denominator),
        chord: serde_json::from_value::<ChordSymbol>(serde_json::json!({
            "root": root, "kind": "major", "bass": null
        }))
        .unwrap(),
        origin: ChordPlanOrigin::HostAuthored,
        sources: Vec::new(),
    }
}

#[test]
fn chord_change_at_insertion_beat_selects_chord_tone_and_replays_deterministically() {
    let mut score = Score::new("context", 120, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
    ];
    let before = serde_json::to_value(&score).unwrap();
    let context = MelodicInsertionContext {
        chord_plan: Some(ChordPlan {
            contract_version: 1,
            entries: vec![chord(0, 0, 1, "C"), chord(0, 3, 4, "B")],
        }),
        ..Default::default()
    };
    let first = insert_melodic_notes_with_context(&score, options(1), &context);
    assert_eq!(first.outcome, acorde_core::MelodicInsertionOutcome::Applied);
    assert_eq!(
        first.insertion.insertions.len(),
        1,
        "{:?}",
        first.insertion.diagnostics
    );
    assert_eq!(first.insertion.insertions[0].pitch.to_midi(), 75); // D#5, a B-major chord tone
    assert!(
        first
            .candidates
            .iter()
            .any(|c| c.selected && c.reason_codes.contains(&"chord-tone".into()))
    );
    assert!(
        first
            .candidates
            .iter()
            .any(|c| c.pitch.to_midi() == 74 && !c.selected)
    );
    let output = first.insertion.score.as_ref().unwrap();
    assert_eq!(
        output.parts[0].staves[0].measures[0].voices[0][2].id,
        score.parts[0].staves[0].measures[0].voices[0][1].id
    );
    assert_eq!(
        to_playback_events(output, &Default::default())
            .iter()
            .map(|event| event.time_beats)
            .collect::<Vec<_>>(),
        vec![0.0, 0.75, 1.0]
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    let second = insert_melodic_notes_with_context(&score, options(1), &context);
    assert_eq!(
        serde_json::to_value(first.insertion.score).unwrap(),
        serde_json::to_value(second.insertion.score).unwrap()
    );
}

#[test]
fn weak_passing_note_remains_available_and_host_can_select_it() {
    let mut score = Score::new("weak", 120, 3, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
        note(Step::G, Duration::Quarter),
    ];
    let mut context = MelodicInsertionContext {
        chord_plan: Some(ChordPlan {
            contract_version: 1,
            entries: vec![chord(0, 0, 1, "C")],
        }),
        ..Default::default()
    };
    let result = insert_melodic_notes_with_context(&score, options(1), &context);
    assert!(result.candidates.iter().any(|c| {
        c.selected
            && c.pitch.to_midi() == 74
            && c.reason_codes
                .contains(&"resolved-weak-beat-nonchord-tone".into())
    }));
    context.selections.push(MelodicInsertionSelection {
        source_anchor: NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        },
        pitch: Pitch::new(Step::D, 5),
    });
    let chosen = insert_melodic_notes_with_context(&score, options(1), &context);
    assert_eq!(chosen.insertion.insertions[0].pitch.to_midi(), 74);
}

#[test]
fn explicit_local_key_change_allows_second_bar_without_changing_written_key() {
    let mut score = Score::new("keys", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    score.parts[0].staves[0].measures[1].key_sig = Some(KeySignature {
        fifths: 1,
        mode: "major".into(),
    });
    score.parts[0].staves[0].measures[1].voices[0] =
        vec![note(Step::G, Duration::Half), note(Step::B, Duration::Half)];
    let context = MelodicInsertionContext {
        local_keys: vec![MelodicLocalKey {
            start_measure: 1,
            key: KeySignature {
                fifths: 1,
                mode: "major".into(),
            },
        }],
        ..Default::default()
    };
    let result = insert_melodic_notes_with_context(&score, options(2), &context);
    assert_eq!(
        result.insertion.insertions.len(),
        2,
        "{:?}",
        result.insertion.diagnostics
    );
    assert_eq!(result.insertion.insertions[1].pitch.to_midi(), 81); // A5
    assert_eq!(
        result.insertion.score.unwrap().parts[0].staves[0].measures[1].key_sig,
        score.parts[0].staves[0].measures[1].key_sig
    );
}

#[test]
fn range_and_leap_failures_are_visible_and_leave_score_unchanged() {
    let mut score = Score::new("limits", 120, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
    ];
    let before = serde_json::to_value(&score).unwrap();
    let context = MelodicInsertionContext {
        chord_plan: Some(ChordPlan {
            contract_version: 1,
            entries: vec![chord(0, 0, 1, "C")],
        }),
        ..Default::default()
    };
    let mut request = options(1);
    request.max_midi = 72;
    let result = insert_melodic_notes_with_context(&score, request, &context);
    assert_eq!(
        result.outcome,
        acorde_core::MelodicInsertionOutcome::NoChange
    );
    assert!(result.insertion.insertions.is_empty());
    assert!(
        result
            .candidates
            .iter()
            .all(|candidate| !candidate.selected)
    );
    assert!(result.candidates.iter().all(|candidate| {
        candidate
            .constraint_violations
            .contains(&"pitch-range".into())
    }));
    assert_eq!(
        serde_json::to_value(result.insertion.score).unwrap(),
        before
    );
}

#[test]
fn phrase_ending_nonchord_tone_is_diagnosed_and_skipped() {
    let mut score = Score::new("ending", 120, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
    ];
    let context = MelodicInsertionContext {
        chord_plan: Some(ChordPlan {
            contract_version: 1,
            entries: vec![chord(0, 0, 1, "C")],
        }),
        phrase_end_measures: vec![0],
        ..Default::default()
    };
    let result = insert_melodic_notes_with_context(&score, options(1), &context);
    assert!(result.insertion.insertions.is_empty());
    assert!(result.candidates.iter().any(|candidate| {
        candidate.pitch.to_midi() == 74
            && candidate
                .reason_codes
                .contains(&"phrase-ending-nonchord-tone".into())
            && !candidate.selected
    }));
    assert!(
        result
            .insertion
            .diagnostics
            .iter()
            .any(|item| item.code == "no-safe-candidate")
    );
}

#[test]
fn weak_dissonance_without_chordal_resolution_is_skipped() {
    let mut score = Score::new("unresolved", 120, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
    ];
    let context = MelodicInsertionContext {
        chord_plan: Some(ChordPlan {
            contract_version: 1,
            entries: vec![chord(0, 0, 1, "F")],
        }),
        ..Default::default()
    };
    let result = insert_melodic_notes_with_context(&score, options(1), &context);
    assert!(result.insertion.insertions.is_empty());
    assert!(result.candidates.iter().any(|candidate| {
        candidate.pitch.to_midi() == 74
            && candidate
                .reason_codes
                .contains(&"unresolved-or-strong-beat-dissonance".into())
    }));
}

#[test]
fn empty_context_matches_legacy_and_unsplittable_notes_are_not_selected() {
    let mut score = Score::new("legacy", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    let legacy = insert_melodic_notes(&score, options(1));
    let contextual = insert_melodic_notes_with_context(&score, options(1), &Default::default());
    assert_eq!(
        serde_json::to_value(legacy).unwrap(),
        serde_json::to_value(contextual.insertion).unwrap()
    );
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Eighth),
        note(Step::E, Duration::Eighth),
        note(Step::G, Duration::Half),
        Note::rest(Duration::Quarter),
    ];
    let result = insert_melodic_notes_with_context(&score, options(1), &Default::default());
    assert!(result.insertion.insertions.is_empty());
    assert!(!result.candidates.iter().any(|candidate| candidate.selected));
}

#[test]
fn per_measure_density_limit_is_respected() {
    let mut score = Score::new("density", 120, 3, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        note(Step::E, Duration::Quarter),
        note(Step::G, Duration::Quarter),
    ];
    let context = MelodicInsertionContext {
        max_insertions_per_measure: Some(1),
        ..Default::default()
    };
    let result = insert_melodic_notes_with_context(&score, options(1), &context);
    assert_eq!(result.insertion.insertions.len(), 1);
    assert_eq!(result.insertion.insertions[0].source_anchor.note, 0);
}
