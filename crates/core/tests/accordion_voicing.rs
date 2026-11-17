use acorde_core::{
    AccordionAccompanimentSection, AccordionBassMode, AccordionMelodicVoice, AccordionPattern,
    AccordionSwingPolicy, AccordionVoicingOptions, Barline, Duration, LayeredAccordionOptions,
    Note, Pitch, PositionedAccordionChord, Score, SectionedAccordionOptions,
    arrange_sections_with_voice_leading, arrange_with_layers, arrange_with_sections,
    arrange_with_voice_leading,
};

fn fixture(chords: &[(&str, &str)], pattern: AccordionPattern) -> (Score, LayeredAccordionOptions) {
    let numerator = match pattern {
        AccordionPattern::WaltzBassChord | AccordionPattern::WaltzAlternatingBassChord => 3,
        AccordionPattern::FineBassChord | AccordionPattern::Habanera => 2,
        _ => 4,
    };
    let mut score = Score::new(
        "original voicing diagnostic",
        120,
        numerator,
        4,
        0,
        chords.len() as u32,
    );
    for bar in &mut score.parts[0].staves[0].measures {
        let mut note = Note::new(
            Pitch::from_midi(84, false),
            if numerator == 4 {
                Duration::Whole
            } else {
                Duration::Half
            },
        );
        note.dot_count = u8::from(numerator == 3);
        bar.voices[0] = vec![note];
    }
    let layers = LayeredAccordionOptions {
        chords: chords
            .iter()
            .enumerate()
            .map(|(measure, (root, kind))| PositionedAccordionChord {
                measure_index: measure,
                offset_beats: 0.0,
                chord: serde_json::from_value(serde_json::json!({"root":root,"kind":kind}))
                    .unwrap(),
            })
            .collect(),
        alternative_chords: None,
        inversion: None,
        accompaniment: Some(pattern),
        melodic_voice: AccordionMelodicVoice::Unchanged,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::FreeBass,
    };
    (score, layers)
}

fn midi(note: &Note) -> Vec<i16> {
    note.pitches.iter().map(Pitch::to_midi).collect()
}

fn distance(a: &[i16], b: &[i16]) -> u64 {
    a.iter()
        .map(|p| b.iter().map(|q| p.abs_diff(*q) as u64).min().unwrap())
        .sum::<u64>()
        + b.iter()
            .map(|p| a.iter().map(|q| p.abs_diff(*q) as u64).min().unwrap())
            .sum::<u64>()
}

fn playback_chords(score: &Score) -> Vec<Vec<i16>> {
    acorde_core::measure_sequence(score)
        .iter()
        .flat_map(|&measure| {
            score.parts[0].staves[1].measures[measure].voices[0]
                .iter()
                .filter(|note| !note.tie_end && note.pitches.len() > 1)
                .map(|note| {
                    midi(note)
                        .into_iter()
                        .filter(|pitch| *pitch >= 48)
                        .collect()
                })
        })
        .collect()
}

fn movement(score: &Score) -> u64 {
    playback_chords(score)
        .windows(2)
        .map(|pair| distance(&pair[0], &pair[1]))
        .sum()
}

#[test]
fn common_progression_moves_less_without_changing_harmony_or_melody() {
    let (source, layers) = fixture(
        &[
            ("C", "major"),
            ("F", "major"),
            ("G", "major"),
            ("C", "major"),
        ],
        AccordionPattern::HeldBassChord,
    );
    let before = serde_json::to_value(&source).unwrap();
    let original = arrange_with_layers(&source, layers.clone()).score.unwrap();
    let result = arrange_with_voice_leading(&source, layers.clone(), Default::default());
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let arranged = result.arrangement.score.as_ref().unwrap();
    assert_eq!(result.original_motion_cost, Some(movement(&original)));
    assert_eq!(result.motion_cost, Some(movement(arranged)));
    assert!(result.motion_cost < result.original_motion_cost);
    assert_eq!(
        serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
        before["parts"][0]["staves"][0]
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(result.arrangement.resolved_chords, layers.chords);
    for (old, new) in original.parts[0].staves[1]
        .measures
        .iter()
        .zip(&arranged.parts[0].staves[1].measures)
    {
        assert_eq!(old.voices[0][0].pitches[0], new.voices[0][0].pitches[0]);
        let pcs = |note: &Note| {
            let mut pcs: Vec<_> = note.pitches.iter().map(|p| p.to_midi() % 12).collect();
            pcs.sort();
            pcs
        };
        assert_eq!(pcs(&old.voices[0][0]), pcs(&new.voices[0][0]));
    }
    let again = arrange_with_voice_leading(&source, layers, Default::default());
    assert_eq!(result.voicings, again.voicings);
}

#[test]
fn repeat_back_edges_and_tied_pickup_fragments_are_counted() {
    let (mut score, layers) = fixture(
        &[("C", "major"), ("G", "dominant")],
        AccordionPattern::HeldBassChord,
    );
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].actual_length = acorde_core::MeasureLength::from_beats(3.5);
    bars[0].voices[0][0].duration = Duration::Half;
    bars[0].voices[0][0].dot_count = 1;
    bars[0].voices[0].push(Note::new(Pitch::from_midi(84, false), Duration::Eighth));
    bars[0].barline_left = Barline::RepeatStart;
    bars[1].barline_right = Barline::RepeatEnd;
    let original = arrange_with_layers(&score, layers.clone()).score.unwrap();
    let result = arrange_with_voice_leading(&score, layers, Default::default());
    let output = result.arrangement.score.unwrap();
    assert_eq!(acorde_core::measure_sequence(&output), vec![0, 1, 0, 1]);
    assert_eq!(result.original_motion_cost, Some(movement(&original)));
    assert_eq!(result.motion_cost, Some(movement(&output)));
    assert!(result.motion_cost <= result.original_motion_cost);
    let tied = &output.parts[0].staves[1].measures[0].voices[0];
    assert!(tied.len() > 1);
    assert!(
        tied.windows(2)
            .all(|pair| pair[0].tie_start && pair[1].tie_end && pair[0].pitches == pair[1].pitches)
    );
}

#[test]
fn block_patterns_keep_bass_notes_rhythm_rests_and_articulations() {
    for pattern in [
        AccordionPattern::BassChordPulse,
        AccordionPattern::AlternatingBassChord,
        AccordionPattern::OffbeatChords,
        AccordionPattern::SwingAccompaniment,
        AccordionPattern::WaltzBassChord,
        AccordionPattern::WaltzAlternatingBassChord,
        AccordionPattern::FineBassChord,
        AccordionPattern::Habanera,
        AccordionPattern::DrivingBassChord,
        AccordionPattern::DrivingBassChordLight,
    ] {
        let (score, layers) = fixture(
            &[("C", "major-seventh"), ("F", "major"), ("G", "dominant")],
            pattern,
        );
        let original = arrange_with_layers(&score, layers.clone()).score.unwrap();
        let result = arrange_with_voice_leading(&score, layers, Default::default());
        assert!(
            result.arrangement.diagnostics.is_empty(),
            "{pattern:?}: {:?}",
            result.arrangement.diagnostics
        );
        let output = result.arrangement.score.unwrap();
        for (old, new) in original.parts[0].staves[1]
            .measures
            .iter()
            .zip(&output.parts[0].staves[1].measures)
        {
            assert_eq!(old.voices[0].len(), new.voices[0].len());
            for (old, new) in old.voices[0].iter().zip(&new.voices[0]) {
                if old.pitches.len() <= 1 {
                    assert_eq!(old.pitches, new.pitches);
                }
                let strip = |note: &Note| {
                    let mut value = serde_json::to_value(note).unwrap();
                    value.as_object_mut().unwrap().remove("id");
                    value.as_object_mut().unwrap().remove("pitches");
                    value
                };
                assert_eq!(strip(old), strip(new));
            }
        }
        assert!(result.motion_cost <= result.original_motion_cost);
    }
}

#[test]
fn explicit_octave_range_preserves_all_triad_and_seventh_tones() {
    let (score, layers) = fixture(
        &[
            ("D", "minor-seventh"),
            ("G", "dominant"),
            ("C", "major-seventh"),
        ],
        AccordionPattern::HeldBassChord,
    );
    let opts = AccordionVoicingOptions {
        min_midi: 48,
        max_midi: 59,
        max_span_semitones: 11,
        ..Default::default()
    };
    let result = arrange_with_voice_leading(&score, layers, opts);
    assert!(
        result.arrangement.score.is_some(),
        "{:?}",
        result.arrangement.diagnostics
    );
    for group in result.voicings {
        assert_eq!(group.chord_midi.len(), group.original_chord_midi.len());
        assert!(
            group
                .chord_midi
                .iter()
                .all(|pitch| (48..=59).contains(pitch))
        );
        let mut actual: Vec<_> = group.chord_midi.iter().map(|pitch| pitch % 12).collect();
        actual.sort();
        let mut original: Vec<_> = group
            .original_chord_midi
            .iter()
            .map(|pitch| pitch % 12)
            .collect();
        original.sort();
        assert_eq!(actual, original);
    }
}

#[test]
fn invalid_or_impossible_requests_never_return_partial_scores_or_mappings() {
    let (score, layers) = fixture(&[("C", "major")], AccordionPattern::HeldBassChord);
    for options in [
        AccordionVoicingOptions {
            min_midi: 60,
            max_midi: 60,
            ..Default::default()
        },
        AccordionVoicingOptions {
            min_midi: 85,
            ..Default::default()
        },
        AccordionVoicingOptions {
            max_span_semitones: 25,
            ..Default::default()
        },
    ] {
        let result = arrange_with_voice_leading(&score, layers.clone(), options);
        assert!(result.arrangement.score.is_none());
        assert!(!result.arrangement.diagnostics.is_empty());
        assert!(result.arrangement.generated_notes.is_empty());
        assert!(result.arrangement.source_notes.is_empty());
        assert!(result.voicings.is_empty());
        assert!(result.motion_cost.is_none());
    }
    for pattern in [
        AccordionPattern::WalkingBass,
        AccordionPattern::Arpeggio,
        AccordionPattern::RisingArpeggio,
        AccordionPattern::AlbertiBass,
    ] {
        let mut options = layers.clone();
        options.accompaniment = Some(pattern);
        assert_eq!(
            arrange_with_voice_leading(&score, options, Default::default())
                .arrangement
                .diagnostics[0]
                .code,
            "unsupported-voicing-pattern"
        );
    }
    let mut options = layers;
    options.bass_mode = AccordionBassMode::StandardBass;
    assert_eq!(
        arrange_with_voice_leading(&score, options, Default::default())
            .arrangement
            .diagnostics[0]
            .code,
        "voicing-requires-free-bass"
    );
}

#[test]
fn standard_entry_points_remain_unchanged_and_redundant_chords_do_not_revoice() {
    let (score, mut layers) = fixture(
        &[("C", "major"), ("F", "major")],
        AccordionPattern::HeldBassChord,
    );
    let before = arrange_with_voice_leading(&score, layers.clone(), Default::default());
    let mut repeated = layers.chords[0].clone();
    repeated.offset_beats = 2.0;
    layers.chords.insert(1, repeated);
    let after = arrange_with_voice_leading(&score, layers.clone(), Default::default());
    assert_eq!(before.voicings, after.voicings);
    assert_eq!(before.motion_cost, after.motion_cost);
    let old = arrange_with_layers(&score, layers).score.unwrap();
    assert_eq!(
        midi(&old.parts[0].staves[1].measures[0].voices[0][0]),
        vec![36, 60, 64, 67]
    );
}

#[test]
fn input_bounds_reject_before_large_accompaniment_generation() {
    let (mut score, layers) = fixture(&[("C", "major")], AccordionPattern::OffbeatChords);
    score.parts[0].staves[0].measures[0].actual_length =
        acorde_core::MeasureLength::from_beats(64.0);
    let too_long = arrange_with_voice_leading(&score, layers.clone(), Default::default());
    assert!(too_long.arrangement.score.is_none());
    assert_eq!(
        too_long.arrangement.diagnostics[0].code,
        "voicing-input-limit"
    );
    let too_many = Score::new("bounded input", 120, 4, 4, 0, 513);
    assert_eq!(
        arrange_with_voice_leading(&too_many, layers, Default::default())
            .arrangement
            .diagnostics[0]
            .code,
        "voicing-input-limit"
    );
}

#[test]
fn slash_bass_and_added_melody_voice_are_preserved() {
    let (score, mut layers) = fixture(
        &[("C", "major"), ("F", "major")],
        AccordionPattern::HeldBassChord,
    );
    layers.chords[0].chord.bass = Some("E".into());
    layers.melodic_voice = AccordionMelodicVoice::OctaveBelow;
    let base = arrange_with_layers(&score, layers.clone()).score.unwrap();
    let result = arrange_with_voice_leading(&score, layers, Default::default());
    let output = result.arrangement.score.unwrap();
    assert_eq!(
        output.parts[0].staves[1].measures[0].voices[0][0].pitches[0].to_midi(),
        40
    );
    let events = |score: &Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .filter(|event| event.source.as_ref().is_some_and(|addr| addr.staff == 0))
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&base), events(&output));
}

fn sections(layers: &LayeredAccordionOptions) -> SectionedAccordionOptions {
    SectionedAccordionOptions {
        chords: layers.chords.clone(),
        bass_mode: layers.bass_mode,
        default_pattern: layers.accompaniment,
        sections: Vec::new(),
    }
}

fn at(measure: usize, pattern: Option<AccordionPattern>) -> AccordionAccompanimentSection {
    AccordionAccompanimentSection {
        first_measure: measure,
        last_measure: measure,
        pattern,
    }
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
fn section_voicing_leaves_all_melodic_patterns_silence_and_bass_unchanged() {
    use AccordionPattern::*;
    let (score, layers) = fixture(
        &[
            ("C", "major"),
            ("F", "major"),
            ("G", "major"),
            ("C", "major"),
            ("F", "major"),
            ("D", "minor"),
            ("G", "dominant"),
            ("C", "major"),
        ],
        HeldBassChord,
    );
    let source_before = serde_json::to_value(&score).unwrap();
    let mut options = sections(&layers);
    options.chords[0].chord.bass = Some("E".into());
    options.sections = vec![
        at(1, Some(BassChordPulse)),
        at(2, None),
        at(3, Some(Arpeggio)),
        at(4, Some(RisingArpeggio)),
        at(5, Some(AlbertiBass)),
        at(6, Some(WalkingBass)),
        at(7, Some(SwingAccompaniment)),
    ];
    let original = arrange_with_sections(&score, options.clone());
    let result = arrange_sections_with_voice_leading(&score, options.clone(), Default::default());
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.as_ref().unwrap();
    let base = original.score.unwrap();
    assert_eq!(result.arrangement.resolved_options, options);
    assert_eq!(
        result.arrangement.resolved_patterns,
        original.resolved_patterns
    );
    assert_eq!(result.arrangement.generated_notes, original.generated_notes);
    assert_eq!(result.arrangement.notated_swing_measures, vec![7]);
    assert!(result.motion_cost < result.original_motion_cost);
    assert_eq!(result.original_motion_cost, Some(movement(&base)));
    assert_eq!(result.motion_cost, Some(movement(output)));
    assert_eq!(serde_json::to_value(&score).unwrap(), source_before);
    assert_eq!(
        serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
        source_before["parts"][0]["staves"][0]
    );
    assert_eq!(
        output.parts[0].staves[1].measures[0].voices[0][0].pitches[0].to_midi(),
        40
    );
    for group in &result.voicings {
        assert!(
            group
                .targets
                .iter()
                .all(|addr| [0, 1, 7].contains(&addr.measure))
        );
    }
    for (measure, (old, new)) in base.parts[0].staves[1]
        .measures
        .iter()
        .zip(&output.parts[0].staves[1].measures)
        .enumerate()
    {
        let mut old_value = serde_json::to_value(old).unwrap();
        let mut new_value = serde_json::to_value(new).unwrap();
        without_ids(&mut old_value);
        without_ids(&mut new_value);
        if (2..=6).contains(&measure) {
            assert_eq!(old_value, new_value, "untouched section {measure}");
        } else {
            for (old, new) in old.voices[0].iter().zip(&new.voices[0]) {
                let bass = |note: &Note| {
                    note.pitches
                        .iter()
                        .filter(|p| p.to_midi() < 48)
                        .cloned()
                        .collect::<Vec<_>>()
                };
                assert_eq!(bass(old), bass(new));
                let pcs = |note: &Note| {
                    let mut pcs: Vec<_> = midi(note).iter().map(|p| p.rem_euclid(12)).collect();
                    pcs.sort();
                    pcs
                };
                assert_eq!(pcs(old), pcs(new));
                assert_eq!(old.beats(), new.beats());
                assert_eq!(old.tie_start, new.tie_start);
                assert_eq!(old.tie_end, new.tie_end);
            }
        }
    }
    let again = arrange_sections_with_voice_leading(&score, options, Default::default());
    assert_eq!(result.voicings, again.voicings);
}

#[test]
fn constant_section_voicing_matches_the_existing_layered_api() {
    use AccordionPattern::*;
    for pattern in [
        HeldBassChord,
        BassChordPulse,
        AlternatingBassChord,
        OffbeatChords,
        SwingAccompaniment,
        WaltzBassChord,
        WaltzAlternatingBassChord,
        FineBassChord,
        Habanera,
        DrivingBassChord,
        DrivingBassChordLight,
    ] {
        let (score, layers) = fixture(
            &[
                ("D", "minor-seventh"),
                ("G", "dominant"),
                ("C", "major-seventh"),
            ],
            pattern,
        );
        let new =
            arrange_sections_with_voice_leading(&score, sections(&layers), Default::default());
        let old = arrange_with_voice_leading(&score, layers, Default::default());
        assert!(
            new.arrangement.diagnostics.is_empty(),
            "{pattern:?}: {:?}",
            new.arrangement.diagnostics
        );
        assert_eq!(new.voicings, old.voicings, "{pattern:?}");
        assert_eq!(new.motion_cost, old.motion_cost);
        assert_eq!(new.original_motion_cost, old.original_motion_cost);
        assert_eq!(new.search_limit_reached, old.search_limit_reached);
        let mut new = serde_json::to_value(new.arrangement.score.unwrap()).unwrap();
        let mut old = serde_json::to_value(old.arrangement.score.unwrap()).unwrap();
        without_ids(&mut new);
        without_ids(&mut old);
        assert_eq!(new, old, "{pattern:?}");
    }
}

#[test]
fn section_voicing_counts_repeat_edges_across_melodic_sections_and_preserves_ties() {
    use AccordionPattern::*;
    let (mut score, layers) = fixture(
        &[
            ("C", "major"),
            ("F", "major"),
            ("G", "dominant"),
            ("C", "major"),
        ],
        HeldBassChord,
    );
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].actual_length = acorde_core::MeasureLength::from_beats(3.5);
    bars[0].voices[0][0].duration = Duration::Half;
    bars[0].voices[0][0].dot_count = 1;
    bars[0].voices[0].push(Note::new(Pitch::from_midi(84, false), Duration::Eighth));
    bars[0].barline_left = Barline::RepeatStart;
    bars[2].barline_right = Barline::RepeatEnd;
    bars[2].volta = Some(acorde_core::VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[3].volta = Some(acorde_core::VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    let mut options = sections(&layers);
    options.sections = vec![at(1, Some(Arpeggio)), at(3, Some(BassChordPulse))];
    let base = arrange_with_sections(&score, options.clone())
        .score
        .unwrap();
    let result = arrange_sections_with_voice_leading(&score, options, Default::default());
    let output = result.arrangement.score.unwrap();
    assert_eq!(
        acorde_core::measure_sequence(&output),
        vec![0, 1, 2, 0, 1, 3]
    );
    assert_eq!(result.original_motion_cost, Some(movement(&base)));
    assert_eq!(result.motion_cost, Some(movement(&output)));
    assert!(result.motion_cost <= result.original_motion_cost);
    let tied = &output.parts[0].staves[1].measures[0].voices[0];
    assert!(tied.len() > 1);
    assert!(
        tied.windows(2)
            .all(|pair| pair[0].tie_start && pair[1].tie_end && pair[0].pitches == pair[1].pitches)
    );
}

#[test]
fn section_voicing_supports_meter_changes_and_explicit_low_chord_register() {
    use AccordionPattern::*;
    let (mut score, layers) = fixture(
        &[
            ("D", "minor-seventh"),
            ("G", "dominant"),
            ("C", "major-seventh"),
        ],
        HeldBassChord,
    );
    let bars = &mut score.parts[0].staves[0].measures;
    for (measure, numerator) in [(1, 3), (2, 2)] {
        bars[measure].time_sig = Some(acorde_core::TimeSignature {
            numerator,
            denominator: 4,
        });
        bars[measure].voices[0][0].duration = Duration::Half;
        bars[measure].voices[0][0].dot_count = u8::from(numerator == 3);
    }
    let mut options = sections(&layers);
    options.sections = vec![at(1, Some(WaltzBassChord)), at(2, Some(Habanera))];
    let result = arrange_sections_with_voice_leading(
        &score,
        options,
        AccordionVoicingOptions {
            min_midi: 48,
            max_midi: 59,
            max_span_semitones: 11,
            ..Default::default()
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert_eq!(
        output.parts[0].staves[1]
            .measures
            .iter()
            .map(|bar| bar.voices[0].iter().map(Note::beats).sum::<f64>())
            .collect::<Vec<_>>(),
        vec![4.0, 3.0, 2.0]
    );
    for group in result.voicings {
        assert!(
            group
                .chord_midi
                .iter()
                .all(|pitch| (48..=59).contains(pitch))
        );
        let pcs = |pitches: &[u8]| {
            let mut pcs: Vec<_> = pitches.iter().map(|pitch| pitch % 12).collect();
            pcs.sort();
            pcs
        };
        assert_eq!(pcs(&group.chord_midi), pcs(&group.original_chord_midi));
    }
}

#[test]
fn section_voicing_errors_clear_all_generated_output_and_swing_metadata() {
    let (score, layers) = fixture(
        &[("C", "major"), ("F", "major")],
        AccordionPattern::SwingAccompaniment,
    );
    let options = sections(&layers);
    let mut cases = Vec::new();
    cases.push((
        options.clone(),
        AccordionVoicingOptions {
            min_midi: 60,
            max_midi: 60,
            ..Default::default()
        },
        "no-voicing-in-range",
    ));
    cases.push((
        options.clone(),
        AccordionVoicingOptions {
            min_midi: 85,
            ..Default::default()
        },
        "invalid-voicing-options",
    ));
    let mut standard = options.clone();
    standard.bass_mode = AccordionBassMode::StandardBass;
    cases.push((standard, Default::default(), "voicing-requires-free-bass"));
    let mut overlap = options.clone();
    overlap.sections = vec![at(0, None), at(0, Some(AccordionPattern::HeldBassChord))];
    cases.push((overlap, Default::default(), "overlapping-sections"));
    for pattern in [
        None,
        Some(AccordionPattern::Arpeggio),
        Some(AccordionPattern::RisingArpeggio),
        Some(AccordionPattern::AlbertiBass),
        Some(AccordionPattern::WalkingBass),
    ] {
        let mut no_blocks = options.clone();
        no_blocks.default_pattern = pattern;
        cases.push((no_blocks, Default::default(), "no-chord-attacks"));
    }
    for (options, voicing, code) in cases {
        let result = arrange_sections_with_voice_leading(&score, options, voicing);
        assert_eq!(result.arrangement.diagnostics[0].code, code);
        assert!(result.arrangement.score.is_none());
        assert!(result.arrangement.generated_notes.is_empty());
        assert!(result.arrangement.source_notes.is_empty());
        assert!(result.arrangement.resolved_patterns.is_empty());
        assert!(result.arrangement.notated_swing_measures.is_empty());
        assert!(!result.arrangement.notated_swing);
        assert!(result.voicings.is_empty());
        assert!(result.motion_cost.is_none());
        assert!(result.original_motion_cost.is_none());
        assert!(!result.search_limit_reached);
    }
}

fn gap_options(gap: u8) -> AccordionVoicingOptions {
    AccordionVoicingOptions {
        min_melody_gap_semitones: Some(gap),
        ..Default::default()
    }
}

fn assert_scheduled_gap(score: &Score, gap: u8) {
    let events = acorde_core::to_playback_events(score, &Default::default());
    let mut overlaps = 0;
    for chord in events.iter().filter(|event| {
        event.source.as_ref().is_some_and(|addr| addr.staff == 1) && event.pitch_midi >= 48
    }) {
        for melody in events
            .iter()
            .filter(|event| event.source.as_ref().is_some_and(|addr| addr.staff == 0))
        {
            if chord.time_beats < melody.time_beats + melody.duration_beats - 1e-9
                && melody.time_beats < chord.time_beats + chord.duration_beats - 1e-9
            {
                assert!(chord.pitch_midi_cents + i32::from(gap) * 100 <= melody.pitch_midi_cents);
                overlaps += 1;
            }
        }
    }
    assert!(overlaps > 0);
}

#[test]
fn melody_gap_accounts_for_a_lower_note_during_a_sustained_chord() {
    let (mut score, layers) = fixture(&[("C", "major")], AccordionPattern::HeldBassChord);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::from_midi(72, false), Duration::Half),
        Note::new(Pitch::from_midi(64, false), Duration::Half),
    ];
    let before = serde_json::to_value(&score).unwrap();
    let free = arrange_with_voice_leading(&score, layers.clone(), Default::default());
    assert_eq!(free.voicings[0].chord_midi, vec![60, 64, 67]);
    let result = arrange_with_voice_leading(&score, layers, gap_options(3));
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(result.voicings[0].melody_ceiling_midi, Some(61));
    assert_eq!(result.voicings[0].chord_midi, vec![52, 55, 60]);
    let output = result.arrangement.score.unwrap();
    assert_scheduled_gap(&output, 3);
    assert_eq!(
        serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
        before["parts"][0]["staves"][0]
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(
        output.parts[0].staves[1].measures[0].voices[0][0].pitches[0].to_midi(),
        36
    );
}

#[test]
fn melody_gap_uses_actual_overlap_and_leaves_chords_during_rests_unrestricted() {
    for resting in [false, true] {
        let (mut score, layers) = fixture(&[("C", "major")], AccordionPattern::BassChordPulse);
        score.parts[0].staves[0].measures[0].voices[0] = (0..4)
            .map(|beat| {
                if beat % 2 == 1 && resting {
                    Note::rest(Duration::Quarter)
                } else {
                    Note::new(
                        Pitch::from_midi(if beat % 2 == 0 { 60 } else { 84 }, false),
                        Duration::Quarter,
                    )
                }
            })
            .collect();
        let result = arrange_with_voice_leading(&score, layers, gap_options(1));
        assert!(
            result.arrangement.diagnostics.is_empty(),
            "{:?}",
            result.arrangement.diagnostics
        );
        assert_eq!(result.voicings[0].chord_midi, vec![60, 64, 67]);
        assert_eq!(
            result.voicings[0].melody_ceiling_midi,
            if resting { None } else { Some(83) }
        );
        if !resting {
            assert_scheduled_gap(&result.arrangement.score.unwrap(), 1);
        }
    }
}

#[test]
fn melody_gap_protects_added_right_hand_voices_and_fractional_pitch() {
    let (mut score, mut layers) = fixture(&[("C", "major")], AccordionPattern::HeldBassChord);
    score.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(72, false);
    layers.melodic_voice = AccordionMelodicVoice::OctaveBelow;
    let base = arrange_with_layers(&score, layers.clone()).score.unwrap();
    let result = arrange_with_voice_leading(&score, layers.clone(), gap_options(1));
    assert_eq!(result.voicings[0].melody_ceiling_midi, Some(59));
    let output = result.arrangement.score.unwrap();
    assert_scheduled_gap(&output, 1);
    assert_eq!(
        base.parts[0].staves[0].measures[0].voices[1]
            .iter()
            .map(midi)
            .collect::<Vec<_>>(),
        output.parts[0].staves[0].measures[0].voices[1]
            .iter()
            .map(midi)
            .collect::<Vec<_>>()
    );

    layers.melodic_voice = AccordionMelodicVoice::Unchanged;
    let staff = &mut score.parts[0].staves[0];
    staff.transpose_semitones = -12;
    staff.measures[0].voices[0][0].pitches[0].microtone_cents = -25;
    let result = arrange_with_voice_leading(&score, layers, gap_options(1));
    assert_eq!(result.voicings[0].melody_ceiling_midi, Some(58));
    assert_scheduled_gap(&result.arrangement.score.unwrap(), 1);
}

#[test]
fn melody_gap_preserves_tied_fragments_and_repeated_section_output() {
    let (mut score, layers) = fixture(
        &[("C", "major"), ("G", "dominant"), ("C", "major")],
        AccordionPattern::HeldBassChord,
    );
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].actual_length = acorde_core::MeasureLength::from_beats(3.5);
    let mut dotted = Note::new(Pitch::from_midi(72, false), Duration::Half);
    dotted.dot_count = 1;
    bars[0].voices[0] = vec![
        dotted,
        Note::new(Pitch::from_midi(64, false), Duration::Eighth),
    ];
    bars[0].barline_left = Barline::RepeatStart;
    bars[1].barline_right = Barline::RepeatEnd;
    bars[1].voices[0][0].pitches[0] = Pitch::from_midi(65, false);
    let mut options = sections(&layers);
    options.sections = vec![
        at(1, Some(AccordionPattern::SwingAccompaniment)),
        at(2, None),
    ];
    let result = arrange_sections_with_voice_leading(&score, options, gap_options(1));
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert_eq!(acorde_core::measure_sequence(&output), vec![0, 1, 0, 1, 2]);
    assert_scheduled_gap(&output, 1);
    let tied = &output.parts[0].staves[1].measures[0].voices[0];
    assert!(tied.len() > 1);
    assert!(
        tied.windows(2)
            .all(|pair| pair[0].tie_start && pair[1].tie_end && pair[0].pitches == pair[1].pitches)
    );
    assert!(
        output.parts[0].staves[1].measures[2].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
}

#[test]
fn impossible_melody_gaps_report_the_restricting_measure_and_no_partial_score() {
    let (mut score, layers) = fixture(
        &[("C", "major"), ("C", "major")],
        AccordionPattern::HeldBassChord,
    );
    score.parts[0].staves[0].measures[1].voices[0][0].pitches[0] = Pitch::from_midi(48, false);
    let result = arrange_with_voice_leading(&score, layers.clone(), gap_options(1));
    assert_eq!(
        result.arrangement.diagnostics[0].code,
        "no-voicing-below-melody"
    );
    assert_eq!(result.arrangement.diagnostics[0].measure_index, Some(1));
    assert!(result.arrangement.score.is_none());
    assert!(result.arrangement.source_notes.is_empty());
    assert!(result.arrangement.generated_notes.is_empty());
    assert!(result.voicings.is_empty());
    assert!(result.motion_cost.is_none());
    let mut sections = sections(&layers);
    sections.default_pattern = Some(AccordionPattern::SwingAccompaniment);
    let result = arrange_sections_with_voice_leading(&score, sections, gap_options(1));
    assert!(result.arrangement.score.is_none());
    assert!(result.arrangement.resolved_patterns.is_empty());
    assert!(!result.arrangement.notated_swing);
    assert!(result.arrangement.notated_swing_measures.is_empty());
    assert!(result.voicings.is_empty());
    assert_eq!(
        arrange_with_voice_leading(&score, layers, gap_options(25))
            .arrangement
            .diagnostics[0]
            .code,
        "invalid-voicing-options"
    );
}

#[test]
fn melody_gap_rejects_unmodelled_grace_timing_and_pitch_bends() {
    let (source, layers) = fixture(&[("C", "major")], AccordionPattern::HeldBassChord);
    for grace in [true, false] {
        let mut score = source.clone();
        if grace {
            let mut note = Note::new(Pitch::from_midi(60, false), Duration::Eighth);
            note.is_grace = true;
            score.parts[0].staves[0].measures[0].voices[0].insert(0, note);
        } else {
            score.parts[0].staves[0].measures[0].voices[0][0].guitar_bend_alter_cents = Some(-100);
        }
        let result = arrange_with_voice_leading(&score, layers.clone(), gap_options(1));
        assert_eq!(
            result.arrangement.diagnostics[0].code,
            "unsupported-melody-clearance"
        );
        assert!(result.arrangement.score.is_none());
    }
}

#[test]
fn old_voicing_json_defaults_to_no_melody_constraint() {
    let options: AccordionVoicingOptions =
        serde_json::from_str(r#"{"min_midi":48,"max_midi":84,"max_span_semitones":12}"#).unwrap();
    assert_eq!(options, AccordionVoicingOptions::default());
    assert!(
        serde_json::to_value(options)
            .unwrap()
            .get("min_melody_gap_semitones")
            .is_none()
    );
    let report: acorde_core::AccordionVoicing = serde_json::from_str(
        r#"{"original_chord_midi":[60,64,67],"chord_midi":[60,64,67],"targets":[]}"#,
    )
    .unwrap();
    assert!(report.melody_ceiling_midi.is_none());
    assert!(
        serde_json::to_value(report)
            .unwrap()
            .get("melody_ceiling_midi")
            .is_none()
    );
}

#[test]
fn zero_melody_gap_allows_unison_while_one_semitone_requires_separation() {
    let (mut score, layers) = fixture(&[("C", "major")], AccordionPattern::HeldBassChord);
    score.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(60, false);
    for (gap, top) in [(0, 60), (1, 55)] {
        let result = arrange_with_voice_leading(&score, layers.clone(), gap_options(gap));
        assert_eq!(result.voicings[0].chord_midi.last(), Some(&top));
        assert_scheduled_gap(&result.arrangement.score.unwrap(), gap);
    }
}
