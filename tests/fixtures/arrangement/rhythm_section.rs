use acorde_core::*;
pub fn source() -> Score {
    let mut s = Score::new("rhythm section", 108, 4, 4, 0, 4);
    s.id = "rhythm-section-fixture".into();
    s.parts[0].id = "melody".into();
    for (i, bar) in s.parts[0].staves[0].measures.iter_mut().enumerate() {
        let mut n = Note::new(
            Pitch::from_midi(72 + [0, 5, 7, 0][i], false),
            Duration::Whole,
        );
        n.id = format!("melody-{i}");
        bar.voices[0] = vec![n];
    }
    s
}
pub fn request(muted: bool) -> RhythmSectionRequest {
    let entries=[("C","major"),("F","major"),("G","dominant"),("C","major")].iter().enumerate().map(|(m,(root,kind))|serde_json::json!({"position":{"measure_index":m,"offset":{"numerator":0,"denominator":1}},"offset_beats":0,"chord":{"root":root,"kind":kind},"origin":"host-authored","sources":[]})).collect::<Vec<_>>();
    let bars = (0..4)
        .map(|measure| RhythmSectionBar {
            measure,
            slots: (0..8)
                .map(|slot| {
                    let kick = if matches!(slot, 0 | 2 | 4) {
                        Some(RhythmKickAttack {
                            hold_ticks: 8,
                            accent: slot == 0,
                        })
                    } else {
                        None
                    };
                    let bass = if matches!(slot, 0 | 3 | 4 | 6 | 7) {
                        Some(RhythmBassAttack {
                            tone: match slot {
                                3 | 6 => RhythmBassTone::Fifth,
                                7 if measure < 3 => RhythmBassTone::Approach {
                                    target: RhythmAttackRef {
                                        measure: measure + 1,
                                        slot: 0,
                                    },
                                    direction: BassApproachDirection::Below,
                                    semitones: 1,
                                },
                                _ => RhythmBassTone::Root,
                            },
                            hold_ticks: if slot == 3 || slot == 7 { 8 } else { 12 },
                            accent: slot == 0,
                            ghost: slot == 3,
                        })
                    } else {
                        None
                    };
                    RhythmSectionSlot {
                        ticks: 16,
                        kick,
                        bass,
                        synchronize: matches!(slot, 0 | 4),
                    }
                })
                .collect(),
        })
        .collect();
    RhythmSectionRequest {
        chords: serde_json::from_value(serde_json::json!({"contract_version":1,"entries":entries}))
            .unwrap(),
        bars,
        start_measure: 0,
        end_measure_exclusive: 4,
        locked_measures: vec![],
        low_midi: 36,
        high_midi: 55,
        center_midi: 43,
        max_leap_semitones: 12,
        bass_program: 33,
        ghost_sound: if muted {
            BassGhostSound::MutedProgram {
                midi_program: 34,
                sound_identity: "acorde-rhythm-audition-v1/muted-bass".into(),
            }
        } else {
            BassGhostSound::ShortQuiet
        },
    }
}
pub fn sound(score: &Score) -> Vec<(usize, u8, u8, u8, u64, u64, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.part_index,
                e.channel,
                e.program,
                e.pitch_midi,
                (e.time_beats * 960000.0).round() as u64,
                (e.duration_beats * 960000.0).round() as u64,
                e.velocity,
            )
        })
        .collect()
}
