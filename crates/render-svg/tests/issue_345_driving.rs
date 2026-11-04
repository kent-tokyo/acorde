use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, PositionedAccordionChord, Score,
    arrange_accordion_preset, to_playback_events,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};

#[test]
fn driving_expression_and_timing_survive_mscz_and_svg() {
    let score = acorde_io::parse_abc("X:1\nM:4/4\nL:1/4\nQ:1/4=120\nK:C\nc4|]\n").unwrap();
    let chord = |root| {
        serde_json::from_value(serde_json::json!({"root":root,"kind":"major","bass":null})).unwrap()
    };
    let selection = AccordionPresetSelection {
        chords: vec![
            PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: chord("C"),
            },
            PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 2.0,
                chord: chord("G"),
            },
        ],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    for id in ["driving-bass-chord", "driving-bass-chord-light"] {
        let result = arrange_accordion_preset(&score, selection.clone(), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        let svg = render_svg(
            &output,
            &SvgRenderOptions {
                interactive: true,
                ..SvgRenderOptions::default()
            },
        )
        .unwrap();
        let notes = &output.parts[0].staves[1].measures[0].voices[0];
        let x = (0..notes.len())
            .map(|index| {
                let marker = format!("data-note-addr=\"0:1:0:0:{index}\"");
                svg.split(&marker)
                    .nth(1)
                    .expect("note address")
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
        assert!(x.windows(2).all(|w| w[1] > w[0]), "{id}: {x:?}");
        if id == "driving-bass-chord" {
            assert!(svg.contains("acorde-articulation"));
        }
        let bytes = acorde_io::serialize_mscz(&output).unwrap();
        let restored = acorde_io::parse_mscz(&bytes).unwrap();
        let events = |s: &Score| {
            to_playback_events(s, &Default::default())
                .iter()
                .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats, e.velocity))
                .collect::<Vec<_>>()
        };
        assert_eq!(events(&output), events(&restored), "{id}");
    }
}
