#![cfg(feature = "abc")]

use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, PositionedAccordionChord,
    arrange_accordion_preset, to_playback_events,
};

#[test]
fn abc_six_eight_half_beat_pickups_arrange_without_moving_the_melody() {
    for (body, pickup_beats) in [("A|D6|D6|]", 0.5), ("A3|D6|D6|]", 1.5)] {
        let abc = format!("X:1\nM:6/8\nL:1/8\nK:D\n{body}");
        let score = acorde_io::parse_abc(&abc).expect("ABC pickup");
        assert_eq!(score.playback_measure_beats(0), pickup_beats);
        let source = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
        let selection = AccordionPresetSelection {
            chords: vec![PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: serde_json::from_value(serde_json::json!({
                    "root": "D", "kind": "major", "bass": null
                }))
                .unwrap(),
            }],
            alternative_chords: None,
            inversion: None,
            bass_mode: AccordionBassMode::FreeBass,
        };
        for preset in ["bass-pulse", "offbeat-chords"] {
            let result = arrange_accordion_preset(&score, selection.clone(), preset);
            assert!(
                result.diagnostics.is_empty(),
                "{body} {preset}: {:?}",
                result.diagnostics
            );
            let arranged = result.score.expect("arranged score");
            assert_eq!(
                serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
                source
            );
            assert_eq!(arranged.playback_measure_beats(0), pickup_beats);
            let accompaniment = &arranged.parts[0].staves[1].measures;
            assert_eq!(
                accompaniment[0].voices[0]
                    .iter()
                    .map(|note| note.beats())
                    .sum::<f64>(),
                pickup_beats
            );
            assert_eq!(
                accompaniment[1].voices[0]
                    .iter()
                    .map(|note| note.beats())
                    .sum::<f64>(),
                3.0
            );
            let first_full_bar_attack = to_playback_events(&arranged, &Default::default())
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|source| source.staff == 1 && source.measure == 1)
                })
                .map(|event| event.time_beats)
                .min_by(f64::total_cmp)
                .expect("first full bar accompaniment");
            let expected = if preset == "bass-pulse" {
                pickup_beats
            } else {
                pickup_beats + 0.5
            };
            assert_eq!(first_full_bar_attack, expected);
        }
    }
}

#[test]
fn shorter_than_half_beat_pickup_keeps_an_explicit_diagnostic() {
    let score = acorde_io::parse_abc("X:1\nM:6/8\nL:1/8\nK:D\nA/|D6|]").expect("sixteenth pickup");
    assert_eq!(score.playback_measure_beats(0), 0.25);
    let selection = AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({
                "root": "D", "kind": "major", "bass": null
            }))
            .unwrap(),
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::FreeBass,
    };
    for preset in ["bass-pulse", "offbeat-chords"] {
        let result = arrange_accordion_preset(&score, selection.clone(), preset);
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "unsupported-rhythm");
        assert_eq!(result.diagnostics[0].measure_index, Some(0));
    }
}
