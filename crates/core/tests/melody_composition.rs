//! A composed melody can be arranged, and the quality report finds nothing the
//! arrangers were asked to avoid.
#![cfg(feature = "unstable-arrangement")]

use acorde_core::{
    ArrangementAssessmentRequest, ArrangementPartRange, BassLineOptions, BassLinePattern,
    ChordSymbol, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, KeySignature,
    MelodyChord, MelodyCompositionRequest, MelodyForm, PositionedAccordionChord, Score,
    arrange_bass_line, arrange_chordal_pad, assess_arrangement, compose_melody, validate,
};
use serde_json::json;

fn chord(root: &str, kind: &str) -> ChordSymbol {
    serde_json::from_value(json!({"root": root, "kind": kind})).unwrap()
}

/// Eight bars, one chord per bar, plus a second chord halfway through bar 6.
fn request(
    minor: bool,
    numerator: u8,
    denominator: u8,
    form: MelodyForm,
) -> MelodyCompositionRequest {
    let progression: [(&str, &str); 8] = if minor {
        [
            ("A", "minor"),
            ("D", "minor"),
            ("E", "dominant"),
            ("E", "dominant"),
            ("A", "minor"),
            ("F", "major"),
            ("D", "minor"),
            ("A", "minor"),
        ]
    } else {
        [
            ("C", "major"),
            ("A", "minor"),
            ("F", "major"),
            ("G", "major"),
            ("C", "major"),
            ("F", "major"),
            ("D", "minor"),
            ("C", "major"),
        ]
    };
    let half = match (numerator, denominator) {
        (4, 4) => 4,
        (2, 4) => 2,
        _ => 3,
    };
    let mut chords: Vec<MelodyChord> = progression
        .iter()
        .enumerate()
        .map(|(measure, (root, kind))| MelodyChord {
            measure,
            offset_eighths: 0,
            chord: chord(root, kind),
        })
        .collect();
    chords.insert(
        7,
        MelodyChord {
            measure: 6,
            offset_eighths: half,
            chord: chord(if minor { "E" } else { "G" }, "dominant"),
        },
    );
    MelodyCompositionRequest {
        key: KeySignature {
            fifths: 0,
            mode: if minor { "minor" } else { "major" }.into(),
        },
        time_numerator: numerator,
        time_denominator: denominator,
        measures: 8,
        form,
        chords,
        motif: None,
        low_midi: 62,
        high_midi: 81,
        max_leap_semitones: 7,
        seed: 0,
        tempo_bpm: 96,
    }
}

fn arrange(melody: &Score, request: &MelodyCompositionRequest) -> Score {
    let chords: Vec<_> = request
        .chords
        .iter()
        .map(|c| PositionedAccordionChord {
            measure_index: c.measure,
            offset_beats: f64::from(c.offset_eighths) / 2.0,
            chord: c.chord.clone(),
        })
        .collect();
    let pad = arrange_chordal_pad(
        melody,
        ChordalPadOptions {
            chords: chords.clone(),
            pattern: ChordalPadPattern::Held,
            pattern_sections: Vec::new(),
            meter_group_sections: Vec::new(),
            phrase_end_measures: Vec::new(),
            voicing: ChordalPadVoicing::Smooth,
            midi_program: 48,
            low_midi: 43,
            high_midi: 72,
            center_midi: 55,
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

#[test]
fn composed_melodies_arrange_without_masking_or_range_errors() {
    let mut runs = 0;
    let mut clash_ticks = 0;
    let mut parallels = 0;
    for minor in [false, true] {
        for (n, d) in [(4, 4), (3, 4), (2, 4), (6, 8)] {
            for form in [MelodyForm::Period, MelodyForm::Sentence] {
                for seed in 0..4 {
                    let mut req = request(minor, n, d, form);
                    req.seed = seed;
                    let label = format!("minor={minor} {n}/{d} {form:?} seed {seed}");
                    let composed = compose_melody(&req).unwrap();
                    let report = &composed.report;
                    assert_eq!(
                        report.strong_beat_chord_tones, report.strong_beat_notes,
                        "{label}"
                    );
                    assert!(
                        report.phrase_endings.iter().all(|e| e.satisfied),
                        "{label}: {:?}",
                        report.phrase_endings
                    );
                    let score = arrange(&composed.score, &req);
                    assert!(validate(&score).is_valid(), "{label}");
                    let mut assessment = ArrangementAssessmentRequest::new(0);
                    assessment.bass_part = Some(2);
                    assessment.ranges = vec![
                        ArrangementPartRange {
                            part: 0,
                            low_midi: 62,
                            high_midi: 81,
                        },
                        ArrangementPartRange {
                            part: 2,
                            low_midi: 28,
                            high_midi: 52,
                        },
                    ];
                    let result = assess_arrangement(&score, &assessment).unwrap();
                    assert_eq!(result.melody_masked_ticks, 0, "{label}");
                    assert_eq!(result.out_of_range_notes, 0, "{label}");
                    clash_ticks += result.melody_semitone_clash_ticks;
                    parallels += result.outer_parallel_fifths + result.outer_parallel_octaves;
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 64);
    println!("{runs} arrangements: {clash_ticks} clash ticks, {parallels} outer parallels");
}
