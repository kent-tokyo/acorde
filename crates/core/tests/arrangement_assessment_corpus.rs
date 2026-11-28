//! Frozen automatic-assessment counts for the fixed arrangement corpus v1.
//!
//! Any change to these counts must be reviewed and the fixture updated with
//! `ACORDE_PRINT_ASSESSMENT_BASELINE=1 cargo test -p acorde-core --features
//! unstable-arrangement --test arrangement_assessment_corpus -- --nocapture`.
#![cfg(feature = "unstable-arrangement")]

use acorde_core::{
    ArrangementAssessment, ArrangementAssessmentRequest, ArrangementFindingKind,
    ArrangementPartRange, ArrangementProposalRequest, BassLineOptions, BassLinePattern,
    ChordProposalRequest, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, Clef, Duration,
    EnsembleRoleOptions, Measure, MeasureLength, Note, Part, Pitch, PositionedAccordionChord,
    Score, Staff, arrange_bass_line, arrange_chordal_pad, assess_arrangement,
    suggest_arrangement_proposals,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Corpus {
    contract_version: u16,
    cases: Vec<Case>,
}

#[derive(Deserialize, Serialize)]
struct Case {
    id: String,
    partition: String,
    meter: [u8; 2],
    #[serde(default)]
    pickup_beats: Option<f64>,
    #[serde(default)]
    second_part: bool,
    #[serde(default)]
    repeat: bool,
    melody: Vec<Vec<u8>>,
    chords: Vec<(usize, f64, String, String)>,
}

fn source(case: &Case) -> Score {
    let mut score = Score::new(
        &case.id,
        120,
        case.meter[0],
        case.meter[1],
        0,
        case.melody.len() as u32,
    );
    for (measure, pitches) in case.melody.iter().enumerate() {
        let beats = if measure == 0 {
            case.pickup_beats
                .unwrap_or(f64::from(case.meter[0]) * 4.0 / f64::from(case.meter[1]))
        } else {
            f64::from(case.meter[0]) * 4.0 / f64::from(case.meter[1])
        };
        let voice = pitches
            .iter()
            .enumerate()
            .map(|(index, pitch)| {
                let duration = if index + 1 == pitches.len() && beats.fract() == 0.5 {
                    Duration::Eighth
                } else {
                    Duration::Quarter
                };
                Note::new(Pitch::from_midi(*pitch, false), duration)
            })
            .collect();
        score.parts[0].staves[0].measures[measure].voices[0] = voice;
    }
    if let Some(beats) = case.pickup_beats {
        score.parts[0].staves[0].measures[0].actual_length = MeasureLength::from_beats(beats);
    }
    if case.repeat {
        let bars = &mut score.parts[0].staves[0].measures;
        bars[0].barline_left = acorde_core::model::notation::Barline::RepeatStart;
        if let Some(last) = bars.last_mut() {
            last.barline_right = acorde_core::model::notation::Barline::RepeatEnd;
        }
    }
    if case.second_part {
        let mut part = Part::new("Oboe", "Ob.");
        part.midi_channel = 1;
        part.midi_program = 68;
        let mut staff = Staff::new(Clef::Treble);
        for measure in 0..case.melody.len() {
            let mut bar = Measure::empty(case.meter[0], case.meter[1]);
            bar.number = (measure + 1) as u32;
            bar.voices[0] = vec![Note::new(Pitch::from_midi(76, false), Duration::Whole)];
            staff.measures.push(bar);
        }
        part.staves.push(staff);
        score.parts.push(part);
    }
    score
}

fn chords(case: &Case) -> Vec<PositionedAccordionChord> {
    case.chords
        .iter()
        .map(
            |(measure_index, offset_beats, root, kind)| PositionedAccordionChord {
                measure_index: *measure_index,
                offset_beats: *offset_beats,
                chord: serde_json::from_value(serde_json::json!({"root":root,"kind":kind}))
                    .unwrap(),
            },
        )
        .collect()
}

fn summary(report: &ArrangementAssessment) -> serde_json::Value {
    assert!(!report.findings_truncated);
    json!({
        "events": report.events,
        "melody_sounding_ticks": report.melody_sounding_ticks,
        "melody_masked_ticks": report.melody_masked_ticks,
        "melody_semitone_clash_ticks": report.melody_semitone_clash_ticks,
        "outer_parallel_fifths": report.outer_parallel_fifths,
        "outer_parallel_octaves": report.outer_parallel_octaves,
        "part_parallel_perfects": report.part_parallel_perfects,
        "out_of_range_notes": report.out_of_range_notes,
        "bass_leaps": report.bass_leaps,
        "max_bass_leap_semitones": report.max_bass_leap_semitones,
    })
}

fn request(
    harmony: usize,
    bass: usize,
    harmony_range: (u8, u8),
    bass_range: (u8, u8),
) -> ArrangementAssessmentRequest {
    let mut request = ArrangementAssessmentRequest::new(0);
    request.bass_part = Some(bass);
    request.ranges = vec![
        ArrangementPartRange {
            part: harmony,
            low_midi: harmony_range.0,
            high_midi: harmony_range.1,
        },
        ArrangementPartRange {
            part: bass,
            low_midi: bass_range.0,
            high_midi: bass_range.1,
        },
    ];
    request
}

fn profile_score(
    score: &Score,
    case: &Case,
    pattern: ChordalPadPattern,
    voicing: ChordalPadVoicing,
    bass_pattern: BassLinePattern,
    phrase_end: bool,
) -> Score {
    let harmony = chords(case);
    let pad = arrange_chordal_pad(
        score,
        ChordalPadOptions {
            chords: harmony.clone(),
            pattern,
            pattern_sections: Vec::new(),
            meter_group_sections: Vec::new(),
            phrase_end_measures: if phrase_end {
                vec![case.melody.len() - 1]
            } else {
                Vec::new()
            },
            voicing,
            midi_program: 48,
            low_midi: 48,
            high_midi: 84,
            center_midi: 64,
            min_melody_gap_semitones: None,
            melody_part_index: None,
        },
    );
    assert!(
        pad.diagnostics.is_empty(),
        "{}: {:?}",
        case.id,
        pad.diagnostics
    );
    let bass = arrange_bass_line(
        &pad.score.unwrap(),
        BassLineOptions {
            chords: harmony,
            pattern: bass_pattern,
            midi_program: 32,
            low_midi: 28,
            high_midi: 60,
            center_midi: 40,
            rhythm_sections: vec![],
        },
    );
    assert!(
        bass.diagnostics.is_empty(),
        "{}: {:?}",
        case.id,
        bass.diagnostics
    );
    bass.score.unwrap()
}

fn corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/corpus-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    corpus
}

fn assessments() -> BTreeMap<String, serde_json::Value> {
    let ensemble_template: EnsembleRoleOptions = serde_json::from_value(json!({
        "harmony": {
            "chords":[], "pattern":"held", "voicing":"smooth",
            "midi_program":0, "low_midi":40, "center_midi":48, "high_midi":56,
            "melody_part_index":0, "min_melody_gap_semitones":4
        },
        "bass": {
            "chords":[], "pattern":"root-pulse", "midi_program":32,
            "low_midi":28, "center_midi":33, "high_midi":39
        },
        "min_bass_harmony_gap_semitones":3
    }))
    .unwrap();
    let mut results = BTreeMap::new();
    for case in &corpus().cases {
        let score = source(case);
        let parts = score.parts.len();
        for (name, pattern, voicing, bass_pattern, phrase_end) in [
            (
                "held-root",
                ChordalPadPattern::Held,
                ChordalPadVoicing::RootPosition,
                BassLinePattern::RootPulse,
                false,
            ),
            (
                "offbeat-fifth",
                ChordalPadPattern::OffbeatPulse,
                ChordalPadVoicing::Smooth,
                BassLinePattern::RootFifth,
                false,
            ),
            (
                "arpeggio-approach",
                ChordalPadPattern::ArpeggioUpDown,
                ChordalPadVoicing::Smooth,
                BassLinePattern::Approach,
                false,
            ),
            (
                "held-open",
                ChordalPadPattern::Held,
                ChordalPadVoicing::Open,
                BassLinePattern::RootPulse,
                false,
            ),
            (
                "metered-ending",
                ChordalPadPattern::MeterPulse,
                ChordalPadVoicing::GlobalSmooth,
                BassLinePattern::RootFifth,
                true,
            ),
        ] {
            let output = profile_score(&score, case, pattern, voicing, bass_pattern, phrase_end);
            let report =
                assess_arrangement(&output, &request(parts, parts + 1, (48, 84), (28, 60)))
                    .unwrap();
            results.insert(format!("{}/profile/{name}", case.id), summary(&report));
        }
        let proposals = suggest_arrangement_proposals(
            &score,
            &ArrangementProposalRequest {
                harmony: ChordProposalRequest::default(),
                locked_chord_entries: Vec::new(),
                ensemble_template: ensemble_template.clone(),
                count: 3,
            },
        );
        assert!(
            proposals.diagnostics.is_empty(),
            "{}: {:?}",
            case.id,
            proposals.diagnostics
        );
        for (index, candidate) in proposals.candidates.iter().enumerate() {
            let report = assess_arrangement(
                &candidate.score,
                &request(parts, parts + 1, (40, 56), (28, 39)),
            )
            .unwrap();
            results.insert(format!("{}/proposal/{index}", case.id), summary(&report));
        }
    }
    results
}

#[test]
fn corpus_assessment_matches_frozen_baseline() {
    let actual = assessments();
    if std::env::var_os("ACORDE_PRINT_ASSESSMENT_BASELINE").is_some() {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"contract_version": 1, "results": actual}))
                .unwrap()
        );
    }
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/assessment-baseline-v1.json"
    ))
    .unwrap();
    assert_eq!(baseline["contract_version"], 1);
    let expected: BTreeMap<String, serde_json::Value> =
        serde_json::from_value(baseline["results"].clone()).unwrap();
    assert_eq!(expected.len(), 64, "8 cases x (5 profiles + 3 proposals)");
    for (key, value) in &expected {
        assert_eq!(actual.get(key), Some(value), "{key}");
    }
    assert_eq!(actual.len(), expected.len());
}

/// Deliberately damaged copies of a corpus arrangement must each raise the
/// matching count above the clean result.
#[test]
fn damaged_arrangements_are_detected() {
    let corpus = corpus();
    let case = &corpus.cases[0];
    let score = source(case);
    let parts = score.parts.len();
    let clean = profile_score(
        &score,
        case,
        ChordalPadPattern::Held,
        ChordalPadVoicing::RootPosition,
        BassLinePattern::RootPulse,
        false,
    );
    let request = request(parts, parts + 1, (48, 84), (28, 60));
    let base = assess_arrangement(&clean, &request).unwrap();

    let transpose = |score: &Score, part: usize, semitones: i16| {
        let mut score = score.clone();
        for staff in &mut score.parts[part].staves {
            for measure in &mut staff.measures {
                for voice in &mut measure.voices {
                    for note in voice.iter_mut() {
                        for pitch in &mut note.pitches {
                            *pitch = Pitch::from_midi((pitch.to_midi() + semitones) as u8, false);
                        }
                    }
                }
            }
        }
        score
    };

    // Pad two octaves up: sounds above the melody and out of range.
    let raised = assess_arrangement(&transpose(&clean, parts, 24), &request).unwrap();
    assert!(raised.melody_masked_ticks > base.melody_masked_ticks);
    assert!(raised.out_of_range_notes > base.out_of_range_notes);
    assert!(
        raised
            .findings
            .iter()
            .any(|f| f.kind == ArrangementFindingKind::MelodyMasked)
    );

    // Bass a semitone up against the melody line: semitone clashes appear.
    let shifted = assess_arrangement(&transpose(&clean, parts, 1), &request).unwrap();
    assert!(shifted.melody_semitone_clash_ticks > base.melody_semitone_clash_ticks);

    // Bass replaced by the melody a twelfth lower: every melody move is an outer parallel.
    let mut doubled = clean.clone();
    doubled.parts[parts + 1] = transpose(&score, 0, -19).parts[0].clone();
    let report = assess_arrangement(&doubled, &request).unwrap();
    assert!(report.outer_parallel_fifths > base.outer_parallel_fifths);
}
