use acorde_core::{
    AccordionBassMode, AccordionPresetSelection, BassLineOptions, BassLinePattern, ChordSymbol,
    ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, PositionedAccordionChord, Score,
    arrange_accordion_preset, arrange_bass_line, arrange_chordal_pad,
};

fn symbol(kind: &str) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root": "C", "kind": kind})).unwrap()
}

fn placed(kind: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index: 0,
        offset_beats: 0.0,
        chord: symbol(kind),
    }
}

#[test]
fn extended_chords_reach_pad_bass_and_free_bass_preset() {
    let source = Score::new("chord qualities", 120, 4, 4, 0, 1);
    let original = serde_json::to_value(&source.parts).unwrap();
    for (kind, expected) in [
        ("suspended-second", vec![0, 2, 7]),
        ("suspended-fourth", vec![0, 5, 7]),
        ("major-add9", vec![0, 2, 4, 7]),
        ("minor-add9", vec![0, 2, 3, 7]),
        ("half-diminished", vec![0, 3, 6, 10]),
    ] {
        let chord = placed(kind);
        let pad = arrange_chordal_pad(
            &source,
            ChordalPadOptions {
                chords: vec![chord.clone()],
                pattern: ChordalPadPattern::Held,
                pattern_sections: vec![],
                meter_group_sections: vec![],
                phrase_end_measures: vec![],
                voicing: ChordalPadVoicing::RootPosition,
                midi_program: 48,
                low_midi: 48,
                high_midi: 84,
                center_midi: 64,
                min_melody_gap_semitones: None,
                melody_part_index: None,
            },
        );
        assert!(pad.diagnostics.is_empty(), "{kind}: {:?}", pad.diagnostics);
        let pad_score = pad.score.unwrap();
        let mut classes = pad_score.parts[1].staves[0].measures[0].voices[0][0]
            .pitches
            .iter()
            .map(|pitch| (pitch.to_midi() as u8) % 12)
            .collect::<Vec<_>>();
        classes.sort_unstable();
        assert_eq!(classes, expected, "{kind}");
        assert_eq!(
            serde_json::to_value(&pad_score.parts[..1]).unwrap(),
            original
        );
        assert!(acorde_core::model::validate::validate(&pad_score).is_valid());

        let bass = arrange_bass_line(
            &pad_score,
            BassLineOptions {
                chords: vec![chord.clone()],
                pattern: BassLinePattern::RootFifth,
                midi_program: 32,
                low_midi: 28,
                high_midi: 60,
                center_midi: 40,
            },
        );
        assert!(
            bass.diagnostics.is_empty(),
            "{kind}: {:?}",
            bass.diagnostics
        );
        let bass_score = bass.score.unwrap();
        let bass_classes = bass_score.parts[2].staves[0].measures[0].voices[0]
            .iter()
            .map(|note| (note.pitches[0].to_midi() as u8) % 12)
            .collect::<Vec<_>>();
        let fifth = if kind == "half-diminished" { 6 } else { 7 };
        assert_eq!(bass_classes, vec![0, fifth, 0, fifth]);
        assert_eq!(
            serde_json::to_value(&bass_score.parts[..2]).unwrap(),
            serde_json::to_value(&pad_score.parts).unwrap()
        );
        assert!(acorde_core::model::validate::validate(&bass_score).is_valid());

        let selection = AccordionPresetSelection {
            chords: vec![chord],
            alternative_chords: None,
            inversion: None,
            bass_mode: AccordionBassMode::FreeBass,
        };
        let preset = arrange_accordion_preset(&source, selection.clone(), "bass-pulse");
        assert!(
            preset.diagnostics.is_empty(),
            "{kind}: {:?}",
            preset.diagnostics
        );
        assert!(preset.score.is_some(), "{kind}");
        let standard = arrange_accordion_preset(
            &source,
            AccordionPresetSelection {
                bass_mode: AccordionBassMode::StandardBass,
                ..selection
            },
            "bass-pulse",
        );
        assert!(standard.score.is_none(), "{kind}");
        assert_eq!(standard.diagnostics[0].code, "unsupported-standard-bass");
    }
}
