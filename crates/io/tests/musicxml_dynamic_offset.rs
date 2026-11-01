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
