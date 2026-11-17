#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/rhythm_section_listening.rs"]
mod listening;
use acorde_core::*;
use listening::small as fixture;
fn events(result: &RhythmSectionResult, addr: &NoteAddr) -> Vec<PlaybackEvent> {
    to_playback_events(&result.score, &Default::default())
        .into_iter()
        .filter(|e| e.source.as_ref() == Some(addr))
        .collect()
}
#[test]
fn chosen_sync_points_rest_lengths_and_approaches_match_actual_playback() {
    let source = fixture::source();
    let before = serde_json::to_value(&source).unwrap();
    let request = fixture::request(false);
    let r = arrange_rhythm_section(&source, &request).unwrap();
    assert!(validate(&r.score).is_valid());
    assert_eq!(
        serde_json::to_value(&r.score.parts[..source.parts.len()]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
    for a in &r.attacks {
        if a.synchronized {
            let b = events(&r, a.bass.as_ref().unwrap());
            let k = events(&r, a.kick.as_ref().unwrap());
            assert_eq!(b[0].time_beats, k[0].time_beats);
        }
        if a.at.slot == 2 {
            assert!(a.bass.is_none() && a.kick.is_some());
        }
        if a.at.slot == 6 {
            assert!(a.bass.is_some() && a.kick.is_none());
        }
    }
    for b in &r.resolved.bass {
        if let Some(t) = &b.approach {
            assert_eq!(t.performed_resolutions, 1);
            assert_eq!(b.pitch.to_midi() + 1, t.target_pitch.to_midi());
        }
    }
    for p in r.score.parts.iter().skip(source.parts.len()) {
        for m in &p.staves[0].measures {
            assert!(
                (m.voices[0].iter().map(|n| n.duration.beats(0)).sum::<f64>() - 4.0).abs() < 1e-9
            );
        }
    }
    let saved: ResolvedRhythmSection =
        serde_json::from_str(&serde_json::to_string(&r.resolved).unwrap()).unwrap();
    let replay = replay_rhythm_section(&source, &saved).unwrap();
    assert_eq!(fixture::sound(&r.score), fixture::sound(&replay.score));
    assert_eq!(before, serde_json::to_value(&source).unwrap());
}
#[test]
fn ghost_notation_is_quiet_short_and_normal_dynamics_resume() {
    let source = fixture::source();
    for muted in [false, true] {
        let r = arrange_rhythm_section(&source, &fixture::request(muted)).unwrap();
        assert_eq!(r.ghost_part.is_some(), muted);
        assert_eq!(
            r.diagnostics,
            vec![if muted {
                RhythmSectionDiagnostic::CallerSuppliedMutedSound
            } else {
                RhythmSectionDiagnostic::ShortQuietGhostApproximation
            }]
        );
        for a in &r.attacks {
            if a.at.slot == 3 {
                let addr = a.bass.as_ref().unwrap();
                let n =
                    &r.score.parts[addr.part].staves[0].measures[addr.measure].voices[0][addr.note];
                assert_eq!(n.pitch_parentheses, vec![true]);
                let e = &events(&r, addr)[0];
                assert_eq!(e.velocity, 36);
                assert_eq!(e.duration_beats, 0.25);
                assert_eq!(e.program, if muted { 34 } else { 33 });
            }
            if a.at.slot == 4 {
                assert_eq!(
                    events(&r, a.bass.as_ref().unwrap())[0].velocity,
                    Dynamic::Mf.to_velocity()
                );
            }
        }
    }
}
#[test]
fn repeat_edges_must_reach_the_saved_approach_target() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    assert!(matches!(
        arrange_rhythm_section(&source, &fixture::request(false)),
        Err(RhythmSectionError::Approach { .. })
    ));
    source.parts[0].staves[0].measures[1].barline_right = Barline::Normal;
    source.parts[0].staves[0].measures[3].barline_right = Barline::RepeatEnd;
    let r = arrange_rhythm_section(&source, &fixture::request(false)).unwrap();
    assert!(
        r.resolved
            .bass
            .iter()
            .filter_map(|b| b.approach.as_ref())
            .all(|a| a.performed_resolutions == 2)
    );
}
#[test]
fn invalid_plans_locks_and_tampered_replays_fail_atomically() {
    let s = fixture::source();
    let base = fixture::request(false);
    let mut r = base.clone();
    r.locked_measures = vec![1];
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Locked { measure: 1 })
    ));
    r = base.clone();
    r.bars[0].slots[0].ticks = 0;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Rhythm { .. })
    ));
    r = base.clone();
    r.bars[0].slots[0].bass = None;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Rhythm { .. })
    ));
    r = base.clone();
    r.chords.entries[0].position.offset.denominator = 0;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Chord { .. })
    ));
    r = base.clone();
    r.bars[0].slots[3].bass.as_mut().unwrap().hold_ticks = 16;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Rhythm { .. })
    ));
    let result = arrange_rhythm_section(&s, &base).unwrap();
    let mut resolved = result.resolved.clone();
    resolved.bass[0].pitch.octave += 1;
    assert!(matches!(
        replay_rhythm_section(&s, &resolved),
        Err(RhythmSectionError::ReplaySelection)
    ));
    resolved = result.resolved.clone();
    resolved.engine_version = "other".into();
    assert!(matches!(
        replay_rhythm_section(&s, &resolved),
        Err(RhythmSectionError::ReplayVersion)
    ));
    let mut changed = s.clone();
    changed.settings.tempo_bpm += 1;
    assert!(matches!(
        replay_rhythm_section(&changed, &result.resolved),
        Err(RhythmSectionError::ReplaySource)
    ));
}
#[test]
fn fixed_eight_and_sixteen_bar_rhythm_comparisons_are_distinct_and_replayable() {
    let corpus: listening::Corpus = serde_json::from_str(listening::FIXTURE).unwrap();
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|c| c.melody.len())
            .collect::<Vec<_>>(),
        vec![8, 16]
    );
    for case in corpus.cases {
        let (source, requests) = listening::input(&case).unwrap();
        let mut sounds = std::collections::BTreeSet::new();
        let mut main_bass = None;
        for (profile, request) in requests {
            let r = arrange_rhythm_section(&source, &request).unwrap();
            let line = r
                .resolved
                .bass
                .iter()
                .filter(|b| b.at.slot != 3)
                .map(|b| (b.at, b.pitch.clone()))
                .collect::<std::collections::BTreeMap<_, _>>();
            if profile == "coordinated" {
                main_bass = Some(line.clone());
            }
            if profile.ends_with("ghosts") {
                assert_eq!(
                    main_bass.as_ref(),
                    Some(&line),
                    "ghosts changed ordinary bass attacks"
                );
            }
            assert!(sounds.insert(fixture::sound(&r.score)));
            assert_eq!(
                fixture::sound(&r.score),
                fixture::sound(&replay_rhythm_section(&source, &r.resolved).unwrap().score)
            );
            assert_eq!(
                serde_json::to_value(&r.score.parts[..source.parts.len()]).unwrap(),
                serde_json::to_value(&source.parts).unwrap()
            );
        }
    }
}
#[test]
fn locked_bars_are_silent_when_omitted_and_unsupported_routes_fail() {
    let s = fixture::source();
    let mut r = fixture::request(false);
    r.bars.retain(|b| b.measure == 3);
    r.locked_measures = vec![0, 1, 2];
    let result = arrange_rhythm_section(&s, &r).unwrap();
    assert!(
        to_playback_events(&result.score, &Default::default())
            .iter()
            .filter(|e| e.part_index >= result.bass_part)
            .all(|e| e.time_beats >= 12.0)
    );
    let mut drums = s.clone();
    drums.parts[0].midi_channel = 9;
    assert!(matches!(
        arrange_rhythm_section(&drums, &r),
        Err(RhythmSectionError::PercussionChannelInUse)
    ));
    let mut malformed = s.clone();
    malformed.parts[0].staves[0].measures.pop();
    assert!(arrange_rhythm_section(&malformed, &r).is_err());
}
#[test]
fn pickups_and_meter_changes_keep_every_generated_bar_exact() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].actual_length = Some(MeasureLength {
        numerator: 1,
        denominator: 4,
    });
    source.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
    source.parts[0].staves[0].measures[2].time_sig = Some(TimeSignature {
        numerator: 3,
        denominator: 4,
    });
    for bar in source.parts[0].staves[0].measures.iter_mut().skip(2) {
        bar.voices[0] = vec![
            Note::new(Pitch::from_midi(72, false), Duration::Half),
            Note::rest(Duration::Quarter),
        ];
    }
    let mut r = fixture::request(false);
    for (m, bar) in r.bars.iter_mut().enumerate() {
        bar.slots.truncate(if m == 0 {
            2
        } else if m >= 2 {
            6
        } else {
            8
        });
    }
    let result = arrange_rhythm_section(&source, &r).unwrap();
    for p in result.score.parts.iter().skip(result.bass_part) {
        let lengths = p.staves[0]
            .measures
            .iter()
            .map(|m| m.voices[0].iter().map(Note::beats).sum::<f64>())
            .collect::<Vec<_>>();
        assert_eq!(lengths, vec![1.0, 4.0, 3.0, 3.0]);
    }
}
#[test]
fn slash_bass_and_chromatic_approaches_keep_the_target_spelling() {
    let s = fixture::source();
    let mut r = fixture::request(false);
    r.chords.entries[1].chord.bass = Some("Gb".into());
    r.bars[1].slots[0].bass.as_mut().unwrap().tone = RhythmBassTone::SlashBass;
    let result = arrange_rhythm_section(&s, &r).unwrap();
    let arrival = result
        .resolved
        .bass
        .iter()
        .find(|b| {
            b.at == RhythmAttackRef {
                measure: 1,
                slot: 0,
            }
        })
        .unwrap();
    assert_eq!(arrival.pitch.step, Step::G);
    assert_eq!(arrival.pitch.alter, -1);
    let approach = result
        .resolved
        .bass
        .iter()
        .find(|b| {
            b.at == RhythmAttackRef {
                measure: 0,
                slot: 7,
            }
        })
        .unwrap();
    assert_eq!(approach.pitch.step, Step::F);
    assert_eq!(approach.pitch.alter, 0);
    assert_eq!(approach.pitch.to_midi() + 1, arrival.pitch.to_midi());
    r.bars[0].slots[7].bass.as_mut().unwrap().tone = RhythmBassTone::Approach {
        target: RhythmAttackRef {
            measure: 1,
            slot: 0,
        },
        direction: BassApproachDirection::Above,
        semitones: 2,
    };
    let result = arrange_rhythm_section(&s, &r).unwrap();
    let a = result
        .resolved
        .bass
        .iter()
        .find(|b| {
            b.at == RhythmAttackRef {
                measure: 0,
                slot: 7,
            }
        })
        .unwrap();
    assert_eq!(
        a.pitch.to_midi() - 2,
        a.approach.as_ref().unwrap().target_pitch.to_midi()
    );
}
#[test]
fn invalid_approach_chains_unperformed_bars_and_impossible_register_are_errors() {
    let s = fixture::source();
    let mut r = fixture::request(false);
    r.bars[1].slots[0].bass.as_mut().unwrap().tone = RhythmBassTone::Approach {
        target: RhythmAttackRef {
            measure: 1,
            slot: 4,
        },
        direction: BassApproachDirection::Below,
        semitones: 1,
    };
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Approach { .. })
    ));
    let mut skipped = s.clone();
    skipped.parts[0].staves[0].measures[0].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    assert!(matches!(
        arrange_rhythm_section(&skipped, &fixture::request(false)),
        Err(RhythmSectionError::UnperformedMeasure { measure: 0 })
    ));
    r = fixture::request(false);
    r.low_midi = 36;
    r.high_midi = 36;
    r.center_midi = 36;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Register { .. })
    ));
    r = fixture::request(false);
    r.max_leap_semitones = 0;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Leap { .. })
    ));
    r = fixture::request(false);
    r.bars[0].slots[0].bass.as_mut().unwrap().hold_ticks = 17;
    assert!(matches!(
        arrange_rhythm_section(&s, &r),
        Err(RhythmSectionError::Rhythm { .. })
    ));
}
