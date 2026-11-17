#[path = "../../../tests/fixtures/arrangement/harmony_arrangement_source.rs"]
mod fixture;
use acorde_core::{
    Barline, BeatFraction, Duration, HarmonyAnticipation, HarmonyAnticipationJoin as Join,
    HarmonyAnticipationLead as Lead, HarmonyArrangementError as Error,
    HarmonyCompingFailure as Failure, HarmonyCompingPattern, MeasureLength, RootVoicingDecision,
    arrange_harmony_voicings, replay_harmony_arrangement,
};

fn anticipated(join: Join, lead: Lead) -> acorde_core::HarmonyArrangementRequest {
    let mut request = fixture::request();
    request.comping.anticipations = vec![HarmonyAnticipation {
        chord_entry: 1,
        lead,
        join,
    }];
    request
}

#[test]
fn next_harmony_sounds_early_and_ties_across_its_original_boundary() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].voices[0] = [84, 84, 84, 72]
        .into_iter()
        .map(|midi| {
            acorde_core::Note::new(
                acorde_core::Pitch::from_midi(midi, false),
                Duration::Quarter,
            )
        })
        .collect();
    let request = anticipated(Join::Tie, Lead::Eighth);
    let original_position = request.chords.entries[1].position.clone();
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    assert_eq!(
        result.resolved.request.chords.entries[1].position,
        original_position
    );
    let spans = result
        .spans
        .iter()
        .filter(|s| s.chord_entry == 1)
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].measure, 0);
    assert_eq!(
        spans[0].start,
        BeatFraction {
            numerator: 7,
            denominator: 2
        }
    );
    assert!(spans[0].tie_to_next && spans[1].tie_from_previous);
    assert_eq!(spans[0].attack, spans[1].attack);
    let events = fixture::sound(&result.score)
        .into_iter()
        .filter(|e| e.0 == result.added_part && e.2 == 3360)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|e| e.3 == 4320));
    // The old D bass remains audible before G13's nominal boundary.
    let node = spans[0].node;
    assert_eq!(result.context.nodes[node].bass_midi, None);
    assert_eq!(result.context.nodes[node].melody_ceiling_midi, Some(69));
    assert_eq!(
        result.voicings.voicings[node].root_decision,
        RootVoicingDecision::IncludedWithoutBassSupport
    );
    let replay = replay_harmony_arrangement(&source, &result.resolved).unwrap();
    assert_eq!(fixture::sound(&result.score), fixture::sound(&replay.score));
}

#[test]
fn sixteenth_anticipation_can_reattack_on_the_nominal_chord() {
    let result = arrange_harmony_voicings(
        &fixture::source(),
        &anticipated(Join::Reattack, Lead::Sixteenth),
    )
    .unwrap();
    let events = fixture::sound(&result.score)
        .into_iter()
        .filter(|e| e.0 == result.added_part)
        .collect::<Vec<_>>();
    assert_eq!(
        events.iter().filter(|e| e.2 == 3600 && e.3 == 240).count(),
        4
    );
    assert_eq!(
        events.iter().filter(|e| e.2 == 3840 && e.3 == 3840).count(),
        4
    );
    assert!(
        result
            .spans
            .iter()
            .all(|s| !s.tie_from_previous && !s.tie_to_next)
    );
}

#[test]
fn offbeats_leave_rests_and_obey_attack_density_limits() {
    let source = fixture::source();
    let mut request = anticipated(Join::Tie, Lead::Eighth);
    request.comping.pattern = HarmonyCompingPattern::OffbeatPulse;
    request.comping.max_attacks_per_measure = 4;
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    let mut attacks = fixture::sound(&result.score)
        .into_iter()
        .filter(|e| e.0 == result.added_part)
        .map(|e| e.2)
        .collect::<Vec<_>>();
    attacks.sort_unstable();
    attacks.dedup();
    assert_eq!(attacks, (0..12).map(|i| i * 960 + 480).collect::<Vec<_>>());
    assert!(
        result.score.parts[2].staves[0]
            .measures
            .iter()
            .flat_map(|m| &m.voices[0])
            .any(|n| n.is_rest)
    );
    request.comping.max_attacks_per_measure = 3;
    assert!(matches!(
        arrange_harmony_voicings(&source, &request),
        Err(Error::Comping {
            reason: Failure::AttackLimit,
            ..
        })
    ));
}

#[test]
fn anticipation_cannot_write_into_locked_or_unselected_bars() {
    let source = fixture::source();
    let mut request = anticipated(Join::Tie, Lead::Eighth);
    request.locked_measures = vec![0];
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 1,
            reason: Failure::LockedMeasure
        }
    );
    request.locked_measures.clear();
    request.start_measure = 1;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 1,
            reason: Failure::OutsideRange
        }
    );
}

#[test]
fn branching_repeat_destinations_and_tie_predecessors_are_rejected() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_right = Barline::RepeatEnd;
    let request = anticipated(Join::Tie, Lead::Eighth);
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 1,
            reason: Failure::AmbiguousRepeat
        }
    );
    source.parts[0].staves[0].measures[0].barline_right = Barline::Normal;
    source.parts[0].staves[0].measures[1].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[2].barline_right = Barline::RepeatEnd;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 1,
            reason: Failure::AmbiguousRepeat
        }
    );
    // Reattack has no incoming tie to confuse the second visit.
    let result =
        arrange_harmony_voicings(&source, &anticipated(Join::Reattack, Lead::Eighth)).unwrap();
    assert!(result.context.edges.iter().any(|edge| edge.repeat_boundary));
}

#[test]
fn pickup_start_is_exact_and_nonexistent_future_harmony_is_an_error() {
    let mut source = fixture::source();
    for part in &mut source.parts {
        let bar = &mut part.staves[0].measures[0];
        bar.actual_length = Some(MeasureLength {
            numerator: 1,
            denominator: 8,
        });
        bar.voices[0][0].duration = Duration::Eighth;
    }
    let mut request = anticipated(Join::Tie, Lead::Eighth);
    let result = arrange_harmony_voicings(&source, &request).unwrap();
    assert_eq!(result.spans[0].start, BeatFraction::ZERO);
    assert_eq!(result.spans[0].chord_entry, 1);
    request.comping.anticipations[0].chord_entry = 3;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 3,
            reason: Failure::InvalidEntry
        }
    );
    request.comping.anticipations[0].chord_entry = 0;
    assert_eq!(
        arrange_harmony_voicings(&source, &request).unwrap_err(),
        Error::Comping {
            entry: 0,
            reason: Failure::BeforeSource
        }
    );
}

#[test]
fn accents_belong_to_attacks_and_never_to_tie_continuations() {
    for join in [Join::Tie, Join::Reattack] {
        let source = fixture::source();
        let mut request = anticipated(join, Lead::Eighth);
        let plain = arrange_harmony_voicings(&source, &request).unwrap();
        request.comping.accent = acorde_core::HarmonyCompingAccent::Anticipations;
        let accented = arrange_harmony_voicings(&source, &request).unwrap();
        let a = fixture::sound(&plain.score);
        let b = fixture::sound(&accented.score);
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert_eq!((a.0, a.1, a.2, a.3), (b.0, b.1, b.2, b.3));
            assert_eq!(
                b.4,
                a.4 + if b.0 == accented.added_part && b.2 == 3360 {
                    20
                } else {
                    0
                }
            );
        }
        for note in accented.score.parts[accented.added_part].staves[0]
            .measures
            .iter()
            .flat_map(|m| &m.voices[0])
        {
            if note.tie_end {
                assert!(note.articulations.is_empty());
            }
        }
        assert_eq!(accented.spans.iter().filter(|s| s.accent).count(), 1);
    }
}
