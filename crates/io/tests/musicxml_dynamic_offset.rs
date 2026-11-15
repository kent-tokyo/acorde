use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::{parse_musicxml, serialize_musicxml};

fn score(body: &str, staves: u8) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>{staves}</staves><clef><sign>G</sign><line>2</line></clef></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn note(step: &str, octave: u8, staff: u8) -> String {
    format!(
        "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>{staff}</staff></note>"
    )
}
fn velocities(xml: &str) -> Vec<(u8, u8)> {
    let parsed = parse_musicxml(xml).unwrap();
    let mut events: Vec<_> = to_playback_events(&parsed, &PlaybackOptions::default())
        .into_iter()
        .map(|e| (e.pitch_midi, e.velocity))
        .collect();
    events.sort_unstable();
    events
}

#[test]
fn positive_and_negative_offsets_match_ordered_directions() {
    let c = note("C", 4, 1);
    let d = note("D", 4, 1);
    let mark = "<direction><direction-type><dynamics><p/></dynamics></direction-type>";
    let forward = score(&format!("{mark}<offset>1</offset></direction>{c}{d}"), 1);
    let ordered = score(&format!("{c}{mark}</direction>{d}"), 1);
    let backward = score(&format!("{c}{d}{mark}<offset>-1</offset></direction>"), 1);
    let expected = vec![(60, 64), (62, 48)];
    assert_eq!(velocities(&forward), expected);
    assert_eq!(velocities(&ordered), expected);
    assert_eq!(velocities(&backward), expected);
    let exported = serialize_musicxml(&parse_musicxml(&forward).unwrap()).unwrap();
    assert_eq!(velocities(&exported), expected);
}

#[test]
fn backup_and_offset_remain_staff_scoped() {
    let upper = format!("{}{}", note("C", 4, 1), note("D", 4, 1));
    let lower = format!("{}{}", note("C", 3, 2), note("D", 3, 2));
    let body = format!(
        "{upper}<backup><duration>2</duration></backup><direction><direction-type><dynamics><p/></dynamics></direction-type><offset>1</offset><staff>2</staff></direction>{lower}"
    );
    assert_eq!(
        velocities(&score(&body, 2)),
        vec![(48, 64), (50, 48), (60, 64), (62, 64)]
    );
}

#[test]
fn offset_can_target_next_or_previous_measure() {
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><direction><direction-type><dynamics><p/></dynamics></direction-type><offset>2</offset></direction>{}{}</measure><measure number="2"><direction><direction-type><dynamics><f/></dynamics></direction-type><offset>-1</offset></direction>{}{}</measure></part></score-partwise>"#,
        note("C", 4, 1),
        note("D", 4, 1),
        note("E", 4, 1),
        note("F", 4, 1)
    );
    assert_eq!(
        velocities(&xml),
        vec![(60, 64), (62, 84), (64, 48), (65, 48)]
    );
}

#[test]
fn dynamic_placement_survives_offsets_inline_notation_and_roundtrip() {
    for (placement, expected) in [("above", Some(true)), ("below", Some(false)), ("", None)] {
        let attribute = if placement.is_empty() {
            String::new()
        } else {
            format!(" placement=\"{placement}\"")
        };
        let body = format!(
            "<direction{attribute}><direction-type><dynamics placement=\"below\"><p/></dynamics></direction-type><offset>1</offset></direction>{}{}",
            note("C", 4, 1),
            note("D", 4, 1)
        );
        let parsed = parse_musicxml(&score(&body, 1)).unwrap();
        let notes = &parsed.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[0].dynamic, None);
        assert_eq!(notes[1].dynamic_above, expected);
        let reparsed = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
        assert_eq!(
            reparsed.parts[0].staves[0].measures[0].voices[0][1].dynamic_above,
            expected
        );

        let inline = note("C", 4, 1).replace(
            "</note>",
            &format!("<notations><dynamics{attribute}><f/></dynamics></notations></note>"),
        );
        let parsed = parse_musicxml(&score(&format!("{inline}{}", note("D", 4, 1)), 1)).unwrap();
        let notes = &parsed.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[0].dynamic, Some(acorde_core::Dynamic::F));
        assert_eq!(notes[0].dynamic_above, expected);
        assert_eq!(notes[1].dynamic_above, None);
        let reparsed = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
        assert_eq!(
            reparsed.parts[0].staves[0].measures[0].voices[0][0].dynamic_above,
            expected
        );
    }
}

#[test]
fn directions_target_the_written_staff_of_a_cross_staff_beam() {
    let upper = note("C", 4, 1)
        .replace("</note>", "<beam number=\"1\">begin</beam></note>")
        .replace("<type>quarter</type>", "<type>eighth</type>");
    let lower = note("D", 3, 2)
        .replace("</note>", "<beam number=\"1\">end</beam></note>")
        .replace("<type>quarter</type>", "<type>eighth</type>");
    let xml = score(&format!("{upper}<direction placement=\"above\"><direction-type><dynamics><p/></dynamics></direction-type><staff>2</staff></direction>{lower}"), 2)
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>");
    let parsed = parse_musicxml(&xml).unwrap();
    let marked: Vec<_> = parsed.parts[0]
        .staves
        .iter()
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .filter(|n| n.dynamic.is_some())
        .collect();
    assert_eq!(marked.len(), 1);
    assert_eq!(marked[0].pitches[0].step, acorde_core::Step::D);
    assert_eq!(marked[0].dynamic_above, Some(true));
    assert_eq!(marked[0].cross_staff.as_ref().unwrap().target_staff, 1);
    let reparsed = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    let marked: Vec<_> = reparsed.parts[0]
        .staves
        .iter()
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .filter(|n| n.dynamic.is_some())
        .collect();
    assert_eq!(marked.len(), 1);
    assert_eq!(marked[0].pitches[0].step, acorde_core::Step::D);
    assert_eq!(marked[0].dynamic_above, Some(true));
}
