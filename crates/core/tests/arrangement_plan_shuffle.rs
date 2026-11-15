use std::collections::BTreeSet;

use acorde_core::model::notation::{TupletBoundary, TupletInfo};
use acorde_core::model::score::MeasureLength;
use acorde_core::*;

fn source() -> Score {
    let mut score = Score::new("shuffle", 120, 4, 4, 0, 8);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![Note::new(Pitch::from_midi(67, false), Duration::Whole)];
    }
    score
}

fn request() -> AccordionPlanShuffleRequest {
    let plan: AccordionPlanOptions = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement_plan_eight_bars.json"
    ))
    .unwrap();
    AccordionPlanShuffleRequest {
        chords: plan.chords,
        bass_mode: plan.bass_mode,
        sections: plan
            .sections
            .into_iter()
            .map(|s| AccordionPlanSectionChoices {
                start_measure: s.start_measure,
                end_measure: s.end_measure,
                allowed_accompaniments: vec![s.accompaniment],
                allowed_melodic_voices: vec![s.melodic_voice],
            })
            .collect(),
        seed: "18446744073709551615".into(),
        max_attempts: 64,
    }
}

fn events(score: &Score) -> Vec<(u8, f64, f64)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
        .collect()
}

#[test]
fn seeded_selection_is_stable_and_replays_the_resolved_plan() {
    let score = source();
    let before = serde_json::to_value(&score).unwrap();
    let mut req = request();
    req.sections[0]
        .allowed_accompaniments
        .push(Some(AccordionPattern::HeldBassChord));
    req.sections[1]
        .allowed_accompaniments
        .push(Some(AccordionPattern::HeldBassChord));
    req.sections[1]
        .allowed_melodic_voices
        .push(AccordionMelodicVoice::Unchanged);
    let first = shuffle_accordion_plan(&score, req.clone());
    let again = shuffle_accordion_plan(&score, req.clone());
    assert!(first.plan.score.is_some(), "{:?}", first.plan.diagnostics);
    assert_eq!(first.pool_size, 8);
    assert_eq!(first.selected_pool_index, again.selected_pool_index);
    assert_eq!(first.plan.applied_sections, again.plan.applied_sections);
    assert_eq!(first.plan.generated_notes, again.plan.generated_notes);
    assert_eq!(
        events(first.plan.score.as_ref().unwrap()),
        events(again.plan.score.as_ref().unwrap())
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    let replay = arrange_from_resolved_plan(&score, first.plan.resolved_config.clone().unwrap());
    assert_eq!(
        events(first.plan.score.as_ref().unwrap()),
        events(replay.score.as_ref().unwrap())
    );
    assert_eq!(first.plan.generated_notes, replay.generated_notes);

    req.sections[0].allowed_accompaniments.reverse();
    req.sections[0]
        .allowed_accompaniments
        .push(Some(AccordionPattern::HeldBassChord));
    req.sections[1].allowed_melodic_voices.reverse();
    let canonical = shuffle_accordion_plan(&score, req);
    assert_eq!(first.selected_pool_index, canonical.selected_pool_index);
    assert_eq!(first.plan.applied_sections, canonical.plan.applied_sections);
    assert_eq!(first.pool_size, canonical.pool_size);

    let mut variants = BTreeSet::new();
    let mut seeds = request();
    seeds.sections[0].allowed_accompaniments = vec![
        Some(AccordionPattern::BassChordPulse),
        Some(AccordionPattern::HeldBassChord),
    ];
    for seed in 0..32 {
        seeds.seed = seed.to_string();
        let chosen = shuffle_accordion_plan(&score, seeds.clone());
        variants.insert(serde_json::to_string(&chosen.plan.applied_sections).unwrap());
    }
    assert_eq!(variants.len(), 2);
}

#[test]
fn incompatible_choices_have_distinct_bounded_outcomes() {
    let score = source();
    let mut req = request();
    req.sections[1].allowed_accompaniments = vec![Some(AccordionPattern::WaltzBassChord)];
    let none = shuffle_accordion_plan(&score, req.clone());
    assert_eq!(none.plan.diagnostics[0].code, "no-candidate");
    assert_eq!(none.rejected.get("unsupported-meter"), Some(&1));
    assert_eq!(none.attempted, 1);
    assert!(none.plan.score.is_none());
    assert!(none.plan.resolved_config.is_none());
    assert!(none.plan.generated_notes.is_empty());

    req.sections[1]
        .allowed_accompaniments
        .push(Some(AccordionPattern::Arpeggio));
    req.max_attempts = 1;
    let limited = shuffle_accordion_plan(&score, req);
    assert_eq!(limited.plan.diagnostics[0].code, "search-limit");
    assert_eq!(limited.attempted, 1);
    assert_eq!(limited.rejected.values().sum::<usize>(), 1);
    assert_eq!(limited.pool_size, 2);
}

#[test]
fn invalid_inputs_are_rejected_before_search() {
    let score = source();
    let mut req = request();
    req.seed = "-1".into();
    let bad_seed = shuffle_accordion_plan(&score, req.clone());
    assert_eq!(bad_seed.plan.diagnostics[0].code, "unsupported");
    assert_eq!(bad_seed.attempted, 0);
    req.seed = "0".into();
    req.max_attempts = 0;
    assert_eq!(
        shuffle_accordion_plan(&score, req.clone()).plan.diagnostics[0].code,
        "unsupported"
    );
    req.max_attempts = 64;
    req.sections[1].start_measure = 3;
    let invalid_range = shuffle_accordion_plan(&score, req.clone());
    assert_eq!(invalid_range.plan.diagnostics[0].code, "unsupported");
    assert_eq!(invalid_range.attempted, 0);
    req.sections[1].start_measure = 4;
    for section in &mut req.sections {
        section.allowed_accompaniments = vec![
            None,
            Some(AccordionPattern::HeldBassChord),
            Some(AccordionPattern::BassChordPulse),
            Some(AccordionPattern::DrivingBassChord),
            Some(AccordionPattern::OffbeatChords),
        ];
        section.allowed_melodic_voices = vec![
            AccordionMelodicVoice::Unchanged,
            AccordionMelodicVoice::ThirdBelow,
            AccordionMelodicVoice::SixthBelow,
            AccordionMelodicVoice::OctaveBelow,
        ];
    }
    req.sections[1].end_measure = 6;
    let mut extra = req.sections[1].clone();
    extra.start_measure = 6;
    extra.end_measure = 7;
    req.sections.insert(2, extra);
    let over_pool = shuffle_accordion_plan(&score, req);
    assert_eq!(over_pool.plan.diagnostics[0].code, "unsupported");
    assert_eq!(over_pool.attempted, 0);
}

#[test]
fn tie_cut_and_tempo_rejections_do_not_force_fallback_modes() {
    let mut score = source();
    score.parts[0].staves[0].measures[3].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[4].voices[0][0].tie_end = true;
    let mut req = request();
    let no_safe_layer = shuffle_accordion_plan(&score, req.clone());
    assert_eq!(no_safe_layer.plan.diagnostics[0].code, "no-candidate");
    assert_eq!(no_safe_layer.rejected.get("unsafe-tie-boundary"), Some(&1));
    req.sections[1].allowed_melodic_voices = vec![AccordionMelodicVoice::Unchanged];
    let safe = shuffle_accordion_plan(&score, req.clone());
    assert!(safe.plan.score.is_some(), "{:?}", safe.plan.diagnostics);
    assert_eq!(
        safe.plan.applied_sections[1].melodic_voice,
        AccordionMelodicVoice::Unchanged
    );
    score.parts[0].staves[0].measures[3].voices[0][0].tie_start = false;
    score.parts[0].staves[0].measures[4].voices[0][0].tie_end = false;
    score.settings.tempo_bpm = 240;
    req.sections[0].allowed_accompaniments = vec![Some(AccordionPattern::HeldBassChord)];
    req.sections[1].allowed_accompaniments = vec![
        Some(AccordionPattern::HeldBassChord),
        Some(AccordionPattern::DrivingBassChord),
    ];
    req.sections[2].allowed_accompaniments = vec![Some(AccordionPattern::HeldBassChord)];
    let tempo = shuffle_accordion_plan(&score, req);
    assert!(tempo.plan.score.is_some(), "{:?}", tempo.plan.diagnostics);
    assert_eq!(
        tempo.plan.applied_sections[1].accompaniment,
        Some(AccordionPattern::HeldBassChord)
    );
    assert_eq!(
        tempo
            .plan
            .resolved_config
            .as_ref()
            .unwrap()
            .options
            .bass_mode,
        AccordionBassMode::StandardBass
    );
}

#[test]
fn pickup_and_tuplet_boundary_keep_written_measure_context() {
    let mut score = source();
    score.parts[0].staves[0].measures[0].actual_length = MeasureLength::from_beats(0.5);
    score.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Eighth;
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
    let mut req = request();
    req.sections[0].allowed_melodic_voices = vec![AccordionMelodicVoice::Unchanged];
    req.sections[1].allowed_melodic_voices = vec![
        AccordionMelodicVoice::Unchanged,
        AccordionMelodicVoice::ThirdBelow,
    ];
    let selected = shuffle_accordion_plan(&score, req);
    assert!(
        selected.plan.score.is_some(),
        "{:?}",
        selected.plan.diagnostics
    );
    assert_eq!(
        selected.plan.applied_sections[1].melodic_voice,
        AccordionMelodicVoice::Unchanged
    );
    assert_eq!(
        selected
            .plan
            .score
            .as_ref()
            .unwrap()
            .playback_measure_beats(0),
        0.5
    );
    assert_eq!(
        serde_json::to_value(
            &selected.plan.score.as_ref().unwrap().parts[0].staves[0].measures[4].voices[0]
        )
        .unwrap(),
        serde_json::to_value(&score.parts[0].staves[0].measures[4].voices[0]).unwrap()
    );
}

#[test]
fn mixed_meter_allows_waltz_only_in_three_four_section() {
    let mut score = source();
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
    let mut req = request();
    req.sections[0].allowed_accompaniments = vec![
        Some(AccordionPattern::WaltzBassChord),
        Some(AccordionPattern::HeldBassChord),
    ];
    req.max_attempts = 2;
    let mut saw_waltz = false;
    for seed in 0..16 {
        req.seed = seed.to_string();
        let result = shuffle_accordion_plan(&score, req.clone());
        let output = result
            .plan
            .score
            .as_ref()
            .expect("compatible mixed-meter plan");
        assert_eq!(output.playback_measure_beats(0), 3.0);
        assert_eq!(output.playback_measure_beats(4), 4.0);
        if result.plan.applied_sections[0].accompaniment == Some(AccordionPattern::WaltzBassChord) {
            saw_waltz = true;
        }
    }
    assert!(saw_waltz);
}
