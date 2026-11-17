#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, Duration, InstrumentDefinition, InstrumentPartAssignment, InstrumentPlanError,
    InstrumentPlanRequest, InstrumentRange, Note, PartTechnique, Pitch, Score,
    arrange_form_inner_voices_performance, plan_instruments, replay_instrument_plan,
};

fn repeated_form() -> Score {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = Barline::RepeatEnd;
    arrange_form_inner_voices_performance(&source, &fixture::form_inner_voice_request())
        .unwrap()
        .performance_score
}

fn instrument(id: &str, program: u8, channel: u8, range: InstrumentRange) -> InstrumentDefinition {
    let mut instrument = InstrumentDefinition::new(id, id);
    instrument.midi_program = program;
    instrument.midi_channel = channel;
    instrument.sounding_range = Some(range);
    instrument
}

fn request(inner_part: usize, inner_channel: u8) -> InstrumentPlanRequest {
    InstrumentPlanRequest {
        assignments: vec![
            InstrumentPartAssignment {
                part_index: 0,
                instrument: instrument(
                    "wind-melody",
                    73,
                    0,
                    InstrumentRange {
                        lowest: 80,
                        highest: 95,
                    },
                ),
                sounding_range: InstrumentRange {
                    lowest: 84,
                    highest: 91,
                },
                technique: PartTechnique::WindLine {
                    max_continuous_beats: 16.0,
                    min_breath_rest_beats: 1.0,
                },
            },
            InstrumentPartAssignment {
                part_index: inner_part,
                instrument: instrument(
                    "piano-right-hand",
                    0,
                    inner_channel,
                    InstrumentRange {
                        lowest: 48,
                        highest: 90,
                    },
                ),
                sounding_range: InstrumentRange {
                    lowest: 55,
                    highest: 71,
                },
                technique: PartTechnique::KeyboardHand {
                    max_simultaneous_pitches: 4,
                    max_span_semitones: 16,
                },
            },
        ],
    }
}

#[test]
fn sectioned_repeat_can_be_assigned_to_wind_and_keyboard_hand() {
    let score = repeated_form();
    let inner_part = score.parts.len() - 1;
    let request = request(inner_part, score.parts[inner_part].midi_channel);
    let result = plan_instruments(&score, &request).unwrap();
    assert_eq!(result.reports.len(), 2);
    assert!(result.reports[0].longest_continuous_beats <= 16.0);
    assert!(result.reports[1].peak_simultaneous_pitches >= 1);
    assert!(
        result
            .score
            .parts
            .iter()
            .zip(&score.parts)
            .all(|(assigned, original)| assigned.id == original.id)
    );
    assert_eq!(fixture::events(&score), fixture::events(&result.score));
    assert_eq!(
        result.score.parts[0].instrument.as_ref().unwrap().id,
        "wind-melody"
    );
    assert_eq!(result.score.parts[0].midi_program, 73);
    let mut wrong_channel = request.clone();
    wrong_channel.assignments[1].instrument.midi_channel = 0;
    assert!(matches!(
        plan_instruments(&score, &wrong_channel),
        Err(InstrumentPlanError::InvalidRequest)
    ));
    let replay = replay_instrument_plan(&score, &result.resolved).unwrap();
    assert_eq!(
        fixture::events(&result.score),
        fixture::events(&replay.score)
    );
    let mut changed = score.clone();
    changed.metadata.title.push('!');
    assert!(matches!(
        replay_instrument_plan(&changed, &result.resolved),
        Err(InstrumentPlanError::SourceMismatch)
    ));
}

#[test]
fn wind_breath_check_includes_return_visit() {
    let mut score = Score::new("Continuous", 120, 4, 4, 0, 2);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = (0..4)
            .map(|_| Note::new(Pitch::from_midi(72, false), Duration::Quarter))
            .collect();
    }
    let request = InstrumentPlanRequest {
        assignments: vec![InstrumentPartAssignment {
            part_index: 0,
            instrument: instrument(
                "wind",
                73,
                0,
                InstrumentRange {
                    lowest: 60,
                    highest: 84,
                },
            ),
            sounding_range: InstrumentRange {
                lowest: 60,
                highest: 84,
            },
            technique: PartTechnique::WindLine {
                max_continuous_beats: 12.0,
                min_breath_rest_beats: 0.5,
            },
        }],
    };
    assert!(plan_instruments(&score, &request).is_ok());
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Breath { beat, .. }) if beat >= 12.0
    ));
}

#[test]
fn keyboard_screen_rejects_chord_count_span_and_range() {
    let mut score = Score::new("Keyboard", 120, 4, 4, 0, 1);
    let mut chord = Note::new(Pitch::from_midi(60, false), Duration::Whole);
    chord.pitches.push(Pitch::from_midi(64, false));
    chord.pitches.push(Pitch::from_midi(72, false));
    score.parts[0].staves[0].measures[0].voices[0] = vec![chord];
    let mut request = InstrumentPlanRequest {
        assignments: vec![InstrumentPartAssignment {
            part_index: 0,
            instrument: instrument(
                "piano-hand",
                0,
                0,
                InstrumentRange {
                    lowest: 48,
                    highest: 84,
                },
            ),
            sounding_range: InstrumentRange {
                lowest: 48,
                highest: 84,
            },
            technique: PartTechnique::KeyboardHand {
                max_simultaneous_pitches: 2,
                max_span_semitones: 16,
            },
        }],
    };
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Simultaneous { count: 3, .. })
    ));
    request.assignments[0].technique = PartTechnique::KeyboardHand {
        max_simultaneous_pitches: 3,
        max_span_semitones: 11,
    };
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Span { span: 12, .. })
    ));
    request.assignments[0].sounding_range.lowest = 64;
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Range { pitch: 60, .. })
    ));
    request.assignments[0].sounding_range.lowest = 48;
    request.assignments[0].sounding_range.highest = 72;
    score.parts[0].staves[0].measures[0].voices[0][0].pitches[2].microtone_cents = 25;
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Range { pitch: 72, .. })
    ));
    request.assignments[0].sounding_range.highest = 84;
    request.assignments[0].technique = PartTechnique::KeyboardHand {
        max_simultaneous_pitches: 3,
        max_span_semitones: 12,
    };
    assert!(matches!(
        plan_instruments(&score, &request),
        Err(InstrumentPlanError::Span { span: 13, .. })
    ));
}
