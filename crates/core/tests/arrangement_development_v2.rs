use std::collections::BTreeSet;

use acorde_core::{to_playback_events, validate};

#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
use development_v2::{Corpus, FIXTURE, Unit, score};

#[test]
fn development_scores_are_distinct_complete_and_playable() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.cases.len(), 12);
    let mut ids = BTreeSet::new();
    let mut melodies = BTreeSet::new();
    let mut meters = BTreeSet::new();
    let mut major = 0;
    let mut minor = 0;
    let mut saw_pickup = false;
    let mut saw_modulation = false;
    let mut saw_repeat = false;
    let mut saw_duet = false;
    let mut saw_32_bar = false;
    for case in &corpus.cases {
        assert!(ids.insert(&case.id), "duplicate id: {}", case.id);
        assert!(
            case.id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        );
        assert!(!case.title.is_empty() && !case.form.is_empty());
        assert!((16..=32).contains(&case.melody.len()), "{}", case.id);
        assert!(case.sections.len() >= 4, "{}", case.id);
        assert_eq!(case.sections[0].start_measure, 0, "{}", case.id);
        assert_eq!(
            case.sections.last().unwrap().end_measure_exclusive,
            case.melody.len(),
            "{}",
            case.id
        );
        assert!(case.sections.iter().all(|section| section.start_measure
            < section.end_measure_exclusive
            && !section.label.is_empty()));
        assert!(
            case.sections
                .windows(2)
                .all(|pair| pair[0].end_measure_exclusive == pair[1].start_measure)
        );
        assert_eq!(
            case.sections
                .iter()
                .map(|section| section.label.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            case.form,
            "{}",
            case.id
        );
        assert!((40..=200).contains(&case.tempo_bpm), "{}", case.id);
        assert!((-7..=7).contains(&case.key_fifths), "{}", case.id);
        assert!(case.melody_program <= 127, "{}", case.id);
        assert!(case.second_program.is_some() == case.second_line.is_some());
        assert!(matches!(case.mode.as_str(), "major" | "minor"));
        major += usize::from(case.mode == "major");
        minor += usize::from(case.mode == "minor");
        let unit_count = match case.unit {
            Unit::Quarter => {
                assert_eq!(case.meter[1], 4, "{}", case.id);
                usize::from(case.meter[0])
            }
            Unit::Eighth => {
                assert_eq!(case.meter[1], 8, "{}", case.id);
                usize::from(case.meter[0])
            }
        };
        meters.insert(case.meter);
        for (measure, pitches) in case.melody.iter().enumerate() {
            let expected = if measure == 0 {
                case.pickup_eighths.unwrap_or(unit_count)
            } else {
                unit_count
            };
            assert_eq!(pitches.len(), expected, "{} bar {}", case.id, measure + 1);
            assert!(pitches.iter().any(|pitch| *pitch != 0), "{}", case.id);
            assert!(
                pitches
                    .iter()
                    .all(|pitch| *pitch == 0 || (36..=96).contains(pitch))
            );
        }
        if let Some(line) = &case.second_line {
            assert_eq!(line.len(), case.melody.len());
            assert!(line.iter().all(|bar| bar.len() == unit_count));
        }
        assert!(
            case.key_changes
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
        assert!(case.key_changes.iter().all(|(measure, fifths)| *measure > 0
            && *measure < case.melody.len()
            && (-7..=7).contains(fifths)));
        if let Some([start, end]) = case.repeat_range {
            assert!(start < end && end <= case.melody.len());
        }
        let melody_key = serde_json::to_string(&case.melody).unwrap();
        assert!(melodies.insert(melody_key), "duplicate melody: {}", case.id);
        let source = score(case);
        let report = validate(&source);
        assert!(report.is_valid(), "{}: {:?}", case.id, report.errors);
        let events = to_playback_events(&source, &Default::default());
        assert!(events.len() >= case.melody.len(), "{}", case.id);
        assert!(events.iter().all(|event| event.time_beats.is_finite()
            && event.duration_beats.is_finite()
            && event.duration_beats > 0.0));
        saw_pickup |= case.pickup_eighths.is_some();
        saw_modulation |= !case.key_changes.is_empty();
        saw_repeat |= case.repeat_range.is_some();
        saw_duet |= case.second_line.is_some();
        saw_32_bar |= case.melody.len() == 32;
    }
    assert!(major >= 4 && minor >= 4);
    assert!(meters.contains(&[3, 4]) && meters.contains(&[4, 4]));
    assert!(meters.contains(&[5, 8]) && meters.contains(&[6, 8]) && meters.contains(&[7, 8]));
    assert!(saw_pickup && saw_modulation && saw_repeat && saw_duet && saw_32_bar);
}
