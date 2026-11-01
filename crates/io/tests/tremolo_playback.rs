#![cfg(feature = "musicxml")]

use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::{parse_musicxml, serialize_musicxml};

fn score_xml(chord: bool, kind: &str) -> String {
    let extra = if chord {
        "<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>"
    } else {
        ""
    };
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><ornaments><tremolo type="{kind}">3</tremolo></ornaments></notations></note>{extra}<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note></measure></part></score-partwise>"#
    )
}

#[test]
fn measured_single_note_and_chord_have_repeated_synchronized_attacks() {
    for chord in [false, true] {
        let score = parse_musicxml(&score_xml(chord, "single")).unwrap();
        let events = to_playback_events(&score, &PlaybackOptions::default());
        let pitches = if chord { vec![60, 64] } else { vec![60] };
        for pitch in pitches {
            let attacks: Vec<_> = events.iter().filter(|e| e.pitch_midi == pitch).collect();
            assert_eq!(attacks.len(), 16);
            for (index, attack) in attacks.iter().enumerate() {
                assert!((attack.time_beats - index as f64 / 8.0).abs() < 1e-9);
                assert!((attack.duration_beats - 0.125).abs() < 1e-9);
            }
        }
        let next = events.iter().find(|e| e.pitch_midi == 62).unwrap();
        assert_eq!(next.time_beats, 2.0);
        assert_eq!(next.duration_beats, 2.0);
    }
}

#[test]
fn paired_tremolo_is_preserved_without_falsely_repeating_one_pitch() {
    let score = parse_musicxml(&score_xml(false, "start")).unwrap();
    assert_eq!(
        score.parts[0].staves[0].measures[0].voices[0][0]
            .tremolo_pair
            .as_deref(),
        Some("start")
    );
    assert_eq!(
        to_playback_events(&score, &PlaybackOptions::default()).len(),
        2
    );
    let xml = serialize_musicxml(&score).unwrap();
    assert!(xml.contains("<tremolo type=\"start\">3</tremolo>"));
}

#[test]
fn tremolo_includes_written_flags_and_integrates_the_tempo_ramp() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("tremolo", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].tempo_ramp_to = Some(60);
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
    note.articulations.push(Articulation::Tremolo(2));
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let attacks = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(attacks.len(), 4);
    for (i, attack) in attacks.iter().enumerate() {
        assert!((attack.time_beats - i as f64 * 0.125).abs() < 1e-9);
    }
    assert!(attacks[3].duration_secs > attacks[0].duration_secs);
}

#[test]
fn tremolo_attached_to_the_second_chord_member_applies_to_the_chord() {
    let xml = score_xml(true, "single");
    let ornament =
        "<notations><ornaments><tremolo type=\"single\">3</tremolo></ornaments></notations>";
    let xml = xml.replacen(ornament, "", 1).replace("<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type>", &format!("<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type>{ornament}"));
    let score = parse_musicxml(&xml).unwrap();
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events.iter().filter(|event| event.pitch_midi == 60).count(),
        16
    );
    assert_eq!(
        events.iter().filter(|event| event.pitch_midi == 64).count(),
        16
    );
}

#[cfg(feature = "mscz")]
#[test]
fn musescore_tremolo_kind_survives_native_roundtrip() {
    for (kind, expected_attacks) in [("r32", 8), ("c32", 1)] {
        let xml = format!(
            r#"<museScore version="4.20"><Score><Part id="1"><Staff id="1"/><trackName>Piano</trackName></Part><Staff id="1"><Measure><voice><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note><Tremolo><subtype>{kind}</subtype></Tremolo></Chord></voice></Measure></Staff></Score></museScore>"#
        );
        let score = acorde_io::parse_mscx(&xml).unwrap();
        assert_eq!(
            to_playback_events(&score, &PlaybackOptions::default()).len(),
            expected_attacks
        );
        let output = acorde_io::serialize_mscx(&score).unwrap();
        assert!(output.contains(&format!("<subtype>{kind}</subtype>")));
        let restored = acorde_io::parse_mscx(&output).unwrap();
        assert_eq!(
            to_playback_events(&restored, &PlaybackOptions::default()).len(),
            expected_attacks
        );
    }
}
