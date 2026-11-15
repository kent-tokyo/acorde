use acorde_core::{
    BeatFraction, ChordPlan, ChordPlanEntry, ChordPlanOrigin, ChordPlanPosition,
    ChordProposalRequest, ChordSymbol, Duration, Note, PhraseHarmonicPace, PhraseHarmonyRequest,
    PhraseHarmonySection, Pitch, Score, suggest_phrase_harmony,
};

fn score(numerator: u8, denominator: u8, bars: usize) -> Score {
    let mut score = Score::new(
        "phrase-harmony development",
        112,
        numerator,
        denominator,
        0,
        bars as u32,
    );
    let beat_unit = if denominator == 8 {
        Duration::Eighth
    } else {
        Duration::Quarter
    };
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = (0..numerator)
            .map(|beat| {
                Note::new(
                    Pitch::from_midi([72, 74, 76, 79][usize::from(beat) % 4], false),
                    beat_unit.clone(),
                )
            })
            .collect();
    }
    score
}

fn request(bars: usize, pace: PhraseHarmonicPace) -> PhraseHarmonyRequest {
    PhraseHarmonyRequest {
        phrases: vec![PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: bars,
            pace,
            meter_groups: vec![],
        }],
        proposal: ChordProposalRequest::default(),
        locked_chord_entries: vec![],
    }
}

#[test]
fn sections_vary_harmonic_rhythm_without_changing_melody() {
    let source = score(4, 4, 8);
    let mut input = request(8, PhraseHarmonicPace::EverySecondBar);
    input.phrases = vec![
        PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: 4,
            pace: PhraseHarmonicPace::EverySecondBar,
            meter_groups: vec![],
        },
        PhraseHarmonySection {
            start_measure: 4,
            end_measure_exclusive: 6,
            pace: PhraseHarmonicPace::EveryBar,
            meter_groups: vec![],
        },
        PhraseHarmonySection {
            start_measure: 6,
            end_measure_exclusive: 8,
            pace: PhraseHarmonicPace::TwoGroupsPerBar,
            meter_groups: vec![],
        },
    ];
    let before = serde_json::to_value(&source).unwrap();
    let result = suggest_phrase_harmony(&source, &input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let starts = result
        .positions
        .iter()
        .map(|p| (p.measure_index, p.offset.beats()))
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        [
            (0, 0.0),
            (2, 0.0),
            (4, 0.0),
            (5, 0.0),
            (6, 0.0),
            (6, 2.0),
            (7, 0.0),
            (7, 2.0)
        ]
    );
    assert!(!result.proposals.candidates.is_empty());
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(
        result.proposals.candidates[0].plan.entries[0].position,
        result.positions[0]
    );
}

#[test]
fn compound_and_irregular_groups_place_changes_on_real_group_boundaries() {
    for (numerator, denominator, expected) in [(6, 8, 1.5), (5, 8, 1.5), (7, 8, 2.0)] {
        let source = score(numerator, denominator, 2);
        let result =
            suggest_phrase_harmony(&source, &request(2, PhraseHarmonicPace::TwoGroupsPerBar));
        assert!(
            result.diagnostics.is_empty(),
            "{numerator}/8: {:?}",
            result.diagnostics
        );
        assert_eq!(result.positions.len(), 4);
        assert_eq!(result.positions[1].offset.beats(), expected);
        assert_eq!(result.positions[3].offset.beats(), expected);
    }
}

#[test]
fn original_chord_and_lock_survive_aligned_phrase_boundaries() {
    let source = score(4, 4, 4);
    let mut input = request(4, PhraseHarmonicPace::EveryBar);
    let chord: ChordSymbol =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major"})).unwrap();
    let entries = (0..4)
        .map(|measure| ChordPlanEntry {
            position: ChordPlanPosition {
                measure_index: measure,
                offset: BeatFraction::ZERO,
            },
            offset_beats: 0.0,
            chord: chord.clone(),
            origin: ChordPlanOrigin::HostAuthored,
            sources: vec![],
        })
        .collect::<Vec<_>>();
    input.proposal.source_plan = Some(ChordPlan {
        contract_version: 1,
        entries: entries.clone(),
    });
    input.proposal.max_changed_spans = Some(2);
    input.locked_chord_entries = vec![1];
    let result = suggest_phrase_harmony(&source, &input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .proposals
            .candidates
            .iter()
            .all(|item| item.plan.entries[1] == entries[1])
    );
    input.phrases[0].pace = PhraseHarmonicPace::EverySecondBar;
    let rejected = suggest_phrase_harmony(&source, &input);
    assert_eq!(
        rejected.diagnostics[0].code,
        "source-harmonic-positions-mismatch"
    );
    assert!(rejected.proposals.candidates.is_empty());
}

#[test]
fn gaps_invalid_groups_and_unanchored_locks_fail_atomically() {
    let source = score(5, 8, 4);
    for case in 0..4 {
        let mut input = request(4, PhraseHarmonicPace::TwoGroupsPerBar);
        match case {
            0 => input.phrases[0].start_measure = 1,
            1 => input.phrases[0].meter_groups = vec![2, 2],
            2 => input.locked_chord_entries = vec![0],
            3 => input.phrases[0].end_measure_exclusive = 3,
            _ => unreachable!(),
        }
        let result = suggest_phrase_harmony(&source, &input);
        assert!(result.positions.is_empty(), "case {case}");
        assert!(result.proposals.candidates.is_empty());
        assert!(!result.diagnostics.is_empty());
    }
}
