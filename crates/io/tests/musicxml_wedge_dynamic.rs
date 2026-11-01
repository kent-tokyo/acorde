use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::parse_musicxml;

#[test]
fn dynamic_at_wedge_endpoint_applies_to_all_voices_and_later_notes() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Music</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><direction><direction-type><wedge type="crescendo" number="1"/></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><direction><direction-type><wedge type="stop" number="1"/></direction-type></direction></measure><measure number="2"><direction><direction-type><dynamics><p/></dynamics></direction-type></direction><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><backup><duration>4</duration></backup><note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>2</voice><type>whole</type></note></measure><measure number="3"><note><pitch><step>F</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    let events = to_playback_events(&score, &PlaybackOptions::default());
    let notes: Vec<_> = events
        .iter()
        .filter(|event| event.time_beats >= 4.0)
        .map(|event| (event.pitch_midi, event.velocity))
        .collect();
    assert_eq!(notes.len(), 3);
    assert!(
        notes.iter().all(|(_, velocity)| *velocity == 48),
        "{notes:?}"
    );
    #[cfg(feature = "midi")]
    {
        let bytes = acorde_io::serialize_midi(&score).unwrap();
        let midi = midly::Smf::parse(&bytes).unwrap();
        let attacks: Vec<_> = midi
            .tracks
            .iter()
            .flatten()
            .filter_map(|event| match event.kind {
                midly::TrackEventKind::Midi {
                    message: midly::MidiMessage::NoteOn { key, vel },
                    ..
                } if vel.as_int() != 0 && [64, 65, 72].contains(&key.as_int()) => {
                    Some((key.as_int(), vel.as_int()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(attacks.len(), 3);
        assert!(
            attacks.iter().all(|(_, velocity)| *velocity == 48),
            "{attacks:?}"
        );
    }
}
