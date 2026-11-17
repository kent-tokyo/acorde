use acorde_core::{
    AccordionBassMode, AccordionDynamicSchedule, AccordionDynamicSection, AccordionPattern,
    Barline, Duration, Dynamic, Note, Pitch, PositionedAccordionChord, Score,
    SectionedAccordionOptions, arrange_sections_with_dynamics,
};

fn fixture(count: u32) -> (Score, SectionedAccordionOptions) {
    let mut score = Score::new("accompaniment energy", 120, 4, 4, 0, count);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(72, false), Duration::Whole)];
    }
    let options = SectionedAccordionOptions {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({"root":"C","kind":"major"})).unwrap(),
        }],
        bass_mode: AccordionBassMode::FreeBass,
        default_pattern: Some(AccordionPattern::HeldBassChord),
        sections: Vec::new(),
    };
    (score, options)
}

fn section(first_measure: usize, last_measure: usize, dynamic: Dynamic) -> AccordionDynamicSection {
    AccordionDynamicSection {
        first_measure,
        last_measure,
        dynamic,
    }
}

#[test]
fn dynamics_change_accompaniment_velocity_without_changing_melody_or_rhythm() {
    let (score, mut accompaniment) = fixture(4);
    accompaniment
        .sections
        .push(acorde_core::AccordionAccompanimentSection {
            first_measure: 2,
            last_measure: 2,
            pattern: None,
        });
    let source_melody = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
    let result = arrange_sections_with_dynamics(
        &score,
        accompaniment,
        AccordionDynamicSchedule {
            default_dynamic: Dynamic::Mp,
            sections: vec![section(3, 3, Dynamic::P), section(1, 1, Dynamic::F)],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(
        result.resolved_dynamics,
        vec![Dynamic::Mp, Dynamic::F, Dynamic::Mp, Dynamic::P]
    );
    assert_eq!(
        result
            .dynamic_marks
            .iter()
            .map(|mark| mark.measure)
            .collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
    let output = result.arrangement.score.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
        source_melody
    );
    assert_eq!(
        serde_json::to_value(&score.parts[0].staves[0]).unwrap(),
        source_melody
    );
    assert!(
        output.parts[0].staves[1].measures[2].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert!(acorde_core::model::validate::validate(output).is_valid());
    let events = acorde_core::to_playback_events(output, &Default::default());
    let melody_velocities = events
        .iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|source| source.staff == 0)
        })
        .map(|event| event.velocity)
        .collect::<Vec<_>>();
    assert_eq!(melody_velocities, vec![64, 64, 64, 64]);
    let mut left = Vec::new();
    for event in events.iter().filter(|event| {
        event
            .source
            .as_ref()
            .is_some_and(|source| source.staff == 1)
    }) {
        let measure = event.source.as_ref().unwrap().measure;
        if left.last().is_none_or(|(last, _)| *last != measure) {
            left.push((measure, event.velocity));
        }
    }
    assert_eq!(left, vec![(0, 60), (1, 84), (3, 48)]);
}

#[test]
fn written_dynamic_marks_reset_at_repeat_back_edges() {
    let (mut score, accompaniment) = fixture(2);
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    let result = arrange_sections_with_dynamics(
        &score,
        accompaniment,
        AccordionDynamicSchedule {
            default_dynamic: Dynamic::Mp,
            sections: vec![section(1, 1, Dynamic::F)],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert_eq!(acorde_core::measure_sequence(&output), vec![0, 1, 0, 1]);
    let mut attacks = Vec::new();
    for event in acorde_core::to_playback_events(&output, &Default::default())
        .iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|source| source.staff == 1)
        })
    {
        if attacks
            .last()
            .is_none_or(|(time, _)| *time != event.time_beats)
        {
            attacks.push((event.time_beats, event.velocity));
        }
    }
    assert_eq!(attacks, vec![(0.0, 60), (4.0, 84), (8.0, 60), (12.0, 84)]);
}

#[test]
fn invalid_dynamic_sections_return_no_partial_score() {
    let (score, accompaniment) = fixture(2);
    for (schedule, code) in [
        (
            AccordionDynamicSchedule {
                default_dynamic: Dynamic::Mp,
                sections: vec![section(0, 2, Dynamic::F)],
            },
            "dynamic-section-range",
        ),
        (
            AccordionDynamicSchedule {
                default_dynamic: Dynamic::Mp,
                sections: vec![section(0, 1, Dynamic::F), section(1, 1, Dynamic::P)],
            },
            "overlapping-dynamic-sections",
        ),
        (
            AccordionDynamicSchedule {
                default_dynamic: Dynamic::Sfz,
                sections: vec![],
            },
            "unsupported-dynamic",
        ),
        (
            AccordionDynamicSchedule {
                default_dynamic: Dynamic::Mp,
                sections: vec![section(0, 0, Dynamic::Fp)],
            },
            "unsupported-dynamic",
        ),
    ] {
        let result = arrange_sections_with_dynamics(&score, accompaniment.clone(), schedule);
        assert!(result.arrangement.score.is_none());
        assert!(result.arrangement.generated_notes.is_empty());
        assert!(result.arrangement.source_notes.is_empty());
        assert!(result.dynamic_marks.is_empty());
        assert!(result.resolved_dynamics.is_empty());
        assert_eq!(result.arrangement.diagnostics[0].code, code);
    }
}
