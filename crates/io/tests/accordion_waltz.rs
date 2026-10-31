#![cfg(feature = "abc")]

use acorde_core::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, AccordionPresetSelection,
    AccordionSwingPolicy, LayeredAccordionOptions, PositionedAccordionChord, Score,
    accordion_preset_capabilities, arrange_accordion_preset, arrange_from_resolved_preset,
    arrange_with_layers, to_playback_events,
};

fn score(body: &str, meter: &str) -> Score {
    acorde_io::parse_abc(&format!("X:1\nM:{meter}\nL:1/4\nQ:1/4=60\nK:C\n{body}"))
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

fn selection(chords: Vec<PositionedAccordionChord>) -> AccordionPresetSelection {
    AccordionPresetSelection {
        chords,
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    }
}

fn bar_pitches(score: &Score, bar: usize) -> Vec<Vec<i16>> {
    score.parts[0].staves[1].measures[bar].voices[0]
        .iter()
        .map(|note| note.pitches.iter().map(|pitch| pitch.to_midi()).collect())
        .collect()
}

fn signature(score: &Score) -> Vec<(u8, u64, u64)> {
    to_playback_events(score, &Default::default())
        .iter()
        .map(|event| {
            (
                event.pitch_midi,
                event.time_beats.to_bits(),
                event.duration_beats.to_bits(),
            )
        })
        .collect()
}

#[test]
fn two_full_bars_have_bass_chord_chord_with_optional_alternating_bass() {
    let source = score("c3|c3|]", "3/4");
    let capabilities = accordion_preset_capabilities();
    for (id, second_bass) in [("waltz-bass", 36), ("waltz-alternating-bass", 43)] {
        assert!(capabilities.presets.iter().any(|preset| preset.id == id));
        let result = arrange_accordion_preset(&source, selection(vec![placed(0, 0.0, "C")]), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.as_ref().unwrap();
        assert_eq!(
            bar_pitches(arranged, 0),
            vec![vec![36], vec![60, 64, 67], vec![60, 64, 67]]
        );
        assert_eq!(
            bar_pitches(arranged, 1),
            vec![vec![second_bass], vec![60, 64, 67], vec![60, 64, 67]]
        );
        assert!(
            arranged.parts[0].staves[1]
                .measures
                .iter()
                .flat_map(|bar| &bar.voices[0])
                .all(|note| note.beats() == 1.0)
        );

        let replay = arrange_from_resolved_preset(
            &score("c3|c3|]", "3/4"),
            serde_json::from_str(&serde_json::to_string(&result.resolved_config).unwrap()).unwrap(),
        );
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(
            signature(arranged),
            signature(replay.score.as_ref().unwrap())
        );
    }
    // Existing four-beat patterns retain their established third-beat bass.
    let old = arrange_accordion_preset(&source, selection(vec![placed(0, 0.0, "C")]), "bass-pulse");
    assert_eq!(bar_pitches(old.score.as_ref().unwrap(), 0)[2], vec![36]);
}

#[test]
fn chord_changes_keep_beat_roles_and_layers_add_a_voice() {
    let source = score("c3|]", "3/4");
    let chords = vec![
        placed(0, 0.0, "C"),
        placed(0, 1.0, "G"),
        placed(0, 2.0, "G"),
    ];
    let preset = arrange_accordion_preset(&source, selection(chords.clone()), "waltz-bass");
    assert!(preset.diagnostics.is_empty(), "{:?}", preset.diagnostics);
    assert_eq!(
        bar_pitches(preset.score.as_ref().unwrap(), 0),
        vec![vec![36], vec![67, 71, 74], vec![67, 71, 74]]
    );
    let options = LayeredAccordionOptions {
        chords,
        alternative_chords: None,
        inversion: None,
        accompaniment: Some(AccordionPattern::WaltzBassChord),
        melodic_voice: AccordionMelodicVoice::SixthBelow,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::StandardBass,
    };
    let layered = arrange_with_layers(&source, options);
    assert!(layered.diagnostics.is_empty(), "{:?}", layered.diagnostics);
    assert!(layered.generated_notes.len() > preset.generated_notes.len());
}

#[test]
fn pickup_short_final_and_repeats_preserve_source_lengths_and_playback() {
    let source = score("c|c3|c2|]", "3/4");
    assert_eq!(
        (0..3)
            .map(|bar| source.playback_measure_beats(bar))
            .collect::<Vec<_>>(),
        vec![1.0, 3.0, 2.0]
    );
    let original = serde_json::to_value(&source.parts[0].staves[0]).unwrap();
    for (id, last_bass) in [("waltz-bass", 36), ("waltz-alternating-bass", 43)] {
        let result = arrange_accordion_preset(&source, selection(vec![placed(0, 0.0, "C")]), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.as_ref().unwrap();
        assert_eq!(
            serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
            original
        );
        assert_eq!(bar_pitches(arranged, 0), vec![vec![60, 64, 67]]);
        assert_eq!(bar_pitches(arranged, 1)[0], vec![36]);
        assert_eq!(
            bar_pitches(arranged, 2),
            vec![vec![last_bass], vec![60, 64, 67]]
        );
        assert_eq!(
            (0..3)
                .map(|bar| arranged.playback_measure_beats(bar))
                .collect::<Vec<_>>(),
            vec![1.0, 3.0, 2.0]
        );
    }

    let repeated = score("|:c3|c3:|", "3/4");
    let result = arrange_accordion_preset(
        &repeated,
        selection(vec![placed(0, 0.0, "C")]),
        "waltz-alternating-bass",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let arranged = result.score.unwrap();
    assert_eq!(bar_pitches(&arranged, 0)[0], vec![36]);
    assert_eq!(bar_pitches(&arranged, 1)[0], vec![43]);
    assert_eq!(
        arranged.parts[0].staves[0].measures[0].barline_left,
        repeated.parts[0].staves[0].measures[0].barline_left
    );
    assert_eq!(
        arranged.parts[0].staves[0].measures[1].barline_right,
        repeated.parts[0].staves[0].measures[1].barline_right
    );
    assert!(
        to_playback_events(&arranged, &Default::default()).len()
            > to_playback_events(&repeated, &Default::default()).len()
    );
}

#[test]
fn unsupported_meter_and_chord_boundaries_have_diagnostics() {
    let four_four = score("c4|]", "4/4");
    let result = arrange_accordion_preset(
        &four_four,
        selection(vec![placed(0, 0.0, "C")]),
        "waltz-bass",
    );
    assert_eq!(result.diagnostics[0].code, "unsupported-meter");
    assert!(result.score.is_none());

    let three_four = score("c3|]", "3/4");
    let result = arrange_accordion_preset(
        &three_four,
        selection(vec![placed(0, 0.0, "C"), placed(0, 0.25, "G")]),
        "waltz-bass",
    );
    assert_eq!(result.diagnostics[0].code, "unsupported-rhythm");
    assert!(result.score.is_none());
}
