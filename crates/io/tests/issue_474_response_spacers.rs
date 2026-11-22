#![cfg(all(feature = "abc", feature = "mscz"))]
use acorde_core::*;
#[test]
fn hidden_timing_spacers_round_trip_without_affecting_notes() {
    let source = acorde_io::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc z2 e | c z2 e |]").unwrap();
    let options:SectionedAccordionOptions=serde_json::from_value(serde_json::json!({"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}}],"bass_mode":"standard-bass","default_pattern":"bass-chord-pulse","sections":[]})).unwrap();
    let result = arrange_with_response_fills(
        &source,
        options,
        AccordionResponseOptions {
            rest_notes: vec![NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            }],
        },
    );
    let score = result.arrangement.score.unwrap();
    let bytes = acorde_io::serialize_mscz(&score).unwrap();
    let restored = acorde_io::parse_mscz(&bytes).unwrap();
    let voice = &restored.parts[0].staves[0].measures[0].voices[1];
    assert!(voice.iter().any(|n| !n.is_rest));
    assert!(voice.iter().filter(|n| n.is_rest).all(|n| n.hidden));
    let events = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| {
                (
                    e.pitch_midi,
                    (e.time_beats * 960.0).round() as i64,
                    (e.duration_beats * 960.0).round() as i64,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&score), events(&restored));
}
