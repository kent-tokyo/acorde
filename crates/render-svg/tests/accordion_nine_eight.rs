use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, PositionedAccordionChord, arrange_accordion_preset,
};
use acorde_layout::{LayoutConfig, compute_layout};
use acorde_render_svg::{SvgRenderOptions, render_svg, render_svg_metadata};

#[test]
fn nine_eight_generated_bass_notes_have_svg_and_metadata_addresses() {
    let source = acorde_io::parse_abc("X:1\nM:9/8\nL:1/8\nK:C\nC3C3C3|]").unwrap();
    let selection = AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({
                "root":"C", "kind":"major", "bass":null, "degrees":[]
            }))
            .unwrap(),
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    let result = arrange_accordion_preset(&source, selection, "alternating-bass");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let options = SvgRenderOptions {
        width: 700.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    };
    let svg = render_svg(&score, &options).expect("9/8 arrangement renders");
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = render_svg_metadata(&score, &layout, &options).expect("9/8 metadata renders");
    assert_eq!(result.generated_notes.len(), 3);
    for address in &result.generated_notes {
        let marker = format!(
            "data-note-addr=\"{}:{}:{}:{}:{}\"",
            address.part, address.staff, address.measure, address.voice, address.note
        );
        assert!(svg.contains(&marker), "missing {marker}");
        assert!(metadata.address_bounds.iter().any(|bound| {
            bound.part == address.part
                && bound.staff == address.staff
                && bound.measure == address.measure
                && bound.voice == address.voice
                && bound.note == address.note
        }));
    }
}
