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
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.timed-pedal-notation-unsupported")
    );
    assert!(
        serialize_musicxml_with_report(&score)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.export-unsupported-midi-control-change")
    );
    let controls = &score.parts[0].midi_control_changes;
    assert_eq!(
        controls
            .iter()
            .map(|c| (c.tick, c.controller, c.value))
            .collect::<Vec<_>>(),
        vec![(0, 64, 127), (1440, 64, 0)]
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
