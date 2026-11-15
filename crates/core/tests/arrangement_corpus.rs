use acorde_core::{
    ArrangementProposalRequest, ArrangementRegionRequest, BassLineOptions, BassLinePattern,
    ChordProposalRequest, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, Clef, Duration,
    EnsembleRole, EnsembleRoleOptions, Measure, MeasureLength, Note, Part, Pitch,
    PositionedAccordionChord, Score, Staff, arrange_bass_line, arrange_chordal_pad,
    regenerate_arrangement_region, replay_arrangement_proposal, suggest_arrangement_proposals,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

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

fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn part_summary(score: &Score, part: usize) -> serde_json::Value {
    let mut onsets = Vec::new();
    let mut durations = BTreeMap::<u64, usize>::new();
    let mut classes = [0usize; 12];
    let mut event_bytes = Vec::new();
    let mut lowest = u8::MAX;
    let mut highest = 0u8;
    let mut event_count = 0usize;
    for event in acorde_core::to_playback_events(score, &Default::default()) {
        if !event
            .source
            .as_ref()
            .is_some_and(|address| address.part == part)
        {
            continue;
        }
        let onset = (event.time_beats * 480.0).round() as u64;
        let duration = (event.duration_beats * 480.0).round() as u64;
        let pitch = event.pitch_midi;
        onsets.push(onset);
        *durations.entry(duration).or_default() += 1;
        classes[usize::from(pitch % 12)] += 1;
        lowest = lowest.min(pitch);
        highest = highest.max(pitch);
        event_bytes.extend_from_slice(&onset.to_le_bytes());
        event_bytes.extend_from_slice(&duration.to_le_bytes());
        event_bytes.push(pitch);
        event_count += 1;
    }
    onsets.sort_unstable();
    onsets.dedup();
    json!({
        "events": event_count,
        "onsets_480": onsets,
        "duration_counts_480": durations,
        "pitch_classes": classes,
        "lowest_midi": lowest,
        "highest_midi": highest,
        "event_fingerprint": fingerprint(&event_bytes)
    })
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

fn source_playback(score: &Score, source_parts: usize) -> Vec<(usize, u8, u64, u64)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event
                .source
                .filter(|address| address.part < source_parts)
                .map(|address| {
                    (
                        address.part,
                        event.pitch_midi,
                        (event.time_beats * 480.0).round() as u64,
                        (event.duration_beats * 480.0).round() as u64,
                    )
                })
        })
        .collect()
}

#[test]
fn unified_proposals_cover_development_and_holdout_meters() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/corpus-v1.json"
    ))
    .unwrap();
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
    for case in &corpus.cases {
        let score = source(case);
        let request = ArrangementProposalRequest {
            harmony: ChordProposalRequest::default(),
            locked_chord_entries: Vec::new(),
            ensemble_template: ensemble_template.clone(),
            count: 3,
        };
        let result = suggest_arrangement_proposals(&score, &request);
        assert!(
            result.diagnostics.is_empty(),
            "{}: {:?}",
            case.id,
            result.diagnostics
        );
        assert_eq!(result.candidates.len(), 3, "{}", case.id);
        for candidate in &result.candidates {
            assert_eq!(
                candidate.score.parts.len(),
                score.parts.len() + 2,
                "{}",
                case.id
            );
            assert_eq!(
                source_playback(&candidate.score, score.parts.len()),
                source_playback(&score, score.parts.len()),
                "{} source playback",
                case.id
            );
            let replay = replay_arrangement_proposal(&score, &candidate.resolved);
            assert!(
                replay.diagnostics.is_empty(),
                "{}: {:?}",
                case.id,
                replay.diagnostics
            );
            assert_eq!(
                part_summary(&replay.score.unwrap(), score.parts.len()),
                part_summary(&candidate.score, score.parts.len()),
                "{} replayed harmony",
                case.id
            );
        }
        let region = regenerate_arrangement_region(
            &score,
            ArrangementRegionRequest {
                original: result.candidates[1].resolved.clone(),
                replacement: result.candidates[2].resolved.clone(),
                start_measure: 0,
                end_measure_exclusive: 1,
                locked_roles: vec![EnsembleRole::Bass],
                locked_measures: Vec::new(),
                locked_chord_entries: Vec::new(),
            },
        );
        assert!(
            region.diagnostics.is_empty(),
            "{}: {:?}",
            case.id,
            region.diagnostics
        );
        let output = region.score.unwrap();
        assert_eq!(
            source_playback(&output, score.parts.len()),
            source_playback(&score, score.parts.len()),
            "{} regenerated source",
            case.id
        );
        assert_eq!(
            part_summary(&output, score.parts.len() + 1),
            part_summary(&result.candidates[1].score, score.parts.len() + 1),
            "{} locked bass",
            case.id
        );
    }
}

#[test]
fn fixed_development_and_holdout_sources_survive_two_independent_parts() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/corpus-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert_eq!(corpus.cases.len(), 8);
    assert_eq!(
        corpus
            .cases
            .iter()
            .filter(|case| case.partition == "development")
            .count(),
        4
    );
    assert_eq!(
        corpus
            .cases
            .iter()
            .filter(|case| case.partition == "holdout")
            .count(),
        4
    );
    let mut reports = Vec::new();
    for case in &corpus.cases {
        let score = source(case);
        assert!(
            acorde_core::model::validate::validate(&score).is_valid(),
            "{} source",
            case.id
        );
        let original_parts = serde_json::to_value(&score.parts).unwrap();
        let original_playback = source_playback(&score, score.parts.len());
        let mut profiles = Vec::new();
        for (name, pad_pattern, voicing, bass_pattern, phrase_end) in [
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
            let harmony = chords(case);
            let phrase_end_measures = if phrase_end {
                vec![case.melody.len() - 1]
            } else {
                Vec::new()
            };
            let pad = arrange_chordal_pad(
                &score,
                ChordalPadOptions {
                    chords: harmony.clone(),
                    pattern: pad_pattern,
                    pattern_sections: Vec::new(),
                    meter_group_sections: Vec::new(),
                    phrase_end_measures: phrase_end_measures.clone(),
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
                "{} {name} pad: {:?}",
                case.id,
                pad.diagnostics
            );
            let pad_score = pad.score.unwrap();
            let bass = arrange_bass_line(
                &pad_score,
                BassLineOptions {
                    chords: harmony,
                    pattern: bass_pattern,
                    midi_program: 32,
                    low_midi: 28,
                    high_midi: 60,
                    center_midi: 40,
                },
            );
            assert!(
                bass.diagnostics.is_empty(),
                "{} {name} bass: {:?}",
                case.id,
                bass.diagnostics
            );
            let output = bass.score.unwrap();
            assert_eq!(
                output.parts.len(),
                score.parts.len() + 2,
                "{} {name}",
                case.id
            );
            assert_eq!(
                serde_json::to_value(&output.parts[..score.parts.len()]).unwrap(),
                original_parts,
                "{} {name}",
                case.id
            );
            assert_eq!(
                source_playback(&output, score.parts.len()),
                original_playback,
                "{} {name}",
                case.id
            );
            assert!(
                acorde_core::model::validate::validate(&output).is_valid(),
                "{} {name} output",
                case.id
            );
            profiles.push(json!({
                "name": name,
                "resolved_plan": {"chords": case.chords, "pad_pattern": pad_pattern, "voicing": voicing, "bass_pattern": bass_pattern, "programs": [48,32], "phrase_end_measures": phrase_end_measures, "meter_groups": pad.resolved_meter_groups, "phrase_start_ticks": pad.phrase_start_ticks},
                "pad_motion": pad.motion_report,
                "bass_motion": bass.motion_report,
                "pad": part_summary(&output, score.parts.len()),
                "bass": part_summary(&output, score.parts.len() + 1)
            }));
        }
        let signatures = profiles
            .iter()
            .map(|profile| {
                (
                    profile["pad"]["event_fingerprint"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    profile["bass"]["event_fingerprint"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            signatures.len(),
            5,
            "{} did not yield five distinct profiles",
            case.id
        );
        reports.push(json!({
            "id": case.id,
            "partition": case.partition,
            "source_fingerprint": fingerprint(&serde_json::to_vec(case).unwrap()),
            "source": part_summary(&score, 0),
            "profiles": profiles
        }));
    }
    let report = json!({"contract_version": 1, "cases": reports});
    if std::env::var_os("ACORDE_PRINT_ARRANGEMENT_BASELINE").is_some() {
        println!("BASELINE_JSON:{}", serde_json::to_string(&report).unwrap());
        return;
    }
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/baseline-v1.json"
    ))
    .unwrap();
    assert_eq!(report, baseline);
}
