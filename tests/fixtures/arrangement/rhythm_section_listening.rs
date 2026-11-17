use acorde_core::*;
#[allow(dead_code)]
#[path = "harmony_targets_listening.rs"]
mod harmony;
#[allow(dead_code)]
#[path = "rhythm_section.rs"]
pub mod small;
pub use harmony::{Case, Corpus};
#[allow(unused_imports)]
pub use harmony::input as harmony_input;
pub const FIXTURE: &str = include_str!("rhythm-section-listening.json");
type ListeningInput = (Score, Vec<(&'static str, RhythmSectionRequest)>);
pub fn input(case: &Case) -> Result<ListeningInput, Box<dyn std::error::Error>> {
    let (melody, harmonies) = harmony::input(case);
    let mut h = suggest_phrase_harmony_targets(&melody, &harmonies[0].1)?;
    if !h.complete || h.candidates.len() != 1 {
        return Err("cannot realize listening harmony".into());
    }
    let candidate = h.candidates.remove(0);
    let mut source = candidate.arrangement.score;
    for (p, part) in source.parts.iter_mut().enumerate() {
        part.id = format!("{}-{p}", case.id);
        for (s, staff) in part.staves.iter_mut().enumerate() {
            for (m, bar) in staff.measures.iter_mut().enumerate() {
                for (v, voice) in bar.voices.iter_mut().enumerate() {
                    for (n, note) in voice.iter_mut().enumerate() {
                        note.id = format!("{}-{p}-{s}-{m}-{v}-{n}", case.id);
                    }
                }
            }
        }
    }
    let count = case.melody.len();
    let mut coordinated = small::request(false);
    coordinated.chords = candidate.resolved.arrangement.request.chords;
    coordinated.end_measure_exclusive = count;
    let template = coordinated.bars[0].clone();
    coordinated.bars = (0..count)
        .map(|m| {
            let mut bar = template.clone();
            bar.measure = m;
            if let Some(b) = bar.slots[7].bass.as_mut() {
                b.tone = if m + 1 < count {
                    RhythmBassTone::Approach {
                        target: RhythmAttackRef {
                            measure: m + 1,
                            slot: 0,
                        },
                        direction: BassApproachDirection::Below,
                        semitones: 1,
                    }
                } else {
                    RhythmBassTone::Root
                };
            }
            bar
        })
        .collect();
    let short_quiet = coordinated.clone();
    for bar in &mut coordinated.bars {
        bar.slots[3].bass = None;
    }
    let mut muted = short_quiet.clone();
    muted.ghost_sound = small::request(true).ghost_sound;
    let mut pulse = coordinated.clone();
    for bar in &mut pulse.bars {
        for (i, slot) in bar.slots.iter_mut().enumerate() {
            slot.kick = if matches!(i, 0 | 4) {
                Some(RhythmKickAttack {
                    hold_ticks: 8,
                    accent: i == 0,
                })
            } else {
                None
            };
            slot.bass = if i % 2 == 0 {
                Some(RhythmBassAttack {
                    tone: RhythmBassTone::Root,
                    hold_ticks: 12,
                    accent: i == 0,
                    ghost: false,
                })
            } else {
                None
            };
            slot.synchronize = matches!(i, 0 | 4);
        }
    }
    Ok((
        source,
        vec![
            ("pulse", pulse),
            ("coordinated", coordinated),
            ("short-quiet-ghosts", short_quiet),
            ("muted-ghosts", muted),
        ],
    ))
}
