#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use acorde_core::{AccordionPlanResult, Barline, Score};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const SOURCE: &str = include_str!("../../../tests/fixtures/arrangement_plan_eight_bars.abc");
const PLAN: &str = include_str!("../../../tests/fixtures/arrangement_plan_eight_bars.json");

fn events(score: &Score) -> serde_json::Value {
    serde_json::to_value(acorde_core::to_playback_events(score, &Default::default())).unwrap()
}

fn musical_events(score: &Score) -> Vec<(u8, f64, f64, f64, f64, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.pitch_midi,
                e.time_beats,
                e.duration_beats,
                e.time_secs,
                e.duration_secs,
                e.velocity,
            )
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn explicit_plan_preserves_melody_replays_renders_and_roundtrips() {
    let input = acorde_wasm::parse_abc(SOURCE).unwrap();
    let score: Score = serde_json::from_str(&input).unwrap();
    let web: AccordionPlanResult =
        serde_json::from_str(&acorde_wasm::arrange_accordion_plan(&input, PLAN).unwrap()).unwrap();
    let native = acorde_core::arrange_accordion_plan(&score, serde_json::from_str(PLAN).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    let output = web.score.as_ref().unwrap();
    assert_eq!(web.generated_notes, native.generated_notes);
    assert_eq!(web.resolved_config, native.resolved_config);
    assert_eq!(events(output), events(native.score.as_ref().unwrap()));
    assert_eq!(
        serde_json::to_value(&score).unwrap(),
        serde_json::from_str::<serde_json::Value>(&input).unwrap()
    );
    for (mi, bar) in score.parts[0].staves[0].measures.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&bar.voices[0]).unwrap(),
            serde_json::to_value(&output.parts[0].staves[0].measures[mi].voices[0]).unwrap()
        );
        assert_eq!(
            output.parts[0].staves[0].measures[mi].voices[1].is_empty(),
            !(4..7).contains(&mi)
        );
    }
    let saved = serde_json::to_string(web.resolved_config.as_ref().unwrap()).unwrap();
    let reparsed = acorde_wasm::parse_abc(SOURCE).unwrap();
    let replay: AccordionPlanResult =
        serde_json::from_str(&acorde_wasm::arrange_from_resolved_plan(&reparsed, &saved).unwrap())
            .unwrap();
    assert_eq!(web.generated_notes, replay.generated_notes);
    assert_eq!(events(output), events(replay.score.as_ref().unwrap()));
    let output_json = serde_json::to_string(output).unwrap();
    let svg = acorde_wasm::render_score_svg(&output_json, r#"{"interactive":true}"#).unwrap();
    assert!(svg.contains("data-note-addr=\"0:0:4:1:0\""));
    assert!(!svg.contains("data-note-addr=\"0:0:3:1:0\""));
    assert!(svg.contains("acorde-articulation"));
    let playback: serde_json::Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output_json, "{}").unwrap())
            .unwrap();
    assert_eq!(playback, events(output));
    let archive = acorde_wasm::serialize_mscz(&output_json).unwrap();
    let restored: Score =
        serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    assert_eq!(musical_events(output), musical_events(&restored));
    for mi in 0..8 {
        let original = &score.parts[0].staves[0].measures[mi].voices[0];
        let restored = &restored.parts[0].staves[0].measures[mi].voices[0];
        assert_eq!(
            original
                .iter()
                .map(|n| (&n.pitches, n.beats()))
                .collect::<Vec<_>>(),
            restored
                .iter()
                .map(|n| (&n.pitches, n.beats()))
                .collect::<Vec<_>>()
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn repeated_written_sections_and_endings_keep_absolute_playback_times() {
    use acorde_core::model::score::{NotationSpanner, NotationSpannerKind, NoteAddr, VoltaBracket};
    let mut score: Score = serde_json::from_str(&acorde_wasm::parse_abc(SOURCE).unwrap()).unwrap();
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[6].barline_right = Barline::RepeatEnd;
    bars[6].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    bars[7].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    bars[4].tempo = Some(150);
    score.spanners.push(NotationSpanner {
        id: "source-slur".into(),
        kind: NotationSpannerKind::Slur,
        start: NoteAddr {
            part: 0,
            staff: 0,
            measure: 3,
            voice: 0,
            note: 0,
        },
        end: NoteAddr {
            part: 0,
            staff: 0,
            measure: 4,
            voice: 0,
            note: 0,
        },
        number: None,
        line_type: None,
        text: None,
        placement: None,
        ottava_size: None,
        ottava_type: None,
    });
    let input = serde_json::to_string(&score).unwrap();
    let result: AccordionPlanResult =
        serde_json::from_str(&acorde_wasm::arrange_accordion_plan(&input, PLAN).unwrap()).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(output.spanners, score.spanners);
    let sequence = acorde_core::model::repeat::measure_sequence(&output);
    assert_eq!(sequence, vec![0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 7]);
    let playback = acorde_core::to_playback_events(&output, &Default::default());
    let starts: Vec<_> = playback
        .iter()
        .filter(|e| e.address.as_deref() == Some("0:0:4:1:0"))
        .map(|e| e.time_beats)
        .collect();
    assert_eq!(starts, [16.0, 44.0]);
    assert!(
        playback
            .iter()
            .any(|e| e.address.as_deref() == Some("0:0:7:0:0") && e.time_beats == 52.0)
    );
    let melody = |s: &Score| {
        acorde_core::to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| {
                e.source
                    .as_ref()
                    .is_some_and(|a| a.staff == 0 && a.voice == 0)
            })
            .map(|e| {
                (
                    e.pitch_midi,
                    e.time_beats,
                    e.duration_beats,
                    e.time_secs,
                    e.duration_secs,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(melody(&score), melody(&output));
    let output_json = serde_json::to_string(&output).unwrap();
    assert!(
        acorde_wasm::render_score_svg(&output_json, "{}")
            .unwrap()
            .contains("<svg")
    );
    let archive = acorde_wasm::serialize_mscz(&output_json).unwrap();
    let restored: Score =
        serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    assert_eq!(
        acorde_core::model::repeat::measure_sequence(&restored),
        sequence
    );
    assert_eq!(musical_events(&output), musical_events(&restored));
}
