#[path = "../../../tests/fixtures/arrangement/standard_bass_staves.rs"]
mod fixture;

use acorde_core::{
    Clef, Score, arrange_accordion_layers, arrange_accordion_preset, arrange_from_resolved_layers,
    arrange_from_resolved_preset, to_playback_events,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};
use std::collections::BTreeMap;

type Event = (f64, f64, u8);

fn events(score: &Score) -> Vec<Event> {
    let mut events: Vec<_> = to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| (event.time_beats, event.duration_beats, event.pitch_midi))
        .collect();
    events.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    events
}

#[test]
fn three_preset_examples_preserve_every_event_through_musicxml_and_replay() {
    // Playback captured at main 8e87ab76 before the staff layout change.
    let baseline: BTreeMap<String, Vec<Event>> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/standard-bass-staves-events.json"
    ))
    .unwrap();
    for (preset, abc) in fixture::EXAMPLES {
        let source = acorde_io::parse_abc(abc).unwrap();
        let result = arrange_accordion_preset(&source, fixture::selection(), preset);
        let score = result.score.unwrap();
        let staves = &score.parts[0].staves;
        assert_eq!(
            staves.iter().map(|staff| &staff.clef).collect::<Vec<_>>(),
            [&Clef::Treble, &Clef::Treble, &Clef::Bass]
        );
        assert_eq!(events(&score), baseline[preset], "{preset}");
        let xml = acorde_io::serialize_musicxml(&score).unwrap();
        let restored = acorde_io::parse_musicxml(&xml).unwrap();
        assert_eq!(restored.parts[0].staves.len(), 3);
        assert_eq!(events(&restored), baseline[preset], "{preset} MusicXML");

        let mut config = result.resolved_config.unwrap();
        let replay = arrange_from_resolved_preset(&source, config.clone());
        assert_eq!(replay.generated_notes, result.generated_notes);
        assert_eq!(events(&replay.score.unwrap()), baseline[preset]);
        config.engine_version = "1.3.14+arrangement-presets.2".into();
        let old = arrange_from_resolved_preset(&source, config);
        let old_score = old.score.unwrap();
        assert_eq!(old_score.parts[0].staves.len(), 2);
        assert!(old.generated_notes.iter().all(|address| address.staff == 1));
        assert_eq!(events(&old_score), baseline[preset]);

        let expected_addresses = staves
            .iter()
            .enumerate()
            .skip(1)
            .flat_map(|(staff, stave)| {
                stave
                    .measures
                    .iter()
                    .enumerate()
                    .flat_map(move |(measure, bar)| {
                        bar.voices[0]
                            .iter()
                            .enumerate()
                            .filter(|(_, note)| !note.is_rest)
                            .map(move |(note, _)| (measure, note, staff))
                    })
            })
            .collect::<Vec<_>>();
        let mut actual_addresses = result
            .generated_notes
            .iter()
            .map(|address| {
                assert_eq!((address.part, address.voice), (0, 0));
                (address.measure, address.note, address.staff)
            })
            .collect::<Vec<_>>();
        actual_addresses.sort_unstable();
        let mut expected_addresses = expected_addresses;
        expected_addresses.sort_unstable();
        assert_eq!(actual_addresses, expected_addresses, "{preset} addresses");

        let svg = render_svg(
            &score,
            &SvgRenderOptions {
                width: 390.0,
                measures_per_system: 1,
                interactive: true,
                ..Default::default()
            },
        )
        .unwrap();
        for address in &result.generated_notes {
            let marker = format!(
                "data-note-addr=\"0:{}:{}:0:{}\"",
                address.staff, address.measure, address.note
            );
            let group = svg
                .split_once(&marker)
                .unwrap()
                .1
                .split_once("</g>")
                .unwrap()
                .0;
            assert!(group.contains("class=\"acorde-notehead\""));
            if address.staff == 1 {
                assert!(
                    group.matches("class=\"acorde-ledger\"").count() <= 1,
                    "{preset}: {marker}"
                );
            }
        }
    }
}

#[test]
fn held_bass_and_chord_ties_survive_the_layer_layout_and_musicxml() {
    let source = acorde_io::parse_abc("X:1\nM:3/4\nL:1/4\nK:C\ne3 | e3 | e3 | e3 |").unwrap();
    let mut selection = fixture::selection();
    selection.chords.truncate(1);
    let preset = arrange_accordion_preset(&source, selection, "held-chord");
    let mut options = preset.resolved_config.unwrap().options;
    options.melodic_voice = acorde_core::AccordionMelodicVoice::ThirdBelow;
    let result = arrange_accordion_layers(&source, options);
    let arranged = result
        .score
        .unwrap_or_else(|| panic!("{:?}", result.diagnostics));
    let staves = &arranged.parts[0].staves;
    assert_eq!(staves.len(), 3);
    for staff in &staves[1..] {
        let notes = &staff.measures[0].voices[0];
        assert!(notes[0].tie_start && notes[1].tie_end);
    }
    assert!(
        result
            .generated_notes
            .iter()
            .any(|a| a.staff == 0 && a.voice == 1)
    );
    let mut config = result.resolved_config.unwrap();
    let replay = arrange_from_resolved_layers(&source, config.clone());
    assert_eq!(replay.generated_notes, result.generated_notes);
    assert_eq!(events(&replay.score.unwrap()), events(&arranged));
    config.engine_version = "1.3.14+arrangement-layers.1".into();
    let legacy = arrange_from_resolved_layers(&source, config).score.unwrap();
    assert_eq!(legacy.parts[0].staves.len(), 2);
    assert_eq!(events(&legacy), events(&arranged));
    let xml = acorde_io::serialize_musicxml(&arranged).unwrap();
    assert_eq!(
        events(&acorde_io::parse_musicxml(&xml).unwrap()),
        events(&arranged)
    );
}
