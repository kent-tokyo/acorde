#[path = "../../../tests/fixtures/arrangement/harmony_arrangement_source.rs"]
mod fixture;
use acorde_core::{
    Articulation, Barline, BeatFraction, Duration, HarmonyArrangementError, HarmonyContextFeature,
    MeasureLength, Note, OttavaKind, Pitch, RootVoicingDecision, arrange_harmony_voicings,
    replay_harmony_arrangement,
};

#[test]
fn spelled_accompaniment_uses_actual_bass_and_preserves_source_parts() {
    let source = fixture::source();
    let before = serde_json::to_value(&source).unwrap();
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&result.score.parts[..2]).unwrap(),
        before["parts"]
    );
    assert_eq!(result.added_part, 2);
    assert!(acorde_core::validate(&result.score).is_valid());
    assert_eq!(result.context.nodes[0].bass_midi, Some(38));
    assert_eq!(result.context.nodes[0].melody_ceiling_midi, Some(81));
    assert_eq!(
        result.voicings.voicings[0].root_decision,
        RootVoicingDecision::OmittedWithBassSupport
    );
    let mut letters = result.score.parts[2].staves[0].measures[0].voices[0][0]
        .pitches
        .iter()
        .map(|p| p.step.to_char())
        .collect::<Vec<_>>();
    letters.sort_unstable();
    assert_eq!(letters, ['A', 'C', 'E', 'F']);
    assert_eq!(
        fixture::sound(&result.score)
            .into_iter()
            .filter(|e| e.0 < 2)
            .collect::<Vec<_>>(),
        fixture::sound(&source)
    );
    assert_eq!(result.generated_notes.len(), 3);
    for note in &result.generated_notes {
        assert_eq!(note.address.measure, result.spans[note.span].measure);
    }
}

#[test]
fn incomplete_or_staccato_bass_does_not_authorize_root_omission() {
    let mut source = fixture::source();
    source.parts[1].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::from_midi(38, false), Duration::Half),
        Note::rest(Duration::Half),
    ];
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].bass_midi, None);
    assert_eq!(
        result.voicings.voicings[0].root_decision,
        RootVoicingDecision::IncludedWithoutBassSupport
    );
    source.parts[1].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(38, false), Duration::Whole)];
    source.parts[1].staves[0].measures[0].voices[0][0]
        .articulations
        .push(Articulation::Staccato);
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].bass_midi, None);
    let mut request = fixture::request();
    request.bass = None;
    assert!(
        arrange_harmony_voicings(&source, &request)
            .unwrap()
            .voicings
            .voicings
            .iter()
            .all(|v| v.root_decision == RootVoicingDecision::IncludedWithoutBassSupport)
    );
}

#[test]
fn bass_register_and_mid_interval_melody_limit_all_the_generated_notes() {
    let mut source = fixture::source();
    source.parts[1].staves[0].measures[0].voices[0][0].pitches = vec![Pitch::from_midi(50, false)];
    source.parts[0].staves[0].measures[0].voices[0] = [84, 84, 72, 84]
        .into_iter()
        .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
        .collect();
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].accompaniment_floor_midi, Some(53));
    assert_eq!(result.context.nodes[0].melody_ceiling_midi, Some(69));
    assert!(
        result.voicings.voicings[0]
            .tones
            .iter()
            .all(|t| (53..=69).contains(&t.pitch.to_midi()))
    );
    assert_eq!(result.context.nodes[0].bass_midi, Some(50));
    assert_eq!(
        result.voicings.voicings[0].root_decision,
        RootVoicingDecision::OmittedWithBassSupport
    );
}

#[test]
fn sounding_transposition_and_ottava_are_included_in_context() {
    let mut source = fixture::source();
    source.parts[0].staves[0].transpose_semitones = -2;
    let melody = &mut source.parts[0].staves[0].measures[0].voices[0][0];
    melody.ottava_start = Some(OttavaKind::Vb8);
    melody.ottava_end = true;
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].melody_ceiling_midi, Some(67));
    assert_eq!(result.context.nodes[1].melody_ceiling_midi, Some(79));
}

#[test]
fn local_instrument_context_keeps_its_channel_separate_from_added_harmony() {
    let mut source = fixture::source();
    let mut instrument = acorde_core::InstrumentDefinition::new("transposing-wind", "Wind");
    instrument.midi_channel = 2;
    instrument.midi_program = 71;
    instrument.transpose_semitones = -2;
    source.parts[0].staves[0].measures[0].instrument_change = Some(instrument);
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].melody_ceiling_midi, Some(79));
    assert_eq!(result.score.parts[2].midi_channel, 3);
    assert_eq!(
        fixture::sound(&result.score)
            .into_iter()
            .filter(|e| e.0 < 2)
            .collect::<Vec<_>>(),
        fixture::sound(&source)
    );
}

#[test]
fn repeat_return_and_locked_range_are_derived_from_the_score() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[2].barline_right = Barline::RepeatEnd;
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.edges.len(), 5);
    assert!(
        result
            .context
            .edges
            .iter()
            .any(|e| e.repeat_boundary && e.from == 2 && e.to == 0)
    );
    let mut request = fixture::request();
    request.start_measure = 1;
    request.locked_measures = vec![1];
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    assert_eq!(result.spans.len(), 1);
    assert_eq!(result.spans[0].chord_entry, 2);
    assert!(
        result.score.parts[2].staves[0].measures[..2]
            .iter()
            .flat_map(|bar| &bar.voices[0])
            .all(|n| n.is_rest)
    );
    request.locked_measures.push(2);
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        HarmonyArrangementError::EmptySelection
    );
}

#[test]
fn pickup_final_bar_and_exact_fraction_validation_never_round() {
    let mut source = fixture::source();
    for part in &mut source.parts {
        let bar = &mut part.staves[0].measures[0];
        bar.actual_length = Some(MeasureLength {
            numerator: 1,
            denominator: 8,
        });
        bar.voices[0][0].duration = Duration::Eighth;
        let bar = &mut part.staves[0].measures[2];
        bar.actual_length = Some(MeasureLength {
            numerator: 7,
            denominator: 8,
        });
        let pitch = bar.voices[0][0].pitches[0].clone();
        bar.voices[0] = vec![
            Note::new(pitch.clone(), Duration::Half),
            Note::new(pitch.clone(), Duration::Quarter),
            Note::new(pitch, Duration::Eighth),
        ];
    }
    let mut request = fixture::request();
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    let output_bars = &result.score.parts[2].staves[0].measures;
    assert_eq!(
        output_bars[0].voices[0]
            .iter()
            .map(Note::beats)
            .sum::<f64>(),
        0.5
    );
    assert_eq!(
        output_bars[2].voices[0]
            .iter()
            .map(Note::beats)
            .sum::<f64>(),
        3.5
    );
    assert!(output_bars[2].voices[0][0].tie_start);
    assert!(output_bars[2].voices[0][1].tie_end);
    request.chords.entries[1].position.offset = BeatFraction {
        numerator: 1,
        denominator: 64,
    };
    request.chords.entries[1].offset_beats = 1.0 / 64.0;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        HarmonyArrangementError::UnrepresentableTime { measure: 1 }
    );
    request.chords.entries[1].position.offset.denominator = 0;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        HarmonyArrangementError::InvalidTime
    );
}

#[test]
fn replay_normalizes_generated_source_ids_but_detects_musical_change() {
    let source = fixture::source();
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    let equivalent = fixture::source();
    let replay = replay_harmony_arrangement(&equivalent, &result.resolved).unwrap();
    assert_eq!(fixture::sound(&result.score), fixture::sound(&replay.score));
    assert_eq!(result.context, replay.context);
    assert_eq!(result.generated_notes, replay.generated_notes);
    let mut modified = equivalent;
    modified.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(83, false);
    assert_eq!(
        replay_harmony_arrangement(&modified, &result.resolved).unwrap_err(),
        HarmonyArrangementError::ReplaySource
    );
    let mut resolved = result.resolved;
    resolved.engine_version.push_str("future");
    assert_eq!(
        replay_harmony_arrangement(&source, &resolved).unwrap_err(),
        HarmonyArrangementError::ReplayVersion
    );
}

#[test]
fn unsupported_effects_are_identified_before_returning_a_score() {
    let mut source = fixture::source();
    source.parts[1].staves[0].measures[0].voices[0][0].pedal_start = true;
    assert!(matches!(
        arrange_harmony_voicings(&source, &fixture::request()),
        Err(HarmonyArrangementError::UnsupportedContext {
            feature: HarmonyContextFeature::Pedal,
            ..
        })
    ));
}

#[test]
fn tuplet_source_context_and_altered_spelling_survive_actual_note_generation() {
    let mut source = fixture::source();
    let mut triplets = [84, 72, 84]
        .into_iter()
        .map(|midi| {
            let mut note = Note::new(Pitch::from_midi(midi, false), Duration::Eighth);
            note.tuplet = Some(
                serde_json::from_value(serde_json::json!({"actual_notes":3,"normal_notes":2}))
                    .unwrap(),
            );
            note
        })
        .collect::<Vec<_>>();
    let mut last = Note::new(Pitch::from_midi(84, false), Duration::Half);
    last.dot_count = 1;
    triplets.push(last);
    source.parts[0].staves[0].measures[0].voices[0] = triplets;
    let result = arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    assert_eq!(result.context.nodes[0].melody_ceiling_midi, Some(69));
    let mut request = fixture::request();
    request.chords.entries.truncate(1);
    request.chords.entries[0].chord = serde_json::from_value(serde_json::json!({
        "root":"B#","kind":"major-seventh"
    }))
    .unwrap();
    request.root_policy = acorde_core::HarmonyRootPolicy::Include;
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    let pitches = &result.score.parts[2].staves[0].measures[0].voices[0][0].pitches;
    assert!(
        pitches
            .iter()
            .any(|p| p.step == acorde_core::Step::B && p.alter == 1)
    );
    assert!(
        pitches
            .iter()
            .any(|p| p.step == acorde_core::Step::D && p.alter == 2)
    );
    assert!(
        pitches
            .iter()
            .any(|p| p.step == acorde_core::Step::F && p.alter == 2)
    );
}
