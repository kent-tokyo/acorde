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
fn dynamic_during_sustained_note_keeps_beat_through_roundtrip() {
    let xml = score(
        "<direction placement=\"below\"><direction-type><dynamics><p/></dynamics></direction-type><offset>1</offset></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>whole</type></note>",
        1,
    ).replace("<divisions>1</divisions>", "<divisions>2</divisions>");
    let parsed = parse_musicxml(&xml).unwrap();
    let directions = &parsed.parts[0].staves[0].measures[0].timed_dynamics;
    assert_eq!(directions.len(), 1);
    assert_eq!(directions[0].position_beats, 0.5);
    assert_eq!(directions[0].dynamic, acorde_core::Dynamic::P);
    assert_eq!(directions[0].above, Some(false));
    let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].timed_dynamics,
        *directions
    );
}

#[test]
fn coincident_directions_and_note_dynamic_survive_roundtrip() {
    let body = format!(
        "<direction><direction-type><dynamics><p/></dynamics></direction-type></direction><direction placement=\"above\"><direction-type><dynamics><mf/></dynamics></direction-type></direction>{}{}",
        note("C", 4, 1).replace(
            "</note>",
            "<notations><dynamics><sfz/></dynamics></notations></note>"
        ),
        note("D", 4, 1),
    );
    let parsed = parse_musicxml(&score(&body, 1)).unwrap();
    let measure = &parsed.parts[0].staves[0].measures[0];
    assert_eq!(
        measure.voices[0][0].dynamic,
        Some(acorde_core::Dynamic::Sfz)
    );
    let marks = |measure: &acorde_core::Measure| {
        let mut marks = measure
            .timed_dynamics
            .iter()
            .map(|d| d.dynamic.to_musicxml_str())
            .collect::<Vec<_>>();
        marks.sort();
        marks
    };
    assert_eq!(marks(measure), vec!["mf", "p"]);
    let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    assert_eq!(
        marks(&restored.parts[0].staves[0].measures[0]),
        vec!["mf", "p"]
    );
}

#[test]
fn multiple_dynamics_in_one_direction_keep_written_order() {
    let body = format!(
        "<direction><direction-type><dynamics><p/><other-dynamics>second time</other-dynamics><pp/></dynamics></direction-type></direction>{}{}",
        note("C", 4, 1),
        note("D", 4, 1)
    );
    let report = acorde_io::parse_musicxml_with_report(&score(&body, 1)).unwrap();
    use acorde_core::{Dynamic, DynamicDirectionComponent as C};
    let direction = &report.score.parts[0].staves[0].measures[0].timed_dynamics[0];
    assert_eq!(direction.dynamic, Dynamic::Pp);
    assert_eq!(
        direction.components,
        vec![
            C::Dynamic(Dynamic::P),
            C::Text("second time".to_owned()),
            C::Dynamic(Dynamic::Pp)
        ]
    );
    let xml = serialize_musicxml(&report.score).unwrap();
    let restored = parse_musicxml(&xml).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].timed_dynamics[0],
        *direction
    );
    assert_eq!(velocities(&xml)[0].1, Dynamic::Pp.to_velocity());
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

#[test]
fn text_and_rehearsal_in_one_direction_keep_distinct_enclosures() {
    let input = score(
        "<direction><direction-type><words enclosure=\"rectangle\">pizz.</words><rehearsal enclosure=\"oval\">A</rehearsal></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>",
        1,
    );
    let parsed = parse_musicxml(&input).unwrap();
    let texts = &parsed.parts[0].staves[0].measures[0].texts;
    let find = |name: &str| {
        texts
            .iter()
            .find(|text| text.text == name)
            .and_then(|text| text.enclosure.as_deref())
    };
    assert_eq!(find("pizz."), Some("rectangle"));
    assert_eq!(find("A"), Some("oval"));
    let xml = serialize_musicxml(&parsed).unwrap();
    let parsed_again = parse_musicxml(&xml).unwrap();
    let texts = &parsed_again.parts[0].staves[0].measures[0].texts;
    assert!(
        texts
            .iter()
            .any(|text| text.text == "pizz." && text.enclosure.as_deref() == Some("rectangle"))
    );
    assert!(
        texts
            .iter()
            .any(|text| text.text == "A" && text.enclosure.as_deref() == Some("oval"))
    );
}

#[test]
fn multiple_rehearsals_in_one_measure_survive_roundtrip() {
    let input = score(
        r#"<direction placement="above"><direction-type><rehearsal>A</rehearsal></direction-type></direction><direction placement="below"><direction-type><rehearsal enclosure="none">B</rehearsal></direction-type><offset>1</offset></direction><direction><direction-type><rehearsal enclosure="square">A</rehearsal></direction-type><offset>2</offset></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>"#,
        1,
    );
    let parsed = parse_musicxml(&input).unwrap();
    let measure = &parsed.parts[0].staves[0].measures[0];
    assert_eq!(measure.rehearsal.as_deref(), Some("A"));
    let marks: Vec<_> = measure
        .texts
        .iter()
        .filter(|t| t.style == acorde_core::TextStyle::RehearsalMark)
        .cloned()
        .collect();
    assert_eq!(marks.len(), 3);
    assert_eq!(
        marks.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
        ["A", "B", "A"]
    );
    assert_eq!(marks[1].enclosure.as_deref(), Some("none"));
    assert_eq!(marks[1].placement.as_deref(), Some("below"));
    assert_eq!(marks[1].position_beats, Some(1.0));
    assert_eq!(marks[2].enclosure.as_deref(), Some("square"));
    assert_eq!(marks[2].position_beats, Some(2.0));
    let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    let restored_marks: Vec<_> = restored.parts[0].staves[0].measures[0]
        .texts
        .iter()
        .filter(|t| t.style == acorde_core::TextStyle::RehearsalMark)
        .cloned()
        .collect();
    assert_eq!(restored_marks, marks);
}

#[test]
fn rehearsal_components_preserve_text_and_report_format_loss() {
    let input = score(
        r#"<direction><direction-type><rehearsal enclosure="none">Section </rehearsal><rehearsal enclosure="square">A</rehearsal></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>"#,
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&input).unwrap();
    let measure = &report.score.parts[0].staves[0].measures[0];
    assert_eq!(measure.rehearsal.as_deref(), Some("Section A"));
    assert_eq!(measure.texts[0].enclosure.as_deref(), Some("none"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.rehearsal-components-flattened")
    );
    let restored = parse_musicxml(&serialize_musicxml(&report.score).unwrap()).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].rehearsal.as_deref(),
        Some("Section A")
    );
}

#[test]
fn staff_scoped_text_and_rehearsal_roundtrip_on_second_staff() {
    let input = score(
        r#"<direction placement="above"><direction-type><words relative-y="10">arpeggio</words></direction-type><offset>1</offset><staff>2</staff></direction><direction placement="below"><direction-type><rehearsal enclosure="none">B</rehearsal></direction-type><staff>2</staff></direction><direction><direction-type><rehearsal>B</rehearsal></direction-type><offset>1</offset><staff>2</staff></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>1</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff></note>"#,
        2,
    );
    let parsed = parse_musicxml(&input).unwrap();
    let upper = &parsed.parts[0].staves[0].measures[0];
    let lower = &parsed.parts[0].staves[1].measures[0];
    assert!(upper.texts.is_empty());
    assert_eq!(lower.texts.len(), 3);
    assert_eq!(lower.texts[0].position_beats, Some(1.0));
    assert_eq!(lower.texts[0].relative_y, Some(10.0));
    assert_eq!(lower.texts[1].enclosure.as_deref(), Some("none"));
    assert_eq!(lower.texts[2].position_beats, Some(1.0));
    let xml = serialize_musicxml(&parsed).unwrap();
    let restored = parse_musicxml(&xml).unwrap();
    assert!(restored.parts[0].staves[0].measures[0].texts.is_empty());
    assert_eq!(restored.parts[0].staves[1].measures[0].texts, lower.texts);
}
