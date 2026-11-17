#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    InnerVoicesError, InnerVoicesRequest, Note, Score, arrange_inner_voices, replay_inner_voices,
};

fn request() -> InnerVoicesRequest {
    fixture::inner_voice_request()
}

fn attacks(voice: &[Note]) -> Vec<(u32, u8)> {
    let mut tick = 0u32;
    let mut attacks = Vec::new();
    for note in voice {
        if !note.is_rest && !note.tie_end {
            attacks.push((tick, note.pitches[0].to_midi() as u8));
        }
        tick += (note.beats() * 32.0).round() as u32;
    }
    attacks
}

#[test]
fn distinct_lines_preserve_source_and_replay() {
    let source = fixture::source();
    let request = request();
    let result = arrange_inner_voices(&source, request.clone()).unwrap();
    assert_eq!(result.added_part, 1);
    assert_eq!(
        serde_json::to_value(&result.score.parts[0]).unwrap(),
        serde_json::to_value(&source.parts[0]).unwrap()
    );
    assert_eq!(result.score.parts[1].staves[0].measures.len(), 16);
    assert!(result.attack_counts[0] >= 16);
    assert!(result.attack_counts[1] > result.attack_counts[0]);
    let mut previous = [None, None];
    for (index, bar) in result.score.parts[1].staves[0].measures.iter().enumerate() {
        let lower = attacks(&bar.voices[0]);
        let upper = attacks(&bar.voices[1]);
        assert!(!lower.is_empty() && !upper.is_empty());
        assert_ne!(
            lower.iter().map(|(tick, _)| tick).collect::<Vec<_>>(),
            upper.iter().map(|(tick, _)| tick).collect::<Vec<_>>()
        );
        for (voice, pitches) in [lower, upper].iter().enumerate() {
            for (_, pitch) in pitches {
                let line = if voice == 0 {
                    &request.lower
                } else {
                    &request.upper
                };
                assert!((line.low_midi..=line.high_midi).contains(pitch));
                if let Some(prior) = previous[voice] {
                    assert!(
                        pitch.abs_diff(prior) <= line.max_leap_semitones,
                        "voice {voice} bar {index}"
                    );
                }
                previous[voice] = Some(*pitch);
            }
        }
    }
    let replay = replay_inner_voices(&source, &result.resolved).unwrap();
    assert_eq!(
        fixture::events(&replay.score),
        fixture::events(&result.score)
    );
    let mut changed = source.clone();
    changed.metadata.title.push('!');
    assert!(matches!(
        replay_inner_voices(&changed, &result.resolved),
        Err(InnerVoicesError::SourceMismatch)
    ));
}

#[test]
fn invalid_ranges_and_chords_fail_atomically() {
    let source = fixture::source();
    let original = serde_json::to_value(&source).unwrap();
    let mut bad = request();
    bad.upper.low_midi = bad.lower.high_midi + 1;
    assert!(matches!(
        arrange_inner_voices(&source, bad),
        Err(InnerVoicesError::InvalidRequest)
    ));
    let mut bad = request();
    bad.chords[0].offset_beats = 2.0;
    assert!(arrange_inner_voices(&source, bad).is_err());
    let mut bad = request();
    bad.lower.max_leap_semitones = 0;
    assert!(matches!(
        arrange_inner_voices(&source, bad),
        Err(InnerVoicesError::Unplayable { voice: 0, .. })
    ));
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}

#[test]
fn inactive_bars_and_irregular_meter_keep_two_complete_voices() {
    let mut source = Score::new("5/8", 100, 5, 8, 0, 2);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = vec![
            Note::new(
                acorde_core::Pitch::from_midi(84, false),
                acorde_core::Duration::Quarter,
            ),
            Note::rest(acorde_core::Duration::Quarter),
            Note::rest(acorde_core::Duration::Eighth),
        ];
    }
    let mut req = request();
    req.chords.truncate(2);
    req.chords[1].measure_index = 1;
    req.active_measures = vec![true, false];
    req.phrase_end_measures = vec![];
    let result = arrange_inner_voices(&source, req).unwrap();
    for bar in &result.score.parts[1].staves[0].measures {
        for voice in &bar.voices[..2] {
            assert_eq!(
                (voice.iter().map(Note::beats).sum::<f64>() * 32.0).round() as u32,
                80
            );
        }
    }
    assert!(
        result.score.parts[1].staves[0].measures[1].voices[..2]
            .iter()
            .all(|voice| voice.iter().all(|note| note.is_rest))
    );
}
