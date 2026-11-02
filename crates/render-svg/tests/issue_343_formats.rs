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
