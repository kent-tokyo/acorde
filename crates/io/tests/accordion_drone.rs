#![cfg(feature = "abc")]

use acorde_core::{
    AccordionBassMode, AccordionDroneOptions, AccordionMelodicVoice, AccordionPattern,
    AccordionSwingPolicy, LayeredAccordionOptions, PositionedAccordionChord, Score,
    arrange_with_drone, arrange_with_layers, to_playback_events,
};

fn score(body: &str, meter: &str) -> Score {
    acorde_io::parse_abc(&format!("X:1\nM:{meter}\nL:1/4\nQ:1/4=60\nK:C\n{body}"))
        .expect("ABC parses")
}

fn options(pitch_midi: u8) -> AccordionDroneOptions {
    AccordionDroneOptions {
        pitch_midi,
        bass_mode: AccordionBassMode::StandardBass,
        layers: None,
    }
}

fn chord(measure_index: usize, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats: 0.0,
        chord: serde_json::from_value(serde_json::json!({
            "root": root, "kind": "major", "bass": null, "degrees": []
        }))
        .unwrap(),
    }
}

fn drone_events(score: &Score, pitch: u8) -> Vec<(u64, u64)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| event.pitch_midi == pitch)
        .map(|event| (event.time_beats.to_bits(), event.duration_beats.to_bits()))
        .collect()
}

#[test]
fn explicit_drone_needs_no_chords_and_ignores_harmony_changes() {
    let source = score("c4|d4|]", "4/4");
    let first = arrange_with_drone(&source, options(36));
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    let arranged = first.score.as_ref().unwrap();
    assert_eq!(first.generated_notes.len(), 2);
    assert_eq!(drone_events(arranged, 36).len(), 2);
    assert_eq!(drone_events(arranged, 36)[0].0, 0.0_f64.to_bits());
    assert_eq!(drone_events(arranged, 36)[1].0, 4.0_f64.to_bits());
    assert_eq!(
        serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
        serde_json::to_value(&source.parts[0].staves[0]).unwrap()
    );

    let mut layered = options(36);
    layered.layers = Some(LayeredAccordionOptions {
        chords: vec![chord(0, "C"), chord(1, "G")],
        alternative_chords: None,
        inversion: None,
        accompaniment: None,
        melodic_voice: AccordionMelodicVoice::OctaveBelow,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::StandardBass,
    });
    let combined = arrange_with_drone(&source, layered.clone());
    assert!(
        combined.diagnostics.is_empty(),
        "{:?}",
        combined.diagnostics
    );
    let combined_score = combined.score.as_ref().unwrap();
    assert_eq!(drone_events(combined_score, 36), drone_events(arranged, 36));
    assert!(!combined_score.parts[0].staves[0].measures[0].voices[1].is_empty());
    let replay_options =
        serde_json::from_str(&serde_json::to_string(&combined.resolved_options).unwrap()).unwrap();
    let replay = arrange_with_drone(&score("c4|d4|]", "4/4"), replay_options);
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert_eq!(
        drone_events(combined_score, 36),
        drone_events(replay.score.as_ref().unwrap(), 36)
    );

    let held = arrange_with_layers(
        &source,
        LayeredAccordionOptions {
            accompaniment: Some(AccordionPattern::HeldBassChord),
            ..layered.layers.unwrap()
        },
    );
    assert!(
        held.score.unwrap().parts[0].staves[1].measures[0].voices[0][0]
            .pitches
            .len()
            > 1
    );
}

#[test]
fn pickup_rest_short_final_and_repeats_follow_source_barlines() {
    let source = score("c|z3|c2|]", "3/4");
    let result = arrange_with_drone(&source, options(36));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let arranged = result.score.unwrap();
    assert_eq!(
        (0..3)
            .map(|i| arranged.playback_measure_beats(i))
            .collect::<Vec<_>>(),
        vec![1.0, 3.0, 2.0]
    );
    assert_eq!(drone_events(&arranged, 36).len(), 3);
    assert!(arranged.parts[0].staves[0].measures[1].voices[0][0].is_rest);
    for measure in &arranged.parts[0].staves[1].measures {
        assert!(!measure.voices[0][0].tie_end);
        assert!(!measure.voices[0].last().unwrap().tie_start);
    }
    assert!(arranged.parts[0].staves[1].measures[1].voices[0][0].tie_start);
    assert!(arranged.parts[0].staves[1].measures[1].voices[0][1].tie_end);

    let repeated = score("|:c4|d4:|", "4/4");
    let result = arrange_with_drone(&repeated, options(36));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(drone_events(result.score.as_ref().unwrap(), 36).len(), 4);
}

#[test]
fn bass_mode_range_and_incompatible_layers_are_diagnosed() {
    let source = score("c4|]", "4/4");
    assert_eq!(
        arrange_with_drone(&source, options(48)).diagnostics[0].code,
        "out-of-range"
    );
    let mut free = options(48);
    free.bass_mode = AccordionBassMode::FreeBass;
    assert!(arrange_with_drone(&source, free).diagnostics.is_empty());
    let mut incompatible = options(36);
    incompatible.layers = Some(LayeredAccordionOptions {
        chords: vec![chord(0, "C")],
        alternative_chords: None,
        inversion: None,
        accompaniment: Some(AccordionPattern::HeldBassChord),
        melodic_voice: AccordionMelodicVoice::Unchanged,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::StandardBass,
    });
    assert_eq!(
        arrange_with_drone(&source, incompatible).diagnostics[0].code,
        "invalid-layers"
    );
}
