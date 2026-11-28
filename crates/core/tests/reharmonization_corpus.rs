//! Reharmonization on the fixed arrangement corpus v1, checked with the
//! automatic arrangement assessment.
#![cfg(feature = "unstable-arrangement")]

use acorde_core::{
    ArrangementAssessmentRequest, ArrangementPartRange, BassLineOptions, BassLinePattern,
    ChordPlan, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, Clef, Duration, Measure,
    MeasureLength, Note, Part, Pitch, PositionedAccordionChord, ReharmonizationRequest, Score,
    Staff, apply_reharmonizations, arrange_bass_line, arrange_chordal_pad, assess_arrangement,
    suggest_reharmonizations,
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

fn plan(case: &Case) -> ChordPlan {
    let entries: Vec<_> = case
        .chords
        .iter()
        .map(|(measure, offset, root, kind)| {
            let sixty_fourths = (offset * 64.0).round() as u32;
            json!({
                "position": {"measure_index": measure, "offset": {"numerator": sixty_fourths, "denominator": 64}},
                "offset_beats": offset,
                "chord": {"root": root, "kind": kind},
                "origin": "host-authored",
                "sources": []
            })
        })
        .collect();
    serde_json::from_value(json!({"contract_version": 1, "entries": entries})).unwrap()
}

fn positioned(plan: &ChordPlan) -> Vec<PositionedAccordionChord> {
    plan.entries
        .iter()
        .map(|entry| PositionedAccordionChord {
            measure_index: entry.position.measure_index,
            offset_beats: entry.position.offset.beats(),
            chord: entry.chord.clone(),
        })
        .collect()
}

/// Smooth held pad under the melody with clearance, root bass.
fn arrange(score: &Score, plan: &ChordPlan) -> Score {
    let chords = positioned(plan);
    let pad = arrange_chordal_pad(
        score,
        ChordalPadOptions {
            chords: chords.clone(),
            pattern: ChordalPadPattern::Held,
            pattern_sections: Vec::new(),
            meter_group_sections: Vec::new(),
            phrase_end_measures: Vec::new(),
            voicing: ChordalPadVoicing::Smooth,
            midi_program: 48,
            low_midi: 40,
            high_midi: 72,
            center_midi: 52,
            min_melody_gap_semitones: Some(1),
            melody_part_index: Some(0),
        },
    );
    assert!(pad.diagnostics.is_empty(), "{:?}", pad.diagnostics);
    let bass = arrange_bass_line(
        &pad.score.unwrap(),
        BassLineOptions {
            chords,
            pattern: BassLinePattern::RootPulse,
            midi_program: 32,
            low_midi: 28,
            high_midi: 52,
            center_midi: 40,
            rhythm_sections: vec![],
        },
    );
    assert!(bass.diagnostics.is_empty(), "{:?}", bass.diagnostics);
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

/// Every candidate on every corpus case: structural invariants, then the
/// assessment counts before and after, frozen in a baseline.
///
/// Regenerate after a reviewed change with `ACORDE_PRINT_REHARMONIZATION_BASELINE=1
/// cargo test -p acorde-core --features unstable-arrangement --test
/// reharmonization_corpus -- --nocapture`.
#[test]
fn corpus_reharmonizations_match_frozen_baseline() {
    let mut results = BTreeMap::new();
    for case in &corpus().cases {
        let score = source(case);
        let parts = score.parts.len();
        let original = plan(case);
        let mut assess = ArrangementAssessmentRequest::new(0);
        assess.bass_part = Some(parts + 1);
        assess.ranges = vec![ArrangementPartRange {
            part: parts,
            low_midi: 40,
            high_midi: 72,
        }];
        let base = assess_arrangement(&arrange(&score, &original), &assess).unwrap();
        let result = suggest_reharmonizations(
            &score,
            &ReharmonizationRequest {
                plan: original.clone(),
                melody_part: 0,
                start_measure: 0,
                end_measure_exclusive: case.melody.len(),
                locked_measures: Vec::new(),
                locked_entries: Vec::new(),
                techniques: Vec::new(),
                max_candidates: 64,
            },
        )
        .unwrap();
        assert_eq!(result.eligible, result.candidates.len());
        for candidate in &result.candidates {
            let applied =
                apply_reharmonizations(&original, std::slice::from_ref(candidate)).unwrap();
            // Only the replaced entry changes; the arrival and all others stay.
            let inserted = candidate.chords.len() - 1;
            assert_eq!(applied.entries.len(), original.entries.len() + inserted);
            for (index, entry) in original.entries.iter().enumerate() {
                if index < candidate.replaced_entry {
                    assert_eq!(&applied.entries[index], entry);
                } else if index > candidate.replaced_entry {
                    assert_eq!(
                        &applied.entries[index + inserted],
                        entry,
                        "{}",
                        candidate.id
                    );
                }
            }
            let arranged = arrange(&score, &applied);
            assert_eq!(
                serde_json::to_value(&arranged.parts[..parts]).unwrap(),
                serde_json::to_value(&score.parts).unwrap(),
                "{} source parts",
                candidate.id
            );
            let report = assess_arrangement(&arranged, &assess).unwrap();
            // The new harmony never adds masking or outer parallels on this corpus.
            assert!(report.melody_masked_ticks <= base.melody_masked_ticks);
            assert!(report.outer_parallel_fifths <= base.outer_parallel_fifths);
            assert!(report.outer_parallel_octaves <= base.outer_parallel_octaves);
            results.insert(
                format!("{}/{}", case.id, candidate.id),
                json!({
                    "chords": candidate.chords.iter().map(|c| format!("{} {}", c.chord.root, c.chord.kind)).collect::<Vec<_>>(),
                    "avoid_beats": candidate.melody.avoid_beats,
                    "semitone_clash_ticks": [base.melody_semitone_clash_ticks, report.melody_semitone_clash_ticks],
                    "out_of_range_notes": [base.out_of_range_notes, report.out_of_range_notes],
                    "bass_leaps": [base.bass_leaps, report.bass_leaps],
                }),
            );
        }
    }
    if std::env::var_os("ACORDE_PRINT_REHARMONIZATION_BASELINE").is_some() {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"contract_version": 1, "results": results}))
                .unwrap()
        );
    }
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/reharmonization-baseline-v1.json"
    ))
    .unwrap();
    assert_eq!(baseline["contract_version"], 1);
    let expected: BTreeMap<String, serde_json::Value> =
        serde_json::from_value(baseline["results"].clone()).unwrap();
    assert_eq!(expected.len(), results.len());
    for (key, value) in &expected {
        assert_eq!(results.get(key), Some(value), "{key}");
    }
}
