use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, PositionedAccordionChord,
    arrange_accordion_preset, to_playback_events,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};

#[test]
fn two_four_patterns_render_and_mscz_preserves_playback() {
    let source = acorde_io::parse_abc("X:1\nM:2/4\nL:1/4\nQ:1/4=60\nK:C\nc2|c2|]\n").unwrap();
    let chord =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major","bass":null})).unwrap();
    let selection = AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord,
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    for id in ["fine-bass-chord", "habanera"] {
        let result = arrange_accordion_preset(&source, selection.clone(), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let score = result.score.unwrap();
        let svg = render_svg(&score, &SvgRenderOptions::default()).unwrap();
        assert!(svg.matches("acorde-notehead").count() >= 10);
        let bytes = acorde_io::serialize_mscz(&score).unwrap();
        let restored = acorde_io::parse_mscz(&bytes).unwrap();
        let events = |score: &acorde_core::Score| {
            to_playback_events(score, &Default::default())
                .iter()
                .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
                .collect::<Vec<_>>()
        };
        assert_eq!(events(&score), events(&restored), "{id}");
    }
}

#[test]
fn generated_svg_positions_follow_notated_two_four_rhythm() {
    let source = acorde_io::parse_abc("X:1\nM:2/4\nL:1/4\nK:C\nc2|]\n").unwrap();
    let chord =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major","bass":null})).unwrap();
    let selection = AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord,
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    for (id, expected_ratios) in [
        ("fine-bass-chord", [1.0, 1.0, 1.0]),
        ("habanera", [3.0, 1.0, 2.0]),
    ] {
        let result = arrange_accordion_preset(&source, selection.clone(), id);
        let svg = render_svg(
            &result.score.unwrap(),
            &SvgRenderOptions {
                interactive: true,
                ..SvgRenderOptions::default()
            },
        )
        .unwrap();
        let x = (0..4)
            .map(|index| {
                let marker = format!("data-note-addr=\"0:1:0:0:{index}\"");
                let element = svg.split(&marker).nth(1).expect("interactive note address");
                element
                    .split("class=\"acorde-notehead\" cx=\"")
                    .nth(1)
                    .expect("notehead")
                    .split('"')
                    .next()
                    .unwrap()
                    .parse::<f32>()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let steps = [x[1] - x[0], x[2] - x[1], x[3] - x[2]];
        let unit = steps[1] / expected_ratios[1];
        for (actual, ratio) in steps.into_iter().zip(expected_ratios) {
            assert!((actual / unit - ratio).abs() < 0.02, "{id}: {x:?}");
        }
    }
}
