use acorde_core::model::notation::{TupletBoundary, TupletInfo};
use acorde_core::model::score::MeasureLength;
use acorde_core::*;

fn melody(count: usize) -> Score {
    let mut score = Score::new("plan", 120, 4, 4, 0, count as u32);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![Note::new(Pitch::from_midi(67, false), Duration::Whole)];
    }
    score
}

fn options() -> AccordionPlanOptions {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement_plan_eight_bars.json"
    ))
    .unwrap()
}

fn events(score: &Score) -> Vec<(u8, f64, f64, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats, e.velocity))
        .collect()
}

fn success(result: AccordionPlanResult) -> Score {
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.score.unwrap()
}

fn failed(score: &Score, options: AccordionPlanOptions, code: &str, measure: Option<usize>) {
    let result = arrange_accordion_plan(score, options);
    assert!(result.score.is_none());
    assert!(result.resolved_config.is_none());
    assert!(result.source_notes.is_empty());
    assert!(result.generated_notes.is_empty());
    assert!(result.applied_sections.is_empty());
    assert_eq!(result.diagnostics[0].code, code, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].measure_index, measure);
}

#[test]
fn explicit_sections_preserve_source_and_match_existing_patterns() {
    let score = melody(8);
    let before = serde_json::to_value(&score).unwrap();
    let options = options();
    let result = arrange_accordion_plan(&score, options.clone());
    let output = result.score.as_ref().expect("arranged score");
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&output.settings).unwrap(),
        serde_json::to_value(&score.settings).unwrap()
    );
    assert_eq!(result.applied_sections, options.sections);
    assert_eq!(
        output.parts[0].staves[1]
            .measures
            .iter()
            .map(|m| m.voices[0].len())
            .collect::<Vec<_>>(),
        [4, 4, 4, 4, 8, 8, 8, 1]
    );
    for (mi, measure) in output.parts[0].staves[0].measures.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&measure.voices[0]).unwrap(),
            serde_json::to_value(&score.parts[0].staves[0].measures[mi].voices[0]).unwrap()
        );
        assert_eq!(!measure.voices[1].is_empty(), (4..7).contains(&mi));
    }
    for section in &options.sections {
        let reference = arrange_with_layers(
            &score,
            LayeredAccordionOptions {
                chords: options.chords.clone(),
                alternative_chords: None,
                inversion: None,
                accompaniment: section.accompaniment,
                melodic_voice: section.melodic_voice,
                swing: AccordionSwingPolicy::Straight,
                bass_mode: options.bass_mode,
            },
        )
        .score
        .unwrap();
        let local = |s: &Score| {
            to_playback_events(s, &Default::default())
                .into_iter()
                .filter(|e| {
                    e.source.as_ref().is_some_and(|a| {
                        (section.start_measure..section.end_measure).contains(&a.measure)
                    })
                })
                .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats, e.velocity))
                .collect::<Vec<_>>()
        };
        assert_eq!(local(output), local(&reference));
    }
    for generated in &result.generated_notes {
        let address = &generated.address;
        let section = &result.applied_sections[generated.section_index];
        assert!((section.start_measure..section.end_measure).contains(&address.measure));
        let note = &output.parts[0].staves[address.staff].measures[address.measure].voices
            [address.voice][address.note];
        assert!(!note.is_rest);
        match &generated.origin {
            AccordionPlanOrigin::MelodicVoice {
                source,
                chord_index,
            } => {
                assert_eq!(source.voice, 0);
                assert_eq!(source.measure, address.measure);
                assert_eq!(source.note, address.note);
                assert_eq!(note.pitches[0].to_midi(), 64);
                assert!(result.resolved_chords[*chord_index].measure_index <= address.measure);
            }
            AccordionPlanOrigin::Accompaniment { chord_index } => {
                assert_eq!(address.staff, 1);
                assert!(result.resolved_chords[*chord_index].measure_index <= address.measure);
            }
        }
    }
    let middle = result
        .generated_notes
        .iter()
        .find(|n| n.address.staff == 1 && n.address.measure == 4 && n.address.note == 4)
        .unwrap();
    assert_eq!(
        middle.origin,
        AccordionPlanOrigin::Accompaniment { chord_index: 1 }
    );
}

#[test]
fn range_errors_are_atomic_and_located() {
    let score = melody(8);
    for (start, end, code, measure) in [
        (0, 0, "invalid-range", 0),
        (0, 9, "invalid-range", 0),
        (1, 4, "uncovered-measures", 0),
    ] {
        let mut plan = options();
        plan.sections[0].start_measure = start;
        plan.sections[0].end_measure = end;
        failed(&score, plan, code, Some(measure));
    }
    let mut plan = options();
    plan.sections[1].start_measure = 3;
    failed(&score, plan, "overlapping-range", Some(3));
    let mut plan = options();
    plan.sections.pop();
    failed(&score, plan, "uncovered-measures", Some(7));
    let mut plan = options();
    plan.sections.clear();
    failed(&score, plan, "uncovered-measures", Some(0));
}

#[test]
fn unsupported_choices_never_fall_back() {
    let score = melody(8);
    let mut plan = options();
    plan.sections[1].accompaniment = Some(AccordionPattern::Arpeggio);
    failed(&score, plan, "unsupported-standard-bass", Some(4));
    let mut plan = options();
    plan.sections[1].accompaniment = Some(AccordionPattern::WaltzBassChord);
    failed(&score, plan, "unsupported-meter", Some(4));
    let mut plan = options();
    plan.sections[1].melodic_voice = AccordionMelodicVoice::SixthBelow;
    failed(&score, plan, "incompatible-harmony", Some(4));
    let mut plan = options();
    plan.chords.push(PositionedAccordionChord {
        measure_index: 4,
        offset_beats: 0.25,
        chord: plan.chords[0].chord.clone(),
    });
    // A real harmony change on a non-eighth boundary cannot be silently rounded.
    plan.chords.last_mut().unwrap().chord.root = "F".into();
    failed(&score, plan, "unsupported-rhythm", Some(4));
}

#[test]
fn section_constraints_use_global_meter_and_tempo_context() {
    let mut score = melody(8);
    score.settings.tempo_bpm = 240;
    score.parts[0].staves[0].measures[4].tempo = Some(120);
    success(arrange_accordion_plan(&score, options()));
    score.parts[0].staves[0].measures[4].tempo = None;
    failed(&score, options(), "unsupported-standard-bass-rate", Some(4));
    score.parts[0].staves[0].measures[4].tempo = Some(120);
    score.parts[0].staves[0].measures[5].tempo_ramp_to = Some(240);
    failed(&score, options(), "unsupported-standard-bass-rate", Some(5));
    let mut score = melody(8);
    for measure in &mut score.parts[0].staves[0].measures[..4] {
        measure.voices[0][0].duration = Duration::Half;
        measure.voices[0][0].dot_count = 1;
    }
    score.parts[0].staves[0].measures[0].time_sig = Some(TimeSignature {
        numerator: 3,
        denominator: 4,
    });
    score.parts[0].staves[0].measures[4].time_sig = Some(TimeSignature {
        numerator: 4,
        denominator: 4,
    });
    let mut plan = options();
    plan.sections[0].accompaniment = Some(AccordionPattern::WaltzBassChord);
    success(arrange_accordion_plan(&score, plan));
}

#[test]
fn pickup_and_short_final_keep_authored_phase_and_length() {
    let mut score = melody(8);
    for mi in [0, 7] {
        let measure = &mut score.parts[0].staves[0].measures[mi];
        measure.actual_length = MeasureLength::from_beats(0.5);
        measure.voices[0][0].duration = Duration::Eighth;
    }
    let mut plan = options();
    plan.sections[0].accompaniment = Some(AccordionPattern::DrivingBassChord);
    let output = success(arrange_accordion_plan(&score, plan));
    let first = &output.parts[0].staves[1].measures[0].voices[0];
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].pitches.len(), 3); // Last offbeat chord, not a new downbeat bass.
    assert!(!first[0].articulations.contains(&Articulation::Accent));
    assert_eq!(output.playback_measure_beats(0), 0.5);
    assert_eq!(output.playback_measure_beats(7), 0.5);
    assert!(
        to_playback_events(&output, &Default::default())
            .iter()
            .any(|e| e.source.as_ref().is_some_and(|a| a.measure == 1) && e.time_beats == 0.5)
    );
}

#[test]
fn tie_boundaries_reject_cuts_and_preserve_continuous_octaves() {
    let mut score = melody(8);
    score.parts[0].staves[0].measures[3].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[4].voices[0][0].tie_end = true;
    failed(&score, options(), "unsafe-tie-boundary", Some(4));
    let mut plan = options();
    for section in &mut plan.sections {
        section.melodic_voice = AccordionMelodicVoice::OctaveBelow;
    }
    let output = success(arrange_accordion_plan(&score, plan.clone()));
    assert!(output.parts[0].staves[0].measures[3].voices[1][0].tie_start);
    assert!(output.parts[0].staves[0].measures[4].voices[1][0].tie_end);
    let events = to_playback_events(&output, &Default::default());
    assert!(
        events
            .iter()
            .any(|e| e.pitch_midi == 55 && e.time_beats == 12.0 && e.duration_beats == 8.0)
    );
    for section in &mut plan.sections {
        section.melodic_voice = AccordionMelodicVoice::Unchanged;
    }
    success(arrange_accordion_plan(&score, plan));
}

#[test]
fn cross_bar_tuplets_are_kept_or_rejected_at_layer_cuts() {
    let mut score = melody(8);
    // Three dotted halves in the time of two: two beats per note, six in total.
    // The group begins after a half rest in bar 4 and ends at the end of bar 5.
    let make_note = |boundary| {
        let mut note = Note::new(Pitch::from_midi(67, false), Duration::Half);
        note.dot_count = 1;
        note.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        note.tuplet_boundary = boundary;
        note
    };
    score.parts[0].staves[0].measures[3].voices[0] = vec![
        Note::rest(Duration::Half),
        make_note(Some(TupletBoundary::Start)),
    ];
    score.parts[0].staves[0].measures[4].voices[0] =
        vec![make_note(None), make_note(Some(TupletBoundary::Stop))];
    failed(&score, options(), "unsafe-tuplet-boundary", Some(4));
    let mut plan = options();
    for section in &mut plan.sections {
        section.melodic_voice = AccordionMelodicVoice::OctaveBelow;
    }
    let output = success(arrange_accordion_plan(&score, plan));
    assert_eq!(
        output.parts[0].staves[0].measures[4].voices[1][1].tuplet_boundary,
        Some(TupletBoundary::Stop)
    );
}

#[test]
fn replay_is_versioned_source_checked_and_uuid_independent() {
    let score = melody(8);
    let arranged = arrange_accordion_plan(&score, options());
    let config = arranged.resolved_config.unwrap();
    assert_eq!(
        config.engine_version,
        format!("{}+arrangement-plan.1", env!("CARGO_PKG_VERSION"))
    );
    let mut previous = config.clone();
    previous.engine_version = "1.3.15+arrangement-plan.1".into();
    assert_eq!(
        arrange_from_resolved_plan(&score, previous).diagnostics[0].code,
        "unsupported-version"
    );
    let saved = serde_json::to_string(&config).unwrap();
    let mut reparsed = score.clone();
    reparsed.id = uuid::Uuid::new_v4().to_string();
    reparsed.parts[0].id = uuid::Uuid::new_v4().to_string();
    for measure in &mut reparsed.parts[0].staves[0].measures {
        for note in &mut measure.voices[0] {
            note.id = uuid::Uuid::new_v4().to_string();
        }
    }
    let replay = success(arrange_from_resolved_plan(
        &reparsed,
        serde_json::from_str(&saved).unwrap(),
    ));
    assert_eq!(events(arranged.score.as_ref().unwrap()), events(&replay));
    reparsed.settings.tempo_bpm += 1;
    assert_eq!(
        arrange_from_resolved_plan(&reparsed, config.clone()).diagnostics[0].code,
        "input-mismatch"
    );
    for bad in [
        AccordionPlanConfig {
            contract_version: 99,
            ..config.clone()
        },
        AccordionPlanConfig {
            engine_version: "future".into(),
            ..config
        },
    ] {
        assert_eq!(
            arrange_from_resolved_plan(&score, bad).diagnostics[0].code,
            "unsupported-version"
        );
    }
}

#[test]
fn budgets_and_explicit_no_layer_sections() {
    let score = melody(8);
    let mut plan = options();
    plan.sections[0].accompaniment = None;
    let output = success(arrange_accordion_plan(&score, plan));
    assert!(
        output.parts[0].staves[1].measures[..4]
            .iter()
            .all(|m| m.voices[0].is_empty())
    );
    let mut plan = options();
    plan.sections = vec![plan.sections[0].clone(); ACCORDION_PLAN_MAX_SECTIONS + 1];
    failed(&score, plan, "limit-exceeded", None);
    let mut plan = options();
    plan.chords = vec![plan.chords[0].clone(); ACCORDION_PLAN_MAX_CHORDS + 1];
    failed(&score, plan, "limit-exceeded", None);
    failed(
        &melody(ACCORDION_PLAN_MAX_MEASURES + 1),
        options(),
        "limit-exceeded",
        None,
    );
    let mut score = melody(8);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::rest(Duration::Quarter); ACCORDION_PLAN_MAX_SOURCE_NOTES + 1];
    failed(&score, options(), "limit-exceeded", None);
}
