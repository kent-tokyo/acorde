use acorde_core::{
    Duration, KeySignature, MelodicInsertionKind, MelodicInsertionOptions, MelodicInsertionSource,
    Note, Pitch, Score, Step, insert_melodic_notes, to_playback_events,
};
fn options(kind: MelodicInsertionKind, source: MelodicInsertionSource) -> MelodicInsertionOptions {
    MelodicInsertionOptions {
        part: 0,
        staff: 0,
        voice: 0,
        start_measure: 0,
        end_measure: 1,
        key: KeySignature::default(),
        kind,
        source,
        max_insertions: 1,
        min_midi: 48,
        max_midi: 96,
        max_leap_semitones: 2,
    }
}
fn note(step: Step, duration: Duration) -> Note {
    Note::new(Pitch::new(step, 5), duration)
}
#[test]
fn passing_note_preserves_source_onsets_and_returns_stable_provenance() {
    let mut score = Score::new("passing", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    let before = serde_json::to_value(&score).unwrap();
    let opts = options(
        MelodicInsertionKind::Passing,
        MelodicInsertionSource::ShortenNote,
    );
    let result = insert_melodic_notes(&score, opts.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(result.insertions.len(), 1);
    assert_eq!(result.insertions[0].source_anchor.note, 0);
    assert_eq!(result.insertions[0].inserted.note, 1);
    assert_eq!(result.source_mapping[1].source.note, 1);
    assert_eq!(result.source_mapping[1].output.note, 2);
    let output = result.score.unwrap();
    let notes = &output.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(
        notes.iter().map(Note::beats).collect::<Vec<_>>(),
        vec![1.5, 0.5, 2.0]
    );
    assert_eq!(
        notes
            .iter()
            .map(|n| n.pitches[0].to_midi())
            .collect::<Vec<_>>(),
        vec![72, 74, 76]
    );
    assert_eq!(
        notes[0].id,
        score.parts[0].staves[0].measures[0].voices[0][0].id
    );
    assert_eq!(
        notes[2].id,
        score.parts[0].staves[0].measures[0].voices[0][1].id
    );
    let events = to_playback_events(&output, &Default::default());
    assert_eq!(
        events.iter().map(|e| e.time_beats).collect::<Vec<_>>(),
        vec![0.0, 1.5, 2.0]
    );
    let again = insert_melodic_notes(&score, opts).score.unwrap();
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        serde_json::to_value(again).unwrap()
    );
}
#[test]
fn neighbor_can_fill_a_rest_without_moving_the_following_note() {
    let mut score = Score::new("rest", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        note(Step::C, Duration::Quarter),
        Note::rest(Duration::Half),
        note(Step::C, Duration::Quarter),
    ];
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::UpperNeighbor,
            MelodicInsertionSource::FillRest,
        ),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let ns = &output.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(
        ns.iter().map(Note::beats).collect::<Vec<_>>(),
        vec![1.0, 1.5, 0.5, 1.0]
    );
    assert!(ns[1].is_rest);
    assert_eq!(ns[2].pitches[0].to_midi(), 74);
    assert_eq!(
        ns[3].id,
        score.parts[0].staves[0].measures[0].voices[0][2].id
    );
    let events = to_playback_events(&output, &Default::default());
    assert_eq!(
        events.iter().map(|e| e.time_beats).collect::<Vec<_>>(),
        vec![0.0, 2.5, 3.0]
    );
}
#[test]
fn ineligible_ties_tuplets_keys_and_references_are_not_silently_changed() {
    let mut score = Score::new("safety", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    let opts = options(
        MelodicInsertionKind::Passing,
        MelodicInsertionSource::ShortenNote,
    );
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[0].voices[0][1].tie_end = true;
    let before = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(&score, opts.clone());
    assert!(result.insertions.is_empty());
    assert_eq!(result.diagnostics[0].code, "ineligible-candidate");
    assert_eq!(serde_json::to_value(result.score.unwrap()).unwrap(), before);
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = false;
    score.parts[0].staves[0].measures[0].voices[0][1].tie_end = false;
    let mut wrong = opts;
    wrong.key.fifths = 1;
    let result = insert_melodic_notes(&score, wrong);
    assert!(result.insertions.is_empty());
    assert_eq!(result.diagnostics[0].code, "unsupported-measure");
}

#[test]
fn explicit_key_lower_neighbor_density_and_pitch_limits_are_enforced() {
    let mut score = Score::new("neighbors", 120, 4, 4, 1, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::G, Duration::Half), note(Step::G, Duration::Half)];
    let mut opts = options(
        MelodicInsertionKind::LowerNeighbor,
        MelodicInsertionSource::ShortenNote,
    );
    opts.key.fifths = 1;
    let result = insert_melodic_notes(&score, opts.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let inserted = &result.score.unwrap().parts[0].staves[0].measures[0].voices[0][1].pitches[0];
    assert_eq!(inserted.step, Step::F);
    assert_eq!(inserted.alter, 1);
    opts.max_insertions = 0;
    let result = insert_melodic_notes(&score, opts.clone());
    assert!(result.insertions.is_empty());
    opts.max_insertions = 1;
    opts.min_midi = 80;
    let result = insert_melodic_notes(&score, opts);
    assert!(result.insertions.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "ineligible-candidate")
    );
}

#[test]
fn harmony_range_end_is_remapped_to_the_original_note() {
    let mut score = Score::new("range", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    let mut harmony: acorde_core::ChordSymbol =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major","bass":null})).unwrap();
    harmony.range_end = Some(acorde_core::NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note: 1,
    });
    score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol = Some(harmony);
    let before = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert_eq!(result.insertions.len(), 1);
    let output = result.score.unwrap();
    let range_end = output.parts[0].staves[0].measures[0].voices[0][0]
        .chord_symbol
        .as_ref()
        .unwrap()
        .range_end
        .as_ref()
        .unwrap();
    assert_eq!(range_end.note, 2);
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
}

#[test]
fn typed_spanners_styles_and_linked_views_keep_original_targets() {
    let mut score = Score::new("references", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    score.parts[0].staves[0].measures[1].voices[0] =
        vec![note(Step::F, Duration::Half), note(Step::A, Duration::Half)];
    let address = |measure, note| acorde_core::NoteAddr {
        part: 0,
        staff: 0,
        measure,
        voice: 0,
        note,
    };
    score.spanners.push(acorde_core::NotationSpanner {
        id: "slur".into(),
        kind: acorde_core::NotationSpannerKind::Slur,
        start: address(0, 0),
        end: address(0, 1),
        number: None,
        line_type: None,
        text: None,
        placement: None,
        ottava_size: None,
        ottava_type: None,
    });
    score.spanners.push(acorde_core::NotationSpanner {
        id: "outside".into(),
        kind: acorde_core::NotationSpannerKind::Slur,
        start: address(1, 0),
        end: address(1, 1),
        number: None,
        line_type: None,
        text: None,
        placement: None,
        ottava_size: None,
        ottava_type: None,
    });
    score
        .object_style_overrides
        .push(acorde_core::ObjectStyleOverride {
            target: acorde_core::ObjectStyleTarget::Note {
                address: address(0, 1),
            },
            property: acorde_core::ViewStyleProperty::TextScale,
            value: 1.1,
            provenance: None,
        });
    score
        .views
        .push(acorde_core::ScoreView::linked_part("part", "Part", 0));
    let before = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert_eq!(result.insertions.len(), 1, "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(output.spanners[0].start, address(0, 0));
    assert_eq!(output.spanners[0].end, address(0, 2));
    assert_eq!(output.spanners[1].start, address(1, 0));
    assert_eq!(output.spanners[1].end, address(1, 1));
    assert_eq!(
        output.object_style_overrides[0].target,
        acorde_core::ObjectStyleTarget::Note {
            address: address(0, 2)
        }
    );
    assert_eq!(output.views.len(), 1);
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
}

#[test]
fn authored_beams_slur_and_chord_on_other_notes_survive_insertion() {
    let mut score = Score::new("marks", 120, 4, 4, 0, 1);
    let mut beamed = [Step::G, Step::A, Step::B, Step::G]
        .into_iter()
        .map(|step| note(step, Duration::Eighth))
        .collect::<Vec<_>>();
    beamed[0].beam = acorde_core::BeamState::Begin;
    beamed[1].beam = acorde_core::BeamState::Continue;
    beamed[2].beam = acorde_core::BeamState::Continue;
    beamed[3].beam = acorde_core::BeamState::End;
    beamed[0].slur_start = true;
    let mut ending = note(Step::E, Duration::Quarter);
    ending.slur_end = true;
    ending
        .articulations
        .push(acorde_core::Articulation::Staccato);
    ending.chord_symbol = Some(
        serde_json::from_value(serde_json::json!({
            "root":"C", "kind":"major", "bass":null
        }))
        .unwrap(),
    );
    beamed.push(note(Step::C, Duration::Quarter));
    beamed.push(ending);
    score.parts[0].staves[0].measures[0].voices[0] = beamed;
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert_eq!(result.insertions.len(), 1, "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let notes = &output.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(notes.len(), 7);
    assert_eq!(notes[0].beam, acorde_core::BeamState::Begin);
    assert_eq!(notes[3].beam, acorde_core::BeamState::End);
    assert!(notes[0].slur_start);
    assert!(notes[6].slur_end);
    assert_eq!(
        notes[6].articulations,
        vec![acorde_core::Articulation::Staccato]
    );
    assert_eq!(notes[6].chord_symbol.as_ref().unwrap().root, "C");
}

#[test]
fn unsafe_bar_does_not_cancel_safe_bar() {
    let mut score = Score::new("partial", 120, 4, 4, 0, 2);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    }
    score.parts[0].staves[0].measures[0].voices[1] = vec![note(Step::G, Duration::Whole)];
    let original_first = serde_json::to_value(&score.parts[0].staves[0].measures[0]).unwrap();
    let mut opts = options(
        MelodicInsertionKind::Passing,
        MelodicInsertionSource::ShortenNote,
    );
    opts.end_measure = 2;
    let result = insert_melodic_notes(&score, opts);
    assert_eq!(result.insertions.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.outcome(),
        acorde_core::MelodicInsertionOutcome::Partial
    );
    assert_eq!(result.insertions[0].source_anchor.measure, 1);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported-measure"
                && d.measure == Some(0)
                && d.message.contains("part 0 staff 0 measure 0 voice 0"))
    );
    assert_eq!(
        serde_json::to_value(&result.score.unwrap().parts[0].staves[0].measures[0]).unwrap(),
        original_first
    );
}

#[test]
fn fermata_timing_is_not_changed_by_shortening() {
    let mut score = Score::new("fermata", 120, 4, 4, 0, 1);
    let mut first = note(Step::C, Duration::Half);
    first.articulations.push(acorde_core::Articulation::Fermata);
    score.parts[0].staves[0].measures[0].voices[0] = vec![first, note(Step::E, Duration::Half)];
    let before = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert!(result.insertions.is_empty());
    assert_eq!(result.diagnostics[0].code, "ineligible-candidate");
    assert_eq!(serde_json::to_value(result.score.unwrap()).unwrap(), before);
}

#[test]
fn tuplets_and_polyphonic_measures_are_diagnosed_without_editing() {
    let mut score = Score::new("tuplet", 120, 4, 4, 0, 1);
    let mut triplets = Vec::new();
    for (index, step) in [Step::C, Step::D, Step::E].into_iter().enumerate() {
        let mut n = note(step, Duration::Eighth);
        n.tuplet = Some(acorde_core::TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        n.tuplet_boundary = match index {
            0 => Some(acorde_core::TupletBoundary::Start),
            2 => Some(acorde_core::TupletBoundary::Stop),
            _ => None,
        };
        triplets.push(n);
    }
    let mut tail = note(Step::G, Duration::Half);
    tail.dot_count = 1;
    triplets.push(tail);
    score.parts[0].staves[0].measures[0].voices[0] = triplets;
    let original = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert_eq!(result.diagnostics[0].code, "ineligible-candidate");
    assert_eq!(
        serde_json::to_value(result.score.unwrap()).unwrap(),
        original
    );
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    score.parts[0].staves[0].measures[0].voices[1] = vec![note(Step::G, Duration::Whole)];
    let original = serde_json::to_value(&score).unwrap();
    let result = insert_melodic_notes(
        &score,
        options(
            MelodicInsertionKind::Passing,
            MelodicInsertionSource::ShortenNote,
        ),
    );
    assert_eq!(result.diagnostics[0].code, "unsupported-measure");
    assert_eq!(
        serde_json::to_value(result.score.unwrap()).unwrap(),
        original
    );
}

#[test]
fn pickup_short_ending_and_repeat_keep_source_note_positions() {
    let mut score = Score::new("pickup", 120, 4, 4, 0, 3);
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].actual_length = acorde_core::MeasureLength::from_beats(1.0);
    bars[0].voices[0] = vec![note(Step::G, Duration::Quarter)];
    bars[0].barline_left = acorde_core::Barline::RepeatStart;
    bars[1].voices[0] = vec![note(Step::C, Duration::Half), note(Step::E, Duration::Half)];
    bars[1].barline_right = acorde_core::Barline::RepeatEnd;
    bars[2].actual_length = acorde_core::MeasureLength::from_beats(1.0);
    bars[2].voices[0] = vec![note(Step::C, Duration::Quarter)];
    let mut opts = options(
        MelodicInsertionKind::Passing,
        MelodicInsertionSource::ShortenNote,
    );
    opts.end_measure = 3;
    let result = insert_melodic_notes(&score, opts);
    assert_eq!(result.insertions.len(), 1);
    let output = result.score.unwrap();
    for mapping in result.source_mapping {
        let before = &score.parts[0].staves[0].measures[mapping.source.measure].voices[0];
        let after = &output.parts[0].staves[0].measures[mapping.output.measure].voices[0];
        assert_eq!(
            before[mapping.source.note].id,
            after[mapping.output.note].id
        );
        assert_eq!(
            before[..mapping.source.note]
                .iter()
                .map(Note::beats)
                .sum::<f64>(),
            after[..mapping.output.note]
                .iter()
                .map(Note::beats)
                .sum::<f64>()
        );
        assert_eq!(
            before[mapping.source.note].pitches,
            after[mapping.output.note].pitches
        );
    }
    assert_eq!(
        output.parts[0].staves[0].measures[0].actual_length,
        score.parts[0].staves[0].measures[0].actual_length
    );
    assert_eq!(
        output.parts[0].staves[0].measures[2].actual_length,
        score.parts[0].staves[0].measures[2].actual_length
    );
    assert_eq!(
        output.parts[0].staves[0].measures[1].barline_right,
        acorde_core::Barline::RepeatEnd
    );
}
