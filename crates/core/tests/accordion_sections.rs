use acorde_core::{
    AccordionAccompanimentSection as Section, AccordionBassMode, AccordionMelodicSchedule,
    AccordionMelodicSection, AccordionMelodicVoice, AccordionPattern as Pattern, Barline, Duration,
    GuidedAccordionOptions, MeasureLength, Note, Pitch, PositionedAccordionChord, Score,
    SectionedAccordionOptions, arrange_sections_with_melodic_voices, arrange_with_explicit_chords,
    arrange_with_sections,
};

fn fixture(count: u32, numerator: u8) -> (Score, SectionedAccordionOptions) {
    let mut score = Score::new(
        "original sectional accompaniment",
        120,
        numerator,
        4,
        0,
        count,
    );
    for bar in &mut score.parts[0].staves[0].measures {
        let mut note = Note::new(
            Pitch::from_midi(72, false),
            if numerator == 4 {
                Duration::Whole
            } else {
                Duration::Half
            },
        );
        note.dot_count = u8::from(numerator == 3);
        bar.voices[0] = vec![note];
    }
    (
        score,
        SectionedAccordionOptions {
            chords: vec![placed(0, 0.0, "C")],
            bass_mode: AccordionBassMode::FreeBass,
            default_pattern: Some(Pattern::HeldBassChord),
            sections: Vec::new(),
        },
    )
}

fn placed(measure_index: usize, offset_beats: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn section(first: usize, last: usize, pattern: Option<Pattern>) -> Section {
    Section {
        first_measure: first,
        last_measure: last,
        pattern,
    }
}

fn melodic_section(
    first: usize,
    last: usize,
    voice: AccordionMelodicVoice,
) -> AccordionMelodicSection {
    AccordionMelodicSection {
        first_measure: first,
        last_measure: last,
        voice,
    }
}

#[test]
fn independent_melodic_sections_keep_the_melody_and_change_right_hand_texture() {
    let (mut score, mut accompaniment) = fixture(4, 4);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0][0].pitches[0] = Pitch::from_midi(76, false);
    }
    accompaniment.sections = vec![
        section(1, 1, Some(Pattern::BassChordPulse)),
        section(3, 3, None),
    ];
    let original = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
    let result = arrange_sections_with_melodic_voices(
        &score,
        accompaniment,
        AccordionMelodicSchedule {
            default_voice: AccordionMelodicVoice::Unchanged,
            sections: vec![
                melodic_section(3, 3, AccordionMelodicVoice::OctaveBelow),
                melodic_section(1, 1, AccordionMelodicVoice::ThirdBelow),
                melodic_section(2, 2, AccordionMelodicVoice::SixthBelow),
            ],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(
        result.resolved_melodic_schedule.sections[0].first_measure,
        1
    );
    assert_eq!(
        result.resolved_melodic_voices,
        vec![
            AccordionMelodicVoice::Unchanged,
            AccordionMelodicVoice::ThirdBelow,
            AccordionMelodicVoice::SixthBelow,
            AccordionMelodicVoice::OctaveBelow,
        ]
    );
    let output = result.arrangement.score.as_ref().unwrap();
    for (index, bar) in output.parts[0].staves[0].measures.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&bar.voices[0]).unwrap(),
            original["measures"][index]["voices"][0]
        );
    }
    let harmony = output.parts[0].staves[0]
        .measures
        .iter()
        .map(|bar| {
            bar.voices[1]
                .first()
                .and_then(|note| (!note.is_rest).then(|| note.pitches[0].to_midi()))
        })
        .collect::<Vec<_>>();
    assert_eq!(harmony, vec![None, Some(72), Some(67), Some(64)]);
    assert_eq!(
        result
            .arrangement
            .generated_notes
            .iter()
            .filter(|address| address.staff == 0)
            .count(),
        3
    );
    assert!(
        output.parts[0].staves[1].measures[3].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert_eq!(
        serde_json::to_value(&score.parts[0].staves[0]).unwrap(),
        original
    );
}

#[test]
fn invalid_melodic_sections_and_incompatible_harmony_return_no_partial_score() {
    let (score, accompaniment) = fixture(2, 4);
    for schedule in [
        AccordionMelodicSchedule {
            default_voice: AccordionMelodicVoice::Unchanged,
            sections: vec![melodic_section(1, 2, AccordionMelodicVoice::ThirdBelow)],
        },
        AccordionMelodicSchedule {
            default_voice: AccordionMelodicVoice::Unchanged,
            sections: vec![
                melodic_section(0, 1, AccordionMelodicVoice::ThirdBelow),
                melodic_section(1, 1, AccordionMelodicVoice::SixthBelow),
            ],
        },
        AccordionMelodicSchedule {
            default_voice: AccordionMelodicVoice::ThirdBelow,
            sections: vec![],
        },
    ] {
        let result = arrange_sections_with_melodic_voices(&score, accompaniment.clone(), schedule);
        assert!(result.arrangement.score.is_none());
        assert!(result.arrangement.generated_notes.is_empty());
        assert!(result.arrangement.source_notes.is_empty());
        assert!(result.resolved_melodic_voices.is_empty());
        assert!(!result.arrangement.diagnostics.is_empty());
    }
}

#[test]
fn octave_tie_stops_at_a_melodic_section_boundary() {
    let (mut score, accompaniment) = fixture(2, 4);
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[1].voices[0][0].tie_end = true;
    let result = arrange_sections_with_melodic_voices(
        &score,
        accompaniment,
        AccordionMelodicSchedule {
            default_voice: AccordionMelodicVoice::Unchanged,
            sections: vec![melodic_section(0, 0, AccordionMelodicVoice::OctaveBelow)],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert!(!output.parts[0].staves[0].measures[0].voices[1][0].tie_start);
    assert!(output.parts[0].staves[0].measures[1].voices[1][0].is_rest);
    assert!(output.parts[0].staves[0].measures[0].voices[0][0].tie_start);
}

fn melody_events(score: &Score) -> Vec<(u8, f64, f64)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| event.source.as_ref().is_some_and(|addr| addr.staff == 0))
        .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
        .collect()
}

fn without_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("id");
            map.values_mut().for_each(without_ids);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(without_ids),
        _ => {}
    }
}

#[test]
fn held_pulse_swing_and_silence_share_one_unchanged_melody() {
    let (score, mut options) = fixture(4, 4);
    options.chords = vec![
        placed(0, 0.0, "C"),
        placed(1, 0.0, "F"),
        placed(2, 0.0, "G"),
        placed(3, 0.0, "C"),
    ];
    options.sections = vec![
        section(3, 3, None),
        section(1, 1, Some(Pattern::BassChordPulse)),
        section(2, 2, Some(Pattern::SwingAccompaniment)),
    ];
    let before = serde_json::to_value(&score).unwrap();
    let result = arrange_with_sections(&score, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.resolved_patterns,
        vec![
            Some(Pattern::HeldBassChord),
            Some(Pattern::BassChordPulse),
            Some(Pattern::SwingAccompaniment),
            None
        ]
    );
    assert!(result.notated_swing);
    assert_eq!(result.notated_swing_measures, vec![2]);
    assert_eq!(result.resolved_options.sections[0].first_measure, 1);
    let output = result.score.unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
        before["parts"][0]["staves"][0]
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(melody_events(&score), melody_events(&output));
    let bars = &output.parts[0].staves[1].measures;
    assert_eq!(
        bars.iter()
            .map(|bar| bar.voices[0].len())
            .collect::<Vec<_>>(),
        vec![1, 4, 8, 1]
    );
    assert!(bars[3].voices[0].iter().all(|note| note.is_rest));
    assert!(
        bars.iter()
            .all(|bar| (bar.voices[0].iter().map(Note::beats).sum::<f64>() - 4.0).abs() < 1e-9)
    );
    assert_eq!(result.source_notes.len(), 4);
    assert_eq!(result.generated_notes.len(), 13);
    for addr in result.generated_notes {
        assert!(addr.measure < 3);
        assert!(!bars[addr.measure].voices[addr.voice][addr.note].is_rest);
    }
}

#[test]
fn constant_schedule_matches_existing_patterns_in_notation_and_playback() {
    for (pattern, meter) in [
        (Pattern::HeldBassChord, 4),
        (Pattern::BassChordPulse, 4),
        (Pattern::AlternatingBassChord, 4),
        (Pattern::Arpeggio, 4),
        (Pattern::RisingArpeggio, 4),
        (Pattern::AlbertiBass, 4),
        (Pattern::OffbeatChords, 4),
        (Pattern::SwingAccompaniment, 4),
        (Pattern::WalkingBass, 4),
        (Pattern::WaltzBassChord, 3),
        (Pattern::WaltzAlternatingBassChord, 3),
        (Pattern::FineBassChord, 2),
        (Pattern::Habanera, 2),
        (Pattern::DrivingBassChord, 4),
        (Pattern::DrivingBassChordLight, 4),
    ] {
        let (score, mut options) = fixture(3, meter);
        options.default_pattern = Some(pattern);
        options.chords.push(placed(1, 0.0, "F"));
        let old = arrange_with_explicit_chords(
            &score,
            GuidedAccordionOptions {
                pattern,
                chords: options.chords.clone(),
            },
        );
        let new = arrange_with_sections(&score, options);
        assert!(
            new.diagnostics.is_empty(),
            "{pattern:?}: {:?}",
            new.diagnostics
        );
        let mut old = serde_json::to_value(old.score.unwrap()).unwrap();
        let mut new = serde_json::to_value(new.score.unwrap()).unwrap();
        without_ids(&mut old);
        without_ids(&mut new);
        assert_eq!(old, new, "{pattern:?}");
    }
}

#[test]
fn patterns_validate_only_their_active_meter() {
    let (mut score, mut options) = fixture(3, 4);
    let bars = &mut score.parts[0].staves[0].measures;
    bars[1].time_sig = Some(acorde_core::TimeSignature {
        numerator: 3,
        denominator: 4,
    });
    bars[1].voices[0][0].duration = Duration::Half;
    bars[1].voices[0][0].dot_count = 1;
    bars[2].time_sig = Some(acorde_core::TimeSignature {
        numerator: 2,
        denominator: 4,
    });
    bars[2].voices[0][0].duration = Duration::Half;
    options.bass_mode = AccordionBassMode::StandardBass;
    options.sections = vec![
        section(0, 0, Some(Pattern::DrivingBassChord)),
        section(1, 1, Some(Pattern::WaltzBassChord)),
        section(2, 2, Some(Pattern::Habanera)),
    ];
    let result = arrange_with_sections(&score, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(
        output.parts[0].staves[1]
            .measures
            .iter()
            .map(|bar| bar.voices[0].iter().map(Note::beats).sum::<f64>())
            .collect::<Vec<_>>(),
        vec![4.0, 3.0, 2.0]
    );
    assert_eq!(
        output.parts[0].staves[1].measures[2].voices[0]
            .iter()
            .map(Note::beats)
            .collect::<Vec<_>>(),
        vec![0.75, 0.25, 0.5, 0.5]
    );
    assert_eq!(melody_events(&score), melody_events(&output));
}

#[test]
fn harmony_changes_during_silence_carry_into_the_resumed_pattern() {
    let (score, mut options) = fixture(3, 4);
    options.default_pattern = Some(Pattern::BassChordPulse);
    options.chords.push(placed(1, 2.0, "F"));
    options.sections = vec![section(1, 1, None)];
    let result = arrange_with_sections(&score, options);
    let output = result.score.unwrap();
    let left = &output.parts[0].staves[1].measures;
    assert!(
        left[1].voices[0]
            .iter()
            .all(|note| note.is_rest && !note.tie_start && !note.tie_end)
    );
    assert_eq!(left[2].voices[0][0].pitches[0].to_midi(), 41);
    assert_eq!(
        left[2].voices[0][1]
            .pitches
            .iter()
            .map(Pitch::to_midi)
            .collect::<Vec<_>>(),
        vec![65, 69, 72]
    );
}

#[test]
fn pickup_keeps_phase_and_new_waltz_runs_start_on_root() {
    let (mut score, mut options) = fixture(4, 3);
    score.parts[0].staves[0].measures[0].actual_length = MeasureLength::from_beats(1.0);
    score.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
    score.parts[0].staves[0].measures[0].voices[0][0].dot_count = 0;
    options.sections = vec![
        section(0, 0, Some(Pattern::WaltzAlternatingBassChord)),
        section(2, 2, Some(Pattern::WaltzAlternatingBassChord)),
        section(3, 3, Some(Pattern::WaltzAlternatingBassChord)),
    ];
    let result = arrange_with_sections(&score, options);
    let output = result.score.unwrap();
    let left = &output.parts[0].staves[1].measures;
    assert_eq!(left[0].voices[0][0].pitches.len(), 3);
    assert_eq!(left[2].voices[0][0].pitches[0].to_midi(), 36);
    assert_eq!(left[3].voices[0][0].pitches[0].to_midi(), 43);
}

#[test]
fn written_sections_survive_repeats_and_alternate_endings() {
    let (mut score, mut options) = fixture(3, 4);
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[1].barline_right = Barline::RepeatEnd;
    bars[1].volta = Some(acorde_core::VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    bars[2].volta = Some(acorde_core::VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    options.default_pattern = Some(Pattern::BassChordPulse);
    options.sections = vec![
        section(1, 1, Some(Pattern::HeldBassChord)),
        section(2, 2, None),
    ];
    let result = arrange_with_sections(&score, options);
    let output = result.score.unwrap();
    assert_eq!(acorde_core::measure_sequence(&output), vec![0, 1, 0, 2]);
    assert_eq!(melody_events(&score), melody_events(&output));
    assert!(
        acorde_core::to_playback_events(&output, &Default::default())
            .iter()
            .filter(|event| event.source.as_ref().is_some_and(|addr| addr.staff == 1))
            .all(|event| event.time_beats < 12.0)
    );
}

#[test]
fn invalid_sections_or_patterns_return_no_partial_arrangement() {
    let (score, options) = fixture(3, 4);
    for (sections, code, measure) in [
        (vec![section(2, 1, None)], "section-range", Some(2)),
        (vec![section(0, 3, None)], "section-range", Some(0)),
        (
            vec![
                section(0, 1, None),
                section(1, 2, Some(Pattern::BassChordPulse)),
            ],
            "overlapping-sections",
            Some(1),
        ),
        (
            vec![section(1, 1, Some(Pattern::WaltzBassChord))],
            "unsupported-meter",
            Some(1),
        ),
    ] {
        let mut options = options.clone();
        options.sections = sections;
        let result = arrange_with_sections(&score, options);
        assert!(result.score.is_none());
        assert!(
            result.source_notes.is_empty()
                && result.generated_notes.is_empty()
                && result.resolved_patterns.is_empty()
        );
        assert!(!result.notated_swing && result.notated_swing_measures.is_empty());
        assert_eq!(result.diagnostics[0].code, code);
        assert_eq!(result.diagnostics[0].measure_index, measure);
    }
}

#[test]
fn standard_bass_rate_checks_use_active_patterns_and_inherited_tempo() {
    let (mut score, mut options) = fixture(2, 4);
    options.bass_mode = AccordionBassMode::StandardBass;
    options.sections = vec![section(1, 1, Some(Pattern::DrivingBassChord))];
    score.settings.tempo_bpm = 240;
    score.parts[0].staves[0].measures[1].tempo = Some(120);
    assert!(
        arrange_with_sections(&score, options.clone())
            .score
            .is_some()
    );
    score.parts[0].staves[0].measures[1].tempo = None;
    let rejected = arrange_with_sections(&score, options.clone());
    assert_eq!(
        rejected.diagnostics[0].code,
        "unsupported-standard-bass-rate"
    );
    assert_eq!(rejected.diagnostics[0].measure_index, Some(1));
    options.sections[0].pattern = Some(Pattern::Arpeggio);
    assert_eq!(
        arrange_with_sections(&score, options).diagnostics[0].code,
        "unsupported-standard-bass"
    );
}

#[test]
fn silence_and_generation_limits_have_explicit_contracts() {
    let (mut score, mut options) = fixture(2, 4);
    options.default_pattern = None;
    let muted = arrange_with_sections(&score, options.clone());
    assert!(muted.score.is_some());
    assert!(muted.generated_notes.is_empty());
    assert_eq!(muted.resolved_patterns, vec![None, None]);
    score.parts[0].staves[0].measures[0].actual_length = MeasureLength::from_beats(64.0);
    assert_eq!(
        arrange_with_sections(&score, options.clone()).diagnostics[0].code,
        "section-input-limit"
    );
    let large = Score::new("bounded", 120, 4, 4, 0, 513);
    assert_eq!(
        arrange_with_sections(&large, options).diagnostics[0].code,
        "section-input-limit"
    );
}
