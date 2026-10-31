#![cfg(all(feature = "abc", feature = "mscz"))]

use acorde_core::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, AccordionPresetSelection,
    AccordionSwingPolicy, LayeredAccordionOptions, PositionedAccordionChord, Score,
    arrange_accordion_preset, arrange_from_resolved_preset, arrange_with_layers,
    to_playback_events,
};

fn score(body: &str, meter: &str) -> Score {
    acorde_io::parse_abc(&format!("X:1\nM:{meter}\nL:1/8\nQ:1/4=60\nK:C\n{body}"))
        .expect("ABC parses")
}

fn placed(measure_index: usize, offset_beats: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats,
        chord: serde_json::from_value(serde_json::json!({
            "root": root, "kind": "major", "bass": null, "degrees": []
        }))
        .unwrap(),
    }
}

fn selection(
    mode: AccordionBassMode,
    chords: Vec<PositionedAccordionChord>,
) -> AccordionPresetSelection {
    AccordionPresetSelection {
        chords,
        alternative_chords: None,
        inversion: None,
        bass_mode: mode,
    }
}

fn left_rhythm(score: &Score, measure: usize) -> Vec<(bool, Vec<i16>, f64)> {
    score.parts[0].staves[1].measures[measure].voices[0]
        .iter()
        .map(|note| {
            (
                note.is_rest,
                note.pitches.iter().map(|pitch| pitch.to_midi()).collect(),
                note.beats(),
            )
        })
        .collect()
}

#[test]
fn nine_eight_presets_write_three_dotted_quarter_pulses_in_both_bass_modes() {
    let source = score("C3C3C3|]", "9/8");
    assert_eq!(source.playback_measure_beats(0), 4.5);
    for mode in [AccordionBassMode::StandardBass, AccordionBassMode::FreeBass] {
        for (preset, final_pitches) in [("bass-pulse", vec![36]), ("alternating-bass", vec![43])] {
            let result = arrange_accordion_preset(
                &source,
                selection(mode, vec![placed(0, 0.0, "C")]),
                preset,
            );
            assert!(
                result.diagnostics.is_empty(),
                "{preset} {mode:?}: {:?}",
                result.diagnostics
            );
            let arranged = result.score.as_ref().unwrap();
            assert_eq!(
                left_rhythm(arranged, 0),
                vec![
                    (false, vec![36], 1.5),
                    (false, vec![60, 64, 67], 1.5),
                    (false, final_pitches, 1.5),
                ]
            );
            assert_eq!(result.generated_notes.len(), 3);
            assert!(
                result
                    .generated_notes
                    .iter()
                    .all(|address| address.part == 0 && address.staff == 1 && address.measure == 0)
            );
            let replay = arrange_from_resolved_preset(&source, result.resolved_config.unwrap());
            assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
            assert_eq!(
                left_rhythm(replay.score.as_ref().unwrap(), 0),
                left_rhythm(arranged, 0)
            );
        }
        let offbeat = arrange_accordion_preset(
            &source,
            selection(mode, vec![placed(0, 0.0, "C")]),
            "offbeat-chords",
        );
        assert!(offbeat.diagnostics.is_empty(), "{:?}", offbeat.diagnostics);
        let arranged = offbeat.score.unwrap();
        assert_eq!(
            left_rhythm(&arranged, 0),
            vec![
                (true, vec![], 0.5),
                (false, vec![60, 64, 67], 1.0),
                (true, vec![], 0.5),
                (false, vec![60, 64, 67], 1.0),
                (true, vec![], 0.5),
                (false, vec![60, 64, 67], 1.0),
            ]
        );
        let left_onsets = to_playback_events(&arranged, &Default::default())
            .into_iter()
            .filter(|event| {
                event
                    .source
                    .as_ref()
                    .is_some_and(|address| address.staff == 1)
            })
            .map(|event| event.time_beats)
            .collect::<Vec<_>>();
        assert_eq!(
            left_onsets,
            vec![0.5, 0.5, 0.5, 2.0, 2.0, 2.0, 3.5, 3.5, 3.5]
        );
    }
}

#[test]
fn nine_eight_pickup_short_ending_and_chord_changes_keep_meter_phase() {
    let source = score("C|C3C3C3|C2|]", "9/8");
    assert_eq!(
        (0..3)
            .map(|i| source.playback_measure_beats(i))
            .collect::<Vec<_>>(),
        vec![0.5, 4.5, 1.0]
    );
    let chords = vec![
        placed(0, 0.0, "C"),
        placed(1, 1.5, "G"),
        placed(1, 2.0, "C"),
    ];
    for preset in ["bass-pulse", "alternating-bass", "offbeat-chords"] {
        let result = arrange_accordion_preset(
            &source,
            selection(AccordionBassMode::StandardBass, chords.clone()),
            preset,
        );
        assert!(
            result.diagnostics.is_empty(),
            "{preset}: {:?}",
            result.diagnostics
        );
        let arranged = result.score.as_ref().unwrap();
        for (index, length) in [0.5, 4.5, 1.0].into_iter().enumerate() {
            assert_eq!(
                left_rhythm(arranged, index)
                    .iter()
                    .map(|entry| entry.2)
                    .sum::<f64>(),
                length,
                "{preset} bar {index}"
            );
        }
        assert_eq!(
            arranged.parts[0].staves[0].measures[0].actual_length,
            source.parts[0].staves[0].measures[0].actual_length
        );
        let midi = acorde_io::serialize_midi(arranged).expect("9/8 MIDI exports");
        assert!(!midi.is_empty());
        let mscz = acorde_io::serialize_mscz(arranged).expect("9/8 MSCZ exports");
        let restored = acorde_io::parse_mscz(&mscz).expect("9/8 MSCZ imports");
        assert_eq!(left_rhythm(&restored, 1), left_rhythm(arranged, 1));
    }
    let pulse = arrange_accordion_preset(
        &source,
        selection(AccordionBassMode::StandardBass, chords),
        "bass-pulse",
    );
    let arranged = pulse.score.unwrap();
    assert_eq!(
        left_rhythm(&arranged, 1),
        vec![
            (false, vec![36], 1.5),
            (false, vec![67, 71, 74], 0.5),
            (false, vec![60, 64, 67], 1.0),
            (false, vec![36], 1.5)
        ]
    );
    assert_eq!(left_rhythm(&arranged, 0), vec![(false, vec![36], 0.5)]);
    assert_eq!(left_rhythm(&arranged, 2), vec![(false, vec![36], 1.0)]);
}

#[test]
fn nine_eight_layer_path_and_unsupported_fine_offset_are_explicit() {
    let source = score("C3C3C3|]", "9/8");
    let layered = arrange_with_layers(
        &source,
        LayeredAccordionOptions {
            chords: vec![placed(0, 0.0, "C")],
            alternative_chords: None,
            inversion: None,
            bass_mode: AccordionBassMode::StandardBass,
            accompaniment: Some(AccordionPattern::BassChordPulse),
            melodic_voice: AccordionMelodicVoice::Unchanged,
            swing: AccordionSwingPolicy::Straight,
        },
    );
    assert!(layered.diagnostics.is_empty(), "{:?}", layered.diagnostics);
    assert_eq!(left_rhythm(layered.score.as_ref().unwrap(), 0)[0].2, 1.5);
    let invalid = arrange_accordion_preset(
        &source,
        selection(
            AccordionBassMode::StandardBass,
            vec![placed(0, 0.0, "C"), placed(0, 0.25, "G")],
        ),
        "bass-pulse",
    );
    assert!(invalid.score.is_none());
    assert_eq!(invalid.diagnostics[0].code, "unsupported-rhythm");
    assert!(
        invalid.diagnostics[0]
            .message
            .contains("eighth-note-aligned")
    );
    for (meter, body) in [("6/8", "C3C3|]"), ("12/8", "C3C3C3C3|]")] {
        let score = score(body, meter);
        for preset in ["bass-pulse", "alternating-bass", "offbeat-chords"] {
            let result = arrange_accordion_preset(
                &score,
                selection(AccordionBassMode::StandardBass, vec![placed(0, 0.0, "C")]),
                preset,
            );
            assert!(
                result.diagnostics.is_empty(),
                "{meter} {preset}: {:?}",
                result.diagnostics
            );
        }
    }
}
