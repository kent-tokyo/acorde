#![cfg(feature = "musicxml")]

use acorde_core::{
    BeamState, Duration, Note, Pitch, Score,
    model::{
        performer_edition::{PerformerEdition, PerformerEditionRequest, arrange_performer_edition},
        rhythm_engraving::engrave_rhythm,
    },
    to_playback_events,
};
use acorde_io::{parse_musicxml, serialize_musicxml};

const SOURCE: &str =
    include_str!("../../../tests/fixtures/arrangement/editions/lantern-16.musicxml");
const SOLO: &str = include_str!("../../../tests/fixtures/arrangement/editions/solo.json");
const DUET: &str = include_str!("../../../tests/fixtures/arrangement/editions/duet.json");

fn playback(score: &Score) -> Vec<(usize, usize, u32, u32, i32, u8)> {
    let mut notes: Vec<_> = to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.part_index,
                e.source.unwrap().staff,
                (e.time_beats * 480.).round() as u32,
                (e.duration_beats * 480.).round() as u32,
                e.pitch_midi_cents,
                e.velocity,
            )
        })
        .collect();
    notes.sort();
    notes
}

#[test]
fn player_parts_preserve_original_melody_and_engraving_preserves_performance() {
    let source = parse_musicxml(SOURCE).unwrap();
    let original = playback(&source);
    assert_eq!(original.len(), 54);
    assert_eq!(original.last().unwrap().2, 57 * 480);
    for config in [SOLO, DUET] {
        let options = serde_json::from_str(config).unwrap();
        let edition = arrange_performer_edition(&source, &options).unwrap();
        let events = playback(&edition.score);
        let duet = edition.score.parts.len() == 2;
        assert_eq!(duet, config == DUET);
        let extra = events.iter().filter(|e| e.1 == 0).count() - original.len();
        if duet {
            assert_eq!(extra, 4);
        } else {
            assert!(extra > 0);
        }
        for &(part, staff, onset, duration, pitch, velocity) in &original {
            let expected_part = if duet && (13 * 480..29 * 480).contains(&onset) {
                1
            } else {
                part
            };
            assert!(events.contains(&(expected_part, staff, onset, duration, pitch, velocity)));
        }
        assert!(edition.left_hand.iter().all(|a| !a.button_id.is_empty()));
        let cleaned = engrave_rhythm(&edition.score, [2, 3]).unwrap();
        assert_eq!(playback(&cleaned), events);
        let imported = parse_musicxml(&serialize_musicxml(&cleaned).unwrap()).unwrap();
        assert_eq!(playback(&imported), events);
        let first = &cleaned.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            [first[0].beam, first[1].beam],
            [BeamState::Begin, BeamState::End]
        );
        assert!(
            cleaned.parts[0].staves[0].measures[1].voices[0].len()
                > edition.score.parts[0].staves[0].measures[1].voices[0].len()
        );
        for i in 0..edition.score.parts.len() {
            let part = cleaned.extract_part_checked(i).unwrap();
            assert_eq!(
                playback(&part).len(),
                events.iter().filter(|e| e.0 == i).count()
            );
        }
    }
}

#[test]
fn rejects_tie_handoff_and_missing_instrument_button_at_written_bar() {
    let source = parse_musicxml(SOURCE).unwrap();
    let mut options: PerformerEditionRequest = serde_json::from_str(DUET).unwrap();
    if let PerformerEdition::Duet {
        partner_measures, ..
    } = &mut options.edition
    {
        *partner_measures = vec![9];
    }
    let err = arrange_performer_edition(&source, &options).unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.measure_index),
        ("tie-handoff", Some(9))
    );
    options
        .accompaniment
        .profile
        .chord_buttons
        .retain(|b| b.id != "G chord");
    assert_eq!(
        arrange_performer_edition(&source, &options)
            .unwrap_err()
            .measure_index,
        Some(0)
    );
}

#[test]
fn eighth_note_grouping_follows_meter_and_authored_beams() {
    for (n, grouping, split) in [(6, [2, 3], 3), (5, [2, 3], 2), (5, [3, 2], 3)] {
        let mut source = Score::new("Groups", 96, n, 8, 0, 1);
        let voice = &mut source.parts[0].staves[0].measures[0].voices[0];
        *voice = (0..n)
            .map(|_| Note::new(Pitch::from_midi(60, false), Duration::Eighth))
            .collect();
        let score = engrave_rhythm(&source, grouping).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[split - 1].beam, BeamState::End);
        assert_eq!(notes[split].beam, BeamState::Begin);
        assert_eq!(playback(&score), playback(&source));
        source.parts[0].staves[0].measures[0].voices[0][0].explicit_unbeamed = true;
        assert!(
            engrave_rhythm(&source, grouping).unwrap().parts[0].staves[0].measures[0].voices[0]
                .iter()
                .all(|n| n.beam == BeamState::None)
        );
    }
}

#[test]
fn preserves_tuplet_boundaries_and_cleans_polyphonic_rests() {
    let tuplets = parse_musicxml(include_str!(
        "../../../tests/fixtures/adjacent-tuplets.musicxml"
    ))
    .unwrap();
    let cleaned = engrave_rhythm(&tuplets, [2, 3]).unwrap();
    assert_eq!(
        tuplets.parts[0].staves[0].measures[0].voices[0],
        cleaned.parts[0].staves[0].measures[0].voices[0]
    );
    let mut poly = Score::new("Rest voices", 90, 4, 4, 0, 1);
    let bar = &mut poly.parts[0].staves[0].measures[0];
    bar.voices[0] = vec![
        Note::new(Pitch::from_midi(74, false), Duration::Half),
        Note::rest(Duration::Half),
    ];
    bar.voices[1] = vec![
        Note::rest(Duration::Half),
        Note::new(Pitch::from_midi(55, false), Duration::Half),
    ];
    bar.voices[0][0].stem_up = Some(false);
    let engraved = engrave_rhythm(&poly, [2, 3]).unwrap();
    let notes = &engraved.parts[0].staves[0].measures[0].voices;
    assert_eq!(notes[0][0].stem_up, Some(false));
    assert_eq!(notes[0][1].relative_y, Some(10.));
    assert_eq!(notes[1][0].relative_y, Some(-10.));
    assert_eq!(notes[1][1].stem_up, Some(false));
    let imported = parse_musicxml(&serialize_musicxml(&engraved).unwrap()).unwrap();
    let voices = &imported.parts[0].staves[0].measures[0].voices;
    assert_eq!(voices[0][1].relative_y, Some(10.));
    assert_eq!(voices[1][0].relative_y, Some(-10.));
    assert_eq!(playback(&imported), playback(&poly));
}

#[test]
fn authored_harmony_wins_over_a_playable_alternative() {
    let source = parse_musicxml(SOURCE).unwrap();
    let mut options: PerformerEditionRequest = serde_json::from_str(SOLO).unwrap();
    options.accompaniment.selection.alternative_chords = Some(vec![]);
    let result = arrange_performer_edition(&source, &options).unwrap();
    assert_eq!(result.left_hand[0].candidate.bass_midi % 12, 7);
}
