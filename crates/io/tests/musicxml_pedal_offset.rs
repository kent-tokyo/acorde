use acorde_core::{
    OfflineRenderRequest, OfflineRenderSemanticEventKind, PlaybackOptions,
    build_offline_render_manifest,
};
use acorde_io::{parse_musicxml, parse_musicxml_with_report, serialize_musicxml_with_report};

#[test]
fn pedal_offset_inside_note_is_independent_of_document_order() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><direction><direction-type><pedal type="start" line="no"/></direction-type></direction><direction><direction-type><pedal type="stop" line="no"/></direction-type><offset>3</offset></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("valid offset pedal parses");
    let report = parse_musicxml_with_report(xml).unwrap();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let written = &score.parts[0].staves[0].measures[0].timed_pedals;
    assert_eq!(
        written
            .iter()
            .map(|p| (p.position_beats, p.down))
            .collect::<Vec<_>>(),
        vec![(0.0, true), (3.0, false)]
    );
    let exported = serialize_musicxml_with_report(&score).unwrap();
    assert!(
        exported.diagnostics.is_empty(),
        "{:?}",
        exported.diagnostics
    );
    let reparsed = parse_musicxml(&exported.output).unwrap();
    assert_eq!(
        written,
        &reparsed.parts[0].staves[0].measures[0].timed_pedals
    );
    let options = PlaybackOptions {
        bpm_override: Some(60),
        ..Default::default()
    };
    let request = OfflineRenderRequest {
        sample_rate_hz: 8_000,
        ..Default::default()
    };
    let manifest = build_offline_render_manifest(&score, &options, &request).unwrap();
    let pedal: Vec<_> = manifest
        .semantic_events
        .iter()
        .filter_map(|event| match event.kind {
            OfflineRenderSemanticEventKind::Pedal { down } => Some((event.frame, down)),
            OfflineRenderSemanticEventKind::Controller {
                controller: 64,
                value,
                ..
            } => Some((event.frame, value >= 64)),
            _ => None,
        })
        .collect();
    assert_eq!(pedal, vec![(0, true), (24_000, false)]);
}

#[cfg(feature = "midi")]
#[test]
fn timed_pedal_follows_repeated_measures_in_midi_and_offline_playback() {
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step, TimedPedalDirection};
    let mut score = Score::new("pedal repeat", 60, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.barline_left = Barline::RepeatStart;
    measure.barline_right = Barline::RepeatEnd;
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    measure.timed_pedals = vec![
        TimedPedalDirection {
            position_beats: 0.0,
            down: true,
            number: 1,
            placement: None,
        },
        TimedPedalDirection {
            position_beats: 3.0,
            down: false,
            number: 1,
            placement: None,
        },
    ];
    let midi = acorde_io::serialize_midi(&score).unwrap();
    let decoded = acorde_io::parse_midi(&midi).unwrap();
    let controls: Vec<_> = decoded.parts[0]
        .midi_control_changes
        .iter()
        .filter(|c| c.controller == 64)
        .map(|c| (c.tick, c.value))
        .collect();
    assert_eq!(controls, vec![(0, 127), (1440, 0), (1920, 127), (3360, 0)]);
    let request = OfflineRenderRequest {
        sample_rate_hz: 8_000,
        ..Default::default()
    };
    let manifest =
        build_offline_render_manifest(&score, &PlaybackOptions::default(), &request).unwrap();
    let pedals: Vec<_> = manifest
        .semantic_events
        .iter()
        .filter_map(|e| match e.kind {
            OfflineRenderSemanticEventKind::Pedal { down } => Some((e.frame, down)),
            _ => None,
        })
        .collect();
    assert_eq!(
        pedals,
        vec![(0, true), (24000, false), (32000, true), (56000, false)]
    );
}

#[test]
fn pedal_with_negative_offset_is_assigned_to_its_score_time() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note><direction placement="below"><direction-type><pedal type="stop" line="no" number="2"/></direction-type><offset>-1</offset></direction></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    let event = &score.parts[0].staves[0].measures[0].timed_pedals[0];
    assert_eq!(
        (
            event.position_beats,
            event.down,
            event.number,
            event.placement.as_deref()
        ),
        (3.0, false, 2, Some("below"))
    );
}

#[test]
fn timed_pedals_keep_staff_ownership_and_survive_diff_patch() {
    use acorde_core::{apply_patch, diff, score_patch};
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>2</staves><time><beats>4</beats><beat-type>4</beat-type></time><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><direction><direction-type><pedal type="start" line="no" number="2"/></direction-type><offset>1</offset><staff>2</staff></direction><direction><direction-type><pedal type="stop" line="no" number="2"/></direction-type><offset>3</offset><staff>2</staff></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note><backup><duration>4</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice><type>whole</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    assert!(score.parts[0].staves[0].measures[0].timed_pedals.is_empty());
    let events = &score.parts[0].staves[1].measures[0].timed_pedals;
    assert_eq!(
        events
            .iter()
            .map(|e| (e.position_beats, e.number))
            .collect::<Vec<_>>(),
        vec![(1.0, 2), (3.0, 2)]
    );
    let mut before = score.clone();
    before.parts[0].staves[1].measures[0].timed_pedals.clear();
    assert!(!diff(&before, &score).is_empty());
    let patched = apply_patch(&before, &score_patch(&before, &score)).unwrap();
    assert_eq!(patched.parts[0].staves[1].measures[0].timed_pedals, *events);
    let xml = acorde_io::serialize_musicxml(&score).unwrap();
    let restored = parse_musicxml(&xml).unwrap();
    assert_eq!(
        restored.parts[0].staves[1].measures[0].timed_pedals,
        *events
    );
}
