#![cfg(feature = "musicxml")]

use acorde_io::{parse_musicxml_with_report, serialize_musicxml};

fn score(body: &str, divisions: u32) -> String {
    format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>{divisions}</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"#
    )
}

fn pitched(step: &str, duration: u32) -> String {
    format!(
        "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>quarter</type></note>"
    )
}

#[test]
fn one_tick_tuplet_rounding_keeps_both_notes_in_the_source_voice() {
    let xml = score(
        &format!(
            "{}<backup><duration>1</duration></backup>{}",
            pitched("C", 5280),
            pitched("D", 5280)
        ),
        5280,
    );
    let report = parse_musicxml_with_report(&xml).unwrap();
    let measure = &report.score.parts[0].staves[0].measures[0];
    assert_eq!(measure.voices[0].len(), 2);
    assert!(measure.voices[0].iter().all(|note| !note.is_rest));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "musicxml.rounded-voice-cursor")
    );
}

#[test]
fn overlaid_measure_rest_uses_a_separate_voice_and_roundtrips() {
    let xml = score(
        &format!(
            "{}<note><rest measure=\"yes\"/><duration>4</duration><voice>1</voice></note><backup><duration>4</duration></backup>{}",
            pitched("C", 1),
            pitched("D", 1)
        ),
        1,
    );
    let report = parse_musicxml_with_report(&xml).unwrap();
    let measure = &report.score.parts[0].staves[0].measures[0];
    assert_eq!(
        measure.voices[0]
            .iter()
            .filter(|note| !note.is_rest)
            .count(),
        2
    );
    assert_eq!(measure.voices[1].len(), 1);
    assert!(measure.voices[1][0].is_rest);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "musicxml.recovered-overlaid-measure-rest")
    );

    let exported = serialize_musicxml(&report.score).unwrap();
    let restored = parse_musicxml_with_report(&exported).unwrap().score;
    let restored_measure = &restored.parts[0].staves[0].measures[0];
    assert_eq!(
        restored_measure.voices[0]
            .iter()
            .filter(|note| !note.is_rest)
            .count(),
        2
    );
    assert_eq!(restored_measure.voices[1].len(), 1);
    assert!(restored_measure.voices[1][0].is_rest);
}
