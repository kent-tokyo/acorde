#![cfg(feature = "abc")]

use acorde_core::{
    AccordionBassMode, AccordionLayerShuffleOptions, AccordionMelodicVoice, AccordionPattern,
    AccordionPresetSelection, AccordionPreviousConfig, AccordionSwingPolicy,
    LayeredAccordionOptions, PositionedAccordionChord, Score, arrange_accordion_layers,
    arrange_accordion_preset, arrange_from_resolved_layers, arrange_from_resolved_preset,
    shuffle_accordion_layers, to_playback_events,
};

const ABC: &str = "X:1\nM:4/4\nL:1/4\nQ:1/4=60\nK:C\nc4|";

fn score() -> Score {
    acorde_io::parse_abc(ABC).expect("ABC parses")
}

fn selection() -> AccordionPresetSelection {
    AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({
                "root": "C", "kind": "major", "bass": null, "degrees": []
            }))
            .unwrap(),
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::FreeBass,
    }
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

fn request() -> AccordionLayerShuffleOptions {
    AccordionLayerShuffleOptions {
        selection: selection(),
        alternative_selections: Vec::new(),
        allowed_accompaniments: vec![
            Some(AccordionPattern::HeldBassChord),
            Some(AccordionPattern::RisingArpeggio),
        ],
        allowed_melodic_voices: vec![
            AccordionMelodicVoice::SixthBelow,
            AccordionMelodicVoice::OctaveBelow,
        ],
        allowed_swing_policies: vec![AccordionSwingPolicy::Straight],
        seed: "18446744073709551615".into(),
        previous_config: None,
    }
}

#[test]
fn free_layer_combinations_replay_after_reparsing_the_same_abc() {
    let first = score();
    let reparsed = score();
    for (preset_id, voice) in [
        ("held-chord", AccordionMelodicVoice::SixthBelow),
        ("rising-arpeggio", AccordionMelodicVoice::OctaveBelow),
    ] {
        let preset = arrange_accordion_preset(&first, selection(), preset_id);
        let mut altered_preset = preset.resolved_config.expect("preset config");
        altered_preset.options.melodic_voice = voice;
        assert_eq!(
            arrange_from_resolved_preset(&first, altered_preset.clone()).diagnostics[0].code,
            "invalid-configuration"
        );

        let layered = arrange_accordion_layers(&first, altered_preset.options);
        assert!(layered.diagnostics.is_empty(), "{:?}", layered.diagnostics);
        assert!(!layered.source_notes.is_empty());
        assert!(!layered.generated_notes.is_empty());
        assert!(!layered.resolved_chords.is_empty());
        let config_json = serde_json::to_string(&layered.resolved_config).unwrap();
        let config = serde_json::from_str(&config_json).unwrap();
        let replay = arrange_from_resolved_layers(&reparsed, config);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(
            signature(layered.score.as_ref().unwrap()),
            signature(replay.score.as_ref().unwrap())
        );
    }
}

#[test]
fn free_layer_shuffle_is_reproducible_and_excludes_prior_music() {
    let first = score();
    let reparsed = score();
    let shuffle_request = request();
    let one = shuffle_accordion_layers(&first, shuffle_request.clone());
    let two = shuffle_accordion_layers(&reparsed, shuffle_request.clone());
    assert!(one.diagnostics.is_empty(), "{:?}", one.diagnostics);
    assert_eq!(one.resolved_config, two.resolved_config);
    assert_eq!(one.seed.as_deref(), Some(shuffle_request.seed.as_str()));
    let previous = one.resolved_config.clone().unwrap();
    let mut next_request = shuffle_request;
    next_request.previous_config = Some(AccordionPreviousConfig::Layers(previous.clone()));
    let next = shuffle_accordion_layers(&reparsed, next_request);
    assert!(next.diagnostics.is_empty(), "{:?}", next.diagnostics);
    assert_ne!(
        signature(one.score.as_ref().unwrap()),
        signature(next.score.as_ref().unwrap())
    );

    let mut only_previous = request();
    only_previous.allowed_accompaniments = vec![previous.options.accompaniment];
    only_previous.allowed_melodic_voices = vec![previous.options.melodic_voice];
    only_previous.allowed_swing_policies = vec![previous.options.swing];
    only_previous.previous_config = Some(AccordionPreviousConfig::Layers(previous));
    assert_eq!(
        shuffle_accordion_layers(&first, only_previous).diagnostics[0].code,
        "no-different-result"
    );
}

#[test]
fn free_layer_shuffle_compares_fixed_preset_music_and_reports_failures() {
    let score = score();
    let preset = arrange_accordion_preset(&score, selection(), "held-chord");
    let mut same_as_preset = request();
    same_as_preset.allowed_accompaniments = vec![Some(AccordionPattern::HeldBassChord)];
    same_as_preset.allowed_melodic_voices = vec![AccordionMelodicVoice::Unchanged];
    same_as_preset.previous_config = Some(AccordionPreviousConfig::Preset(
        preset.resolved_config.unwrap(),
    ));
    assert_eq!(
        shuffle_accordion_layers(&score, same_as_preset).diagnostics[0].code,
        "no-different-result"
    );

    let mut empty = request();
    empty.allowed_accompaniments.clear();
    assert_eq!(
        shuffle_accordion_layers(&score, empty).diagnostics[0].code,
        "empty-eligible-pool"
    );
    let mut too_many = request();
    too_many.allowed_accompaniments = vec![Some(AccordionPattern::HeldBassChord); 9];
    too_many.allowed_melodic_voices = vec![
        AccordionMelodicVoice::Unchanged,
        AccordionMelodicVoice::ThirdBelow,
        AccordionMelodicVoice::SixthBelow,
        AccordionMelodicVoice::OctaveBelow,
    ];
    too_many.allowed_swing_policies = vec![
        AccordionSwingPolicy::Straight,
        AccordionSwingPolicy::NotatedAccompanimentTwoToOne,
    ];
    assert_eq!(
        shuffle_accordion_layers(&score, too_many).diagnostics[0].code,
        "input-too-large"
    );
    let mut bad_seed = request();
    bad_seed.seed = "-1".into();
    assert_eq!(
        shuffle_accordion_layers(&score, bad_seed).diagnostics[0].code,
        "invalid-seed"
    );
}

#[test]
fn free_layer_config_rejects_wrong_engine_contract_and_input() {
    let score = score();
    let options = LayeredAccordionOptions {
        chords: selection().chords,
        alternative_chords: None,
        inversion: None,
        accompaniment: Some(AccordionPattern::HeldBassChord),
        melodic_voice: AccordionMelodicVoice::SixthBelow,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::FreeBass,
    };
    let config = arrange_accordion_layers(&score, options.clone())
        .resolved_config
        .unwrap();
    let mut wrong_contract = config.clone();
    wrong_contract.contract_version += 1;
    assert_eq!(
        arrange_from_resolved_layers(&score, wrong_contract).diagnostics[0].code,
        "unsupported-version"
    );
    let mut wrong_engine = config.clone();
    wrong_engine.engine_version.push_str("-other");
    assert_eq!(
        arrange_from_resolved_layers(&score, wrong_engine).diagnostics[0].code,
        "unsupported-version"
    );
    let changed = acorde_io::parse_abc("X:1\nM:4/4\nL:1/4\nQ:1/4=60\nK:C\nd4|")
        .expect("different ABC parses");
    assert_eq!(
        arrange_from_resolved_layers(&changed, config.clone()).diagnostics[0].code,
        "input-mismatch"
    );
    let mut stale_previous = request();
    stale_previous.previous_config = Some(AccordionPreviousConfig::Layers(config));
    assert_eq!(
        shuffle_accordion_layers(&changed, stale_previous).diagnostics[0].code,
        "previous-mismatch"
    );

    let mut unsupported = options;
    unsupported.chords[0].chord.kind = "unrecognized".into();
    let failed = arrange_accordion_layers(&score, unsupported);
    assert!(failed.score.is_none());
    assert!(!failed.diagnostics.is_empty());
}
